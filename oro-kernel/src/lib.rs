//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate is a library with the core kernel functionality, datatypes,
//! etc. and provides a common interface for architectures to implement
//! the Oro kernel on their respective platforms.
#![no_std]
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
// SAFETY(qix-): This is either going to be stabilized, or the workaround
// SAFETY(qix-): for it to be pulled will have a trivial workaround that
// SAFETY(qix-): has equally good codegen. Either, way this is zero risk.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/90850
#![feature(downcast_unchecked)]
// SAFETY(qix-): This is almost stabilized.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/70142
#![feature(result_flattening)]

pub mod arch;
pub mod event;
pub mod hash;
pub mod iface;
pub mod instance;
pub mod interface;
pub mod module;
pub mod port;
pub mod ring;
pub mod scheduler;
pub mod sync;
pub mod syscall;
pub mod tab;
pub mod table;
pub mod thread;
pub mod token;

use core::{
	cell::UnsafeCell,
	mem::MaybeUninit,
	sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arch::{Arch, CoreHandle};
use event::Resumption;
use interface::RingInterface;
use nolock::queues::{
	DequeueError,
	mpmc::bounded::scq::{Receiver, Sender},
};
use oro_macro::assert;
use oro_mem::{
	alloc::boxed::Box,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace as _, MapError},
	pfa::Alloc,
};
use scheduler::Scheduler;
use tab::Tab;
use thread::Thread;

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
	id:           u32,
	/// Global reference to the shared kernel state.
	global_state: &'static GlobalKernelState<A>,
	/// Cached mapper handle for the kernel.
	mapper:       SupervisorHandle<A>,
	/// Core-local, architecture-specific handle.
	handle:       A::CoreHandle,
	/// The kernel scheduler.
	///
	/// Guaranteed valid after a successful call to `initialize_for_core`.
	scheduler:    MaybeUninit<Tab<Scheduler<A>>>,
}

impl<A: Arch> Kernel<A> {
	/// Returns the core's ID.
	#[must_use]
	pub fn id(&self) -> u32 {
		self.id
	}

	/// Initializes a new core-local instance of the Oro kernel.
	///
	/// The [`oro_mem::mapper::AddressSpace::kernel_core_local()`] segment must
	/// be empty prior to calling this function, else it will
	/// return [`MapError::Exists`].
	///
	/// # Panics
	/// Panics if the system runs out of memory.
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
		global_state: &'static GlobalKernelState<A>,
		handle: A::CoreHandle,
	) -> Result<&'static Self, MapError> {
		assert::fits::<Self, 4096>();

		// SAFETY: Safety requirements about exclusive access are offloaded to the caller.
		let mapper = unsafe { AddressSpace::<A>::current_supervisor_space() };
		let core_local_segment = AddressSpace::<A>::kernel_core_local();

		let kernel_base = core_local_segment.range().0;
		debug_assert!((kernel_base as *mut Self).is_aligned());

		{
			let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
			core_local_segment.map(&mapper, kernel_base, phys)?;
		}

		let kernel_ptr = kernel_base as *mut Self;
		// SAFETY: We've just mapped in the kernel instance, and have debug-checked it's aligned.
		unsafe {
			kernel_ptr.write(Self {
				id,
				handle,
				global_state,
				scheduler: MaybeUninit::uninit(),
				mapper,
			});
		}

		// SAFETY: Now that the kernel has been mapped in, we can initialize the _real_
		// SAFETY: core-local ID function. This is effectively a no-op for secondary
		// SAFETY: cores, but doing it here ensures that any stray usage of `ReentrantLock`s
		// SAFETY: at least see _some_ core ID. This isn't ideal, it's a bit of a hack, but
		// SAFETY: it's a one-off situation that isn't trivial to avoid.
		unsafe {
			sync::initialize_kernel_id_fn::<A>();
		}

		// SAFETY: We've just written the kernel instance to the core-local segment; it's safe to read.
		unsafe {
			(*kernel_ptr).scheduler.write(
				tab::get()
					.add(Scheduler::new(&*kernel_ptr))
					.expect("failed to allocate scheduler; out of memory"),
			);
		}

		if !global_state.has_initialized_root.swap(true, SeqCst) {
			global_state.root_ring.with_mut(|root_ring| {
				root_ring
					.register_interface(RingInterface::<A>::new(
						iface::root_ring::debug_out_v0::DebugOutV0::new(),
						global_state.root_ring.id(),
					))
					.ok_or(MapError::OutOfMemory)
			})?;

			global_state.root_ring.with_mut(|root_ring| {
				root_ring
					.register_interface(RingInterface::<A>::new(
						iface::root_ring::test_ports::RootTestPorts::new(),
						global_state.root_ring.id(),
					))
					.ok_or(MapError::OutOfMemory)
			})?;

			#[cfg(feature = "boot-vbuf-v0")]
			{
				global_state.root_ring.with_mut(|root_ring| {
					root_ring
						.register_interface(RingInterface::<A>::new(
							iface::root_ring::boot_vbuf_v0::BootVbufV0::new(),
							global_state.root_ring.id(),
						))
						.ok_or(MapError::OutOfMemory)
				})?;
			}
		}

		// SAFETY: We just wrote the kernel instance to the core-local segment; it's safe to deref.
		unsafe { Ok(&*kernel_ptr) }
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
	/// Architectures **must** make sure [`Kernel::initialize_for_core()`]
	/// has been called as soon as possible after the core boots.
	#[must_use]
	pub fn get() -> &'static Self {
		// SAFETY(qix-): The kernel instance is initialized for the core
		// SAFETY(qix-): before any other code runs.
		unsafe { &*(AddressSpace::<A>::kernel_core_local().range().0 as *const Self) }
	}

	/// Returns the underlying [`GlobalKernelState`] for this kernel instance.
	#[must_use]
	pub fn global_state(&self) -> &'static GlobalKernelState<A> {
		self.global_state
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

	/// Runs the kernel's main loop.
	///
	/// This function is the main entry point for the kernel.
	/// The architecture essentially gives up primary control
	/// to the architecture-agnostic Oro kernel, only being
	/// called back through the [`arch`] handles.
	///
	/// # Safety
	/// > ⚠️**THIS FUNCTION IS VERY UNSAFE.**⚠️
	///
	/// Callers must be aware that their ENTIRE stack space will be
	/// blown away. Absolutely NO stack items may be "live", to be
	/// used later; all kernel state MUST be stored in non-stack
	/// memory segments. **Absolutely no exceptions.**
	pub unsafe fn run(&self) -> ! {
		// Immediately perform a kernel halt with the smallest
		// timeslice to immediately invoke a scheduler run.
		// SAFETY: Calling with a `None` context is always safe, barring bugs in the
		// SAFETY: architecture implementation.
		unsafe {
			self.handle.run_context(None, Some(1), None);
		}
		#[expect(unreachable_code)]
		{
			unreachable!("architecture returned from context switch! this is a bug!");
		}
	}

	/// Gets a reference to the scheduler.
	///
	/// # Safety
	/// Before locking the scheduler, the caller must ensure that
	/// interrupts are disabled; the spinlock is _not_ a critical
	/// spinlock and thus does not disable interrupts.
	#[must_use]
	pub unsafe fn scheduler(&self) -> &Tab<Scheduler<A>> {
		// SAFETY: Always valid if we have a valid `self` reference.
		unsafe { self.scheduler.assume_init_ref() }
	}

	/// Handles a preemption event.
	///
	/// # Safety
	/// - The caller must ensure that the stack is restored to the top of the
	///   kernel stack segment ([`oro_mem::mapper::AddressSpace::kernel_stack()`]).
	/// - Interrupts must be disabled. Any NMI handlers must be primed to dump core
	///   and kernel panic; **this function is non-reentrant**.
	/// - This function must only be called **exactly once** per context switch.
	/// - The caller must ensure that any thread state is properly updated prior
	///   to calling this function.
	/// - The caller must ensure that the preemption event pertains to the currently
	///   running context. This function **must not** be called with any value other
	///   than [`event::PreemptionEvent::Timer`] if the context was `None`.
	#[expect(clippy::needless_pass_by_value)]
	pub unsafe fn handle_event(&self, event: event::PreemptionEvent) -> ! {
		// TODO(qix-): The scheduler is going to go away at some point,
		// TODO(qix-): at least in its current form. This is temporary.
		// SAFETY: The kernel is initialized if we're inside this method.
		unsafe {
			let (ctx, ticks, resumption) = self.scheduler().with_mut(|sched| {
				let switch = match &event {
					event::PreemptionEvent::Timer => sched.event_timer_expired(),
					event::PreemptionEvent::PageFault(pf) => {
						sched.event_page_fault(
							match pf.access {
								event::PageFaultAccess::Execute => {
									scheduler::PageFaultType::Execute
								}
								event::PageFaultAccess::Write => scheduler::PageFaultType::Write,
								event::PageFaultAccess::Read => scheduler::PageFaultType::Read,
							},
							pf.address,
						)
					}
					event::PreemptionEvent::SystemCall(req) => sched.event_system_call(req),
					unknown => {
						todo!("unknown incoming preemption event: {unknown:?}");
					}
				};

				match switch {
					scheduler::Switch::KernelResume | scheduler::Switch::UserToKernel => {
						(None, Some(1000), None)
					}
					scheduler::Switch::KernelToUser(thr, sys)
					| scheduler::Switch::UserResume(thr, sys)
					| scheduler::Switch::UserToUser(thr, sys) => {
						let sys = sys.map(Resumption::SystemCall);

						// SAFETY: This isn't. It's highly unsafe. But should work for now.
						let handle = &*thr
							.with(|t| core::ptr::from_ref(t.handle()))
							.cast::<UnsafeCell<_>>();

						(Some(handle), Some(1000), sys)
					}
				}
			});

			self.handle.run_context(ctx, ticks, resumption);
		}
	}
}

/// Global state shared by all [`Kernel`] instances across
/// core boot/powerdown/bringup cycles.
pub struct GlobalKernelState<A: Arch> {
	/// Unclaimed thread deque sender.
	thread_tx: Sender<Tab<Thread<A>>>,
	/// Unclaimed thread deque receiver.
	thread_rx: Receiver<Tab<Thread<A>>>,
	/// The root ring.
	root_ring: Tab<ring::Ring<A>>,
	/// Kernel interfaces, made globall available.
	kernel_interfaces: table::Table<Box<dyn iface::kernel::KernelInterface<A>>>,
	/// Whether or not the root ring has been initialized.
	///
	/// We have to do this on a per-core basis because allocators
	/// and the local core mappings haven't been set up at the time
	/// the global kernel state is initialized.
	has_initialized_root: AtomicBool,
}

impl<A: Arch> GlobalKernelState<A> {
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
		// SAFETY: Must be first, before anything else happens in the kernel.
		unsafe {
			sync::install_dummy_kernel_id_fn();
		}

		// SAFETY: We've offloaded the requirement of being called once for the entire
		// SAFETY: kernel lifetime to the caller.
		let root_ring = unsafe { ring::Ring::<A>::new_root()? };

		let (thread_rx, thread_tx) = nolock::queues::mpmc::bounded::scq::queue(128);

		let mut kernel_interfaces = table::Table::new();
		iface::kernel::register_kernel_interfaces(&mut kernel_interfaces);
		A::register_kernel_interfaces(&mut kernel_interfaces);

		this.write(Self {
			thread_tx,
			thread_rx,
			root_ring,
			kernel_interfaces,
			has_initialized_root: AtomicBool::new(false),
		});

		Ok(())
	}

	/// Returns a handle to the root ring.
	pub fn root_ring(&self) -> &Tab<ring::Ring<A>> {
		&self.root_ring
	}

	/// Submits a thread to the kernel state to be claimed by
	/// the next 'free' scheduler.
	///
	/// # Panics
	/// This function panics if the unclaimed thread queue is full.
	///
	/// For now, this is acceptable, but will be relaxed in the future
	/// as the scheduler is fleshed out a bit more.
	pub fn submit_unclaimed_thread(&self, thread: Tab<Thread<A>>) {
		// Tell the thread it's been deallocated.
		unsafe {
			Thread::<A>::deallocate(&thread);
		}

		if let Err((err, _)) = self.thread_tx.try_enqueue(thread) {
			panic!("thread queue full or disconnected: {err:?}")
		}
	}

	/// Tries to take the next unclaimed thread.
	#[expect(clippy::missing_panics_doc)]
	pub fn try_claim_thread(&self) -> Option<Tab<Thread<A>>> {
		match self.thread_rx.try_dequeue() {
			Ok(thread) => Some(thread),
			Err(DequeueError::Closed) => {
				// NOTE(qix-): Should never happen.
				panic!("thread queue disconnected");
			}
			Err(DequeueError::Empty) => None,
		}
	}
}

/// Helper trait association type for `Arch::AddrSpace`.
pub(crate) type AddressSpace<A> = <A as Arch>::AddressSpace;
/// Helper trait association type for `Arch::AddrSpace::SupervisorHandle`.
pub(crate) type SupervisorHandle<A> =
	<AddressSpace<A> as oro_mem::mapper::AddressSpace>::SupervisorHandle;
/// Helper trait association type for `Arch::AddrSpace::UserHandle`.
pub(crate) type UserHandle<A> = <AddressSpace<A> as oro_mem::mapper::AddressSpace>::UserHandle;
