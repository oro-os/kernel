//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate is a library with the core kernel functionality, datatypes,
//! etc. and provides a common interface for architectures to implement
//! the Oro kernel on their respective platforms.
#![no_std]
// SAFETY(qix-): `adt_const_params` isn't strictly necessary but is on track for acceptance,
// SAFETY(qix-): and the open questions (e.g. mangling) are not of concern here.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/95174
#![feature(adt_const_params)]
// SAFETY(qix-): Technically not required but helps clean things up a bit for the archs.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/29661
#![feature(associated_type_defaults)]
// SAFETY(qix-): Needed to make the system call key checks work inline.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/76001
#![feature(inline_const_pat)]
// SAFETY(qix-): Necessary to make the hashbrown crate wrapper work.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/32838
#![feature(allocator_api)]
// SAFETY(qix-): Needed for the global table to initialize arrays of null pointers
// SAFETY(qix-): safely, in order not to assume that `null == 0` (which is true on
// SAFETY(qix-): most platforms, but is not specified anywhere). Technically we could
// SAFETY(qix-): eschew this given that _we're the ones making a platform_ but it's still
// SAFETY(qix-): a good idea to be explicit.
#![feature(maybe_uninit_uninit_array)]
// SAFETY(qix-): To be stabilized soon. Needed for the global table.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/96097
#![feature(maybe_uninit_array_assume_init)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

pub mod arch;
pub mod iface;
pub mod instance;
pub mod interface;
pub mod module;
pub mod ring;
pub mod scheduler;
pub mod sync;
pub mod syscall;
pub mod tab;
pub mod table;
pub mod thread;

use core::mem::MaybeUninit;

use interface::RingInterface;
use oro_macro::assert;
use oro_mem::{
	alloc::vec::Vec,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace as _, MapError},
	pfa::Alloc,
};
use oro_sync::{Lock, TicketMutex};

use self::{arch::Arch, scheduler::Scheduler};

/// Core-local instance of the Oro kernel.
///
/// This object's constructor sets up a core-local
/// mapping of itself such that it can be accessed
/// from anywhere in the kernel as a static reference.
///
/// # Safety
/// **The fields of this structure are NOT accessible
/// between cores.** Taking references to fields of this structure
/// and passing them between cores is **undefined behavior**.
///
/// The generic type `A` **must** be the same type across
/// all cores in the system, else undefined behavior WILL occur.
pub struct Kernel<A: Arch> {
	/// The core's ID.
	id:        u32,
	/// Global reference to the shared kernel state.
	state:     &'static KernelState<A>,
	/// The kernel scheduler.
	///
	/// Guaranteed valid after a successful call to `initialize_for_core`.
	scheduler: MaybeUninit<TicketMutex<Scheduler<A>>>,
	/// Cached mapper handle for the kernel.
	mapper:    SupervisorHandle<A>,
	/// Core-local, architecture-specific handle.
	handle:    A::CoreHandle,
}

impl<A: Arch> Kernel<A> {
	/// Initializes a new core-local instance of the Oro kernel.
	///
	/// The [`oro_mem::mapper::AddressSpace::kernel_core_local()`] segment must
	/// be empty prior to calling this function, else it will
	/// return [`MapError::Exists`].
	///
	/// # Safety
	/// Must only be called once per CPU session (i.e.
	/// boot or bringup after a powerdown case, where the
	/// previous core-local [`Kernel`] was migrated or otherwise
	/// destroyed).
	///
	/// The `state` given to the kernel must be shared for all
	/// instances of the kernel that wish to partake in the same
	/// Oro kernel universe.
	///
	/// This function will fetch and store the current supervisor
	/// address space mapper handle for the kernel to use. It must
	/// be the final one that will be used for the lifetime of the core.
	pub unsafe fn initialize_for_core(
		id: u32,
		global_state: &'static KernelState<A>,
		handle: A::CoreHandle,
	) -> Result<&'static Self, MapError> {
		assert::fits::<Self, 4096>();

		let mapper = AddressSpace::<A>::current_supervisor_space();
		let core_local_segment = AddressSpace::<A>::kernel_core_local();

		let kernel_base = core_local_segment.range().0;
		debug_assert!((kernel_base as *mut Self).is_aligned());

		{
			let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
			core_local_segment.map(&mapper, kernel_base, phys)?;
		}

		let kernel_ptr = kernel_base as *mut Self;
		kernel_ptr.write(Self {
			id,
			handle,
			state: global_state,
			scheduler: MaybeUninit::uninit(),
			mapper,
		});

		// SAFETY(qix-): Now that the kernel has been mapped in, we can initialize the _real_
		// SAFETY(qix-): core-local ID function. This is effectively a no-op for secondary
		// SAFETY(qix-): cores, but doing it here ensures that any stray usage of `ReentrantLock`s
		// SAFETY(qix-): at least see _some_ core ID. This isn't ideal, it's a bit of a hack, but
		// SAFETY(qix-): it's a one-off situation that isn't trivial to avoid.
		sync::initialize_kernel_id_fn::<A>();

		(*kernel_ptr)
			.scheduler
			.write(TicketMutex::new(Scheduler::new(&*kernel_ptr)));

		Ok(&*kernel_ptr)
	}

	/// Returns a reference to the core-local kernel instance.
	///
	/// # Assumed Safety
	/// This function is not marked `unsafe` since, under pretty much
	/// every circumstance, the kernel instance should be initialized
	/// for the core before any other code runs. If this function is
	/// called before the kernel is initialized, undefined behavior
	/// will occur.
	///
	/// Architectures **must** make sure [`Self::initialize_for_core()`]
	/// has been called as soon as possible after the core boots.
	#[must_use]
	pub fn get() -> &'static Self {
		// SAFETY(qix-): The kernel instance is initialized for the core
		// SAFETY(qix-): before any other code runs.
		unsafe { &*(AddressSpace::<A>::kernel_core_local().range().0 as *const Self) }
	}

	/// Returns the core's ID.
	#[must_use]
	pub fn id(&self) -> u32 {
		self.id
	}

	/// Returns the underlying [`KernelState`] for this kernel instance.
	#[must_use]
	pub fn state(&self) -> &'static KernelState<A> {
		self.state
	}

	/// Returns the architecture-specific core local handle reference.
	#[must_use]
	pub fn handle(&self) -> &A::CoreHandle {
		&self.handle
	}

	/// Returns the architecture-specific core local handle reference.
	#[must_use]
	pub fn handle_mut(&mut self) -> &mut A::CoreHandle {
		&mut self.handle
	}

	/// Returns the mapper for the kernel.
	#[must_use]
	pub fn mapper(&self) -> &SupervisorHandle<A> {
		&self.mapper
	}

	/// Gets a reference to the scheduler.
	///
	/// # Safety
	/// Before locking the scheduler, the caller must ensure that
	/// interrupts are disabled; the spinlock is _not_ a critical
	/// spinlock and thus does not disable interrupts.
	#[must_use]
	pub unsafe fn scheduler(&self) -> &TicketMutex<Scheduler<A>> {
		self.scheduler.assume_init_ref()
	}
}

/// Global state shared by all [`Kernel`] instances across
/// core boot/powerdown/bringup cycles.
pub struct KernelState<A: Arch> {
	/// List of all threads.
	threads:   TicketMutex<Vec<tab::Tab<thread::Thread<A>>>>,
	/// The root ring.
	root_ring: tab::Tab<ring::Ring<A>>,
}

impl<A: Arch> KernelState<A> {
	/// Creates a new instance of the kernel state. Meant to be called
	/// once for all cores at boot time.
	///
	/// # Safety
	/// This function must ONLY be called by the primary core,
	/// only at boot time, and only before any other cores are brought up,
	/// exactly once.
	///
	/// This function sets up shared page table mappings that MUST be
	/// shared across cores. The caller MUST initialize the kernel
	/// state (this struct) prior to booting _any other cores_
	/// or else registry accesses will page fault.
	#[allow(clippy::missing_panics_doc)]
	pub unsafe fn init(this: &'static mut MaybeUninit<Self>) -> Result<(), MapError> {
		// SAFETY(qix-): Must be first, before anything else happens in the kernel.
		self::sync::install_dummy_kernel_id_fn();

		let root_ring = ring::Ring::<A>::new_root()?;

		// Install root ring interfaces.
		{
			let ifaceid = tab::get()
				.add(RingInterface::<A>::new(
					self::iface::root_ring::debug_out_v0::DebugOutV0::new(),
					root_ring.id(),
				))
				.expect("out of memory");

			// XXX(qix-): DEBUG
			::oro_debug::dbg!("registered DebugLogV0: {}", ifaceid.id());

			root_ring.with_mut(|root_ring| {
				root_ring.interfaces_mut().push(ifaceid);
			});

			#[cfg(feature = "boot-vbuf-v0")]
			{
				let ifaceid = tab::get()
					.add(RingInterface::<A>::new(
						self::iface::root_ring::boot_vbuf_v0::BootVbufV0::new(),
						root_ring.id(),
					))
					.expect("out of memory");

				// XXX(qix-): DEBUG
				::oro_debug::dbg!("registered BootVbufV0: {}", ifaceid.id());

				root_ring.with_mut(|root_ring| {
					root_ring.interfaces_mut().push(ifaceid);
				});
			}
		}

		this.write(Self {
			root_ring,
			threads: TicketMutex::default(),
		});

		Ok(())
	}

	/// Returns a handle to the root ring.
	pub fn root_ring(&self) -> &tab::Tab<ring::Ring<A>> {
		&self.root_ring
	}

	/// Returns a reference to the mutex-guarded list of threads.
	pub fn threads(&self) -> &impl Lock<Target = Vec<tab::Tab<thread::Thread<A>>>> {
		&self.threads
	}
}

/// Helper trait association type for `Arch::AddrSpace`.
pub(crate) type AddressSpace<A> = <A as Arch>::AddressSpace;
/// Helper trait association type for `Arch::AddrSpace::SupervisorHandle`.
pub(crate) type SupervisorHandle<A> =
	<AddressSpace<A> as oro_mem::mapper::AddressSpace>::SupervisorHandle;
/// Helper trait association type for `Arch::AddrSpace::UserHandle`.
pub(crate) type UserHandle<A> = <AddressSpace<A> as oro_mem::mapper::AddressSpace>::UserHandle;
