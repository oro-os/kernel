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

pub mod instance;
pub mod module;
pub mod port;
pub mod ring;
pub mod scheduler;
pub mod thread;

use core::{
	mem::MaybeUninit,
	sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use oro_debug::dbg_err;
use oro_id::{Id, IdType};
use oro_macro::assert;
use oro_mem::{
	alloc::{
		sync::{Arc, Weak},
		vec,
		vec::Vec,
	},
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::Alloc,
};
use oro_sync::{Lock, Mutex, TicketMutex};

use self::scheduler::Scheduler;

/// Core-local instance of the Oro kernel.
///
/// This object's constructor sets up a core-local
/// mapping of itself such that it can be accessed
/// from anywhere in the kernel as a static reference.
pub struct Kernel<A: Arch> {
	/// The core's ID.
	id:         usize,
	/// Local core state. The kernel instance owns this
	/// due to all of the machinery already in place to make
	/// this kernel instance object core-local and accessible
	/// from anywhere in the kernel.
	core_state: A::CoreState,
	/// Global reference to the shared kernel state.
	state:      &'static KernelState<A>,
	/// The kernel scheduler.
	///
	/// Guaranteed valid after a successful call to `initialize_for_core`.
	scheduler:  MaybeUninit<TicketMutex<Scheduler<A>>>,
	/// Cached mapper handle for the kernel.
	mapper:     SupervisorHandle<A>,
}

impl<A: Arch> Kernel<A> {
	/// Initializes a new core-local instance of the Oro kernel.
	///
	/// The [`AddressSpace::kernel_core_local()`] segment must
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
		id: usize,
		global_state: &'static KernelState<A>,
		core_state: A::CoreState,
	) -> Result<&'static Self, MapError> {
		assert::fits::<Self, 4096>();

		let mapper = AddrSpace::<A>::current_supervisor_space();
		let core_local_segment = AddrSpace::<A>::kernel_core_local();

		let kernel_base = core_local_segment.range().0;
		debug_assert!((kernel_base as *mut Self).is_aligned());

		{
			let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
			core_local_segment.map(&mapper, kernel_base, phys)?;
		}

		let kernel_ptr = kernel_base as *mut Self;
		kernel_ptr.write(Self {
			id,
			core_state,
			state: global_state,
			scheduler: MaybeUninit::uninit(),
			mapper,
		});

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
		unsafe { &*(AddrSpace::<A>::kernel_core_local().range().0 as *const Self) }
	}

	/// Returns the core's ID.
	#[must_use]
	pub fn id(&self) -> usize {
		self.id
	}

	/// Returns the underlying [`KernelState`] for this kernel instance.
	#[must_use]
	pub fn state(&self) -> &'static KernelState<A> {
		self.state
	}

	/// Returns the architecture-specific core local state reference.
	#[must_use]
	pub fn core(&self) -> &A::CoreState {
		&self.core_state
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
	/// The base userspace address space mapper.
	///
	/// This is a clone of the supervisor space mapper handle
	/// devoid of any userspace mappings, as well as all kernel-local
	/// mappings removed.
	///
	/// It serves as the base for all userspace mappers.
	user_mapper: UserHandle<A>,

	/// List of all modules.
	modules:   TicketMutex<Vec<Weak<Mutex<module::Module<A>>>>>,
	/// List of all rings.
	rings:     TicketMutex<Vec<Weak<Mutex<ring::Ring<A>>>>>,
	/// List of all instances.
	instances: TicketMutex<Vec<Weak<Mutex<instance::Instance<A>>>>>,
	/// List of all threads.
	threads:   TicketMutex<Vec<Weak<Mutex<thread::Thread<A>>>>>,

	/// The root ring.
	root_ring: Arc<Mutex<ring::Ring<A>>>,

	/// The ID counter for resource allocation.
	id_counter: AtomicUsize,
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
		let supervisor_space = AddrSpace::<A>::current_supervisor_space();
		let user_mapper =
			AddrSpace::<A>::new_user_space(&supervisor_space).ok_or(MapError::OutOfMemory)?;

		let root_ring = Arc::new(Mutex::new(ring::Ring::<A> {
			id:        0,
			parent:    None,
			instances: Vec::new(),
		}));

		this.write(Self {
			user_mapper,
			root_ring: root_ring.clone(),
			modules: TicketMutex::default(),
			rings: TicketMutex::new(vec![Arc::downgrade(&root_ring)]),
			instances: TicketMutex::default(),
			threads: TicketMutex::default(),
			id_counter: AtomicUsize::new(0),
		});

		let this = this.assume_init_mut();

		// Sanity check
		debug_assert_eq!(this.root_ring.lock().id(), 0, "root ring ID must be 0");
		debug_assert_eq!(this.allocate_id(), 0, "first allocated ID must be 0");

		Ok(())
	}

	/// Returns a handle to the root ring.
	pub fn root_ring(&'static self) -> Arc<Mutex<ring::Ring<A>>> {
		self.root_ring.clone()
	}

	/// Returns a reference to the mutex-guarded list of threads.
	pub fn threads(
		&'static self,
	) -> &'static impl Lock<Target = Vec<Weak<Mutex<thread::Thread<A>>>>> {
		&self.threads
	}

	/// Allocates a new resource ID.
	fn allocate_id(&self) -> usize {
		self.id_counter.fetch_add(1, Relaxed)
	}

	/// Creates a new ring and returns a handle to it.
	pub fn create_ring(
		&'static self,
		parent: Arc<Mutex<ring::Ring<A>>>,
	) -> Arc<Mutex<ring::Ring<A>>> {
		let ring = Arc::new(Mutex::new(ring::Ring::<A> {
			id:        self.allocate_id(),
			parent:    Some(parent),
			instances: Vec::new(),
		}));

		debug_assert_ne!(ring.lock().id(), 0, "ring ID must not be 0");

		self.rings.lock().push(Arc::downgrade(&ring));

		ring
	}

	/// Creates a new module and returns a [`Handle`] to it.
	pub fn create_module(
		&'static self,
		id: Id<{ IdType::Module }>,
	) -> Result<Arc<Mutex<module::Module<A>>>, MapError> {
		let mapper = {
			AddrSpace::<A>::duplicate_user_space_shallow(&self.user_mapper)
				.ok_or(MapError::OutOfMemory)?
		};

		let module = Arc::new(Mutex::new(module::Module::<A> {
			id: self.allocate_id(),
			module_id: id,
			instances: Vec::new(),
			mapper,
		}));

		debug_assert_ne!(module.lock().id(), 0, "module ID must not be 0");

		self.modules.lock().push(Arc::downgrade(&module));

		Ok(module)
	}

	/// Creates a new instance and returns a [`Handle`] to it.
	#[expect(clippy::needless_pass_by_value)]
	pub fn create_instance(
		&'static self,
		module: Arc<Mutex<module::Module<A>>>,
		ring: Arc<Mutex<ring::Ring<A>>>,
	) -> Result<Arc<Mutex<instance::Instance<A>>>, MapError> {
		let mapper = {
			let module_lock = module.lock();
			AddrSpace::<A>::duplicate_user_space_shallow(module_lock.mapper())
				.ok_or(MapError::OutOfMemory)?
		};

		let instance = Arc::new(Mutex::new(instance::Instance {
			id: self.allocate_id(),
			module: module.clone(),
			ring: ring.clone(),
			threads: Vec::new(),
			ports: Vec::new(),
			mapper,
		}));

		debug_assert_ne!(instance.lock().id(), 0, "instance ID must not be 0");

		self.instances.lock().push(Arc::downgrade(&instance));
		module.lock().instances.push(instance.clone());
		ring.lock().instances.push(instance.clone());

		Ok(instance)
	}

	/// Creates a new thread and returns a [`Handle`] to it.
	///
	/// `stack_size` is rounded up to the next page size.
	#[expect(clippy::missing_panics_doc, clippy::needless_pass_by_value)]
	pub fn create_thread(
		&'static self,
		instance: Arc<Mutex<instance::Instance<A>>>,
		stack_size: usize,
		mut thread_state: A::ThreadState,
	) -> Result<Arc<Mutex<thread::Thread<A>>>, MapError> {
		let mapper = {
			let instance_lock = instance.lock();
			AddrSpace::<A>::duplicate_user_space_shallow(instance_lock.mapper())
				.ok_or(MapError::OutOfMemory)?
		};

		// Map the stack for the thread.
		// SAFETY(qix-): We don't panic here.
		let thread = unsafe {
			// Map a stack for the thread.
			let stack_segment = AddrSpace::<A>::module_thread_stack();
			let stack_size = (stack_size + 0xFFF) & !0xFFF;

			let stack_high_guard = stack_segment.range().1 & !0xFFF;
			let stack_first_page = stack_high_guard - stack_size;
			#[cfg(debug_assertions)]
			let stack_low_guard = stack_first_page - 0x1000;

			let map_result = {
				for virt in (stack_first_page..stack_high_guard).step_by(0x1000) {
					let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
					stack_segment.map(&mapper, virt, phys)?;
				}

				// Make sure the guard pages are unmapped.
				// This is more of an assertion to make sure the thread
				// state is not in a bad state, but should never be the
				// case outside of bugs.
				#[cfg(debug_assertions)]
				{
					match stack_segment.unmap(&mapper, stack_low_guard) {
						Ok(phys) => {
							panic!("a module's thread stack low guard page was mapped: {phys:016X}")
						}
						Err(UnmapError::NotMapped) => {}
						Err(e) => {
							panic!("failed to unmap a module's thread stack low guard page: {e:?}")
						}
					}

					match stack_segment.unmap(&mapper, stack_high_guard) {
						Ok(phys) => {
							panic!(
								"a module's thread stack high guard page was mapped: {phys:016X}"
							)
						}
						Err(UnmapError::NotMapped) => {}
						Err(e) => {
							panic!("failed to unmap a module's thread stack high guard page: {e:?}")
						}
					}
				}

				// Let the architecture do any additional stack setup.
				A::initialize_thread_mappings(&mapper, &mut thread_state)?;

				let thread = Arc::new(Mutex::new(thread::Thread::<A> {
					id: self.allocate_id(),
					instance: instance.clone(),
					// We set it in a moment.
					mapper: MaybeUninit::uninit(),
					// We set it in a moment.
					thread_state: MaybeUninit::uninit(),
					run_on_id: None,
					running_on_id: None,
				}));

				debug_assert_ne!(thread.lock().id(), 0, "thread ID must not be 0");

				instance.lock().threads.push(thread.clone());
				self.threads.lock().push(Arc::downgrade(&thread));

				Ok(thread)
			};

			// Try to reclaim the memory we just allocated, if any.
			match map_result {
				Err(err) => {
					if let Err(err) = A::reclaim_thread_mappings(&mapper, &mut thread_state) {
						dbg_err!(
							"failed to reclaim architecture thread mappings - MEMORY MAY LEAK: \
							 {err:?}"
						);
					}

					if let Err(err) = stack_segment.unmap_all_and_reclaim(&mapper) {
						dbg_err!("failed to reclaim thread stack - MEMORY MAY LEAK: {err:?}");
					}

					AddrSpace::<A>::free_user_space(mapper);

					return Err(err);
				}
				Ok(thread) => thread,
			}
		};

		{
			let mut thread_lock = thread.lock();
			thread_lock.mapper.write(mapper);
			thread_lock.thread_state.write(thread_state);
		}

		Ok(thread)
	}
}

/// A trait for architectures to list commonly used types
/// to be passed around the kernel.
pub trait Arch: 'static {
	/// The address space layout the architecture uses.
	type AddrSpace: AddressSpace;
	/// Architecture-specific thread state to be stored alongside
	/// each thread.
	type ThreadState: Sized + Send = ();
	/// The core-local state type.
	type CoreState: Sized + Send + Sync + 'static = ();

	/// Allows the architecture to further initialize an instance
	/// thread's mappings when threads are created.
	///
	/// This guarantees that, no matter from where the thread is
	/// created, the thread's address space will be initialized
	/// correctly for the architecture.
	fn initialize_thread_mappings(
		_thread: &<Self::AddrSpace as AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
	) -> Result<(), MapError> {
		Ok(())
	}

	/// Allows the architecture to further reclaim any memory
	/// associated with a thread when it is destroyed.
	///
	/// This method only reclaim memory that was allocated in
	/// [`Self::initialize_thread_mappings()`].
	///
	/// Further, it should _not_ expect the mappings to be present
	/// all the time; it may be called if allocation fails during
	/// thread creation, causing a fragmented thread map.
	fn reclaim_thread_mappings(
		_thread: &<Self::AddrSpace as AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
	) -> Result<(), UnmapError> {
		Ok(())
	}
}

/// Helper trait association type for `Arch::AddrSpace`.
pub(crate) type AddrSpace<A> = <A as Arch>::AddrSpace;
/// Helper trait association type for `Arch::AddrSpace::SupervisorHandle`.
pub(crate) type SupervisorHandle<A> = <AddrSpace<A> as AddressSpace>::SupervisorHandle;
/// Helper trait association type for `Arch::AddrSpace::UserHandle`.
pub(crate) type UserHandle<A> = <AddrSpace<A> as AddressSpace>::UserHandle;
