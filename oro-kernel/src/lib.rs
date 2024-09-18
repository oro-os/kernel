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
pub mod registry;
pub mod ring;
pub mod scheduler;
pub mod thread;

use self::{
	registry::{Handle, List, ListRegistry, Registry},
	scheduler::Scheduler,
};
use core::mem::MaybeUninit;
use oro_debug::dbg_err;
use oro_id::{Id, IdType};
use oro_macro::assert;
use oro_mem::{
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::alloc::Alloc,
	translate::Translator,
};
use oro_sync::spinlock::{
	unfair::UnfairSpinlock,
	unfair_critical::{InterruptController, UnfairCriticalSpinlock},
};

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
	scheduler:  MaybeUninit<UnfairSpinlock<Scheduler<A>>>,
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

		let mapper = AddrSpace::<A>::current_supervisor_space(&global_state.pat);
		let core_local_segment = AddrSpace::<A>::kernel_core_local();

		let kernel_base = core_local_segment.range().0;
		debug_assert!((kernel_base as *mut Self).is_aligned());

		{
			let mut pfa = global_state.pfa.lock::<A::IntCtrl>();
			let phys = pfa.allocate().ok_or(MapError::OutOfMemory)?;
			core_local_segment.map(&mapper, &mut *pfa, &global_state.pat, kernel_base, phys)?;
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
			.write(UnfairSpinlock::new(Scheduler::new(&*kernel_ptr)));

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
	pub unsafe fn scheduler(&self) -> &UnfairSpinlock<Scheduler<A>> {
		#[cfg(debug_assertions)]
		debug_assert!(!A::IntCtrl::interrupts_enabled());
		self.scheduler.assume_init_ref()
	}
}

/// Global state shared by all [`Kernel`] instances across
/// core boot/powerdown/bringup cycles.
pub struct KernelState<A: Arch> {
	/// The shared, spinlocked page frame allocator (PFA) for the
	/// entire system.
	pfa: UnfairCriticalSpinlock<A::Pfa>,
	/// The physical address translator.
	pat: A::Pat,

	/// The base userspace address space mapper.
	///
	/// This is a clone of the supervisor space mapper handle
	/// devoid of any userspace mappings, as well as all kernel-local
	/// mappings removed.
	///
	/// It serves as the base for all userspace mappers.
	user_mapper: UserHandle<A>,

	/// List of all modules.
	///
	/// Always `Some` after a valid initialization.
	/// Can be safely `.unwrap()`'d in most situations.
	modules:   Option<Handle<List<module::Module<A>, A>>>,
	/// List of all rings.
	///
	/// Always `Some` after a valid initialization.
	/// Can be safely `.unwrap()`'d in most situations.
	rings:     Option<Handle<List<ring::Ring<A>, A>>>,
	/// List of all instances.
	///
	/// Always `Some` after a valid initialization.
	/// Can be safely `.unwrap()`'d in most situations.
	instances: Option<Handle<List<instance::Instance<A>, A>>>,
	/// List of all threads.
	///
	/// Always `Some` after a valid initialization.
	/// Can be safely `.unwrap()`'d in most situations.
	threads:   Option<Handle<List<thread::Thread<A>, A>>>,

	/// The root ring.
	///
	/// Always `Some` after a valid initialization.
	/// Can be safely `.unwrap()`'d in most situations.
	root_ring: Option<Handle<ring::Ring<A>>>,

	/// Ring registry.
	ring_registry:          Registry<ring::Ring<A>, A>,
	/// Ring list registry.
	ring_list_registry:     ListRegistry<ring::Ring<A>, A>,
	/// Module registry.
	module_registry:        Registry<module::Module<A>, A>,
	/// Module list registry.
	module_list_registry:   ListRegistry<module::Module<A>, A>,
	/// Instance registry.
	instance_registry:      Registry<instance::Instance<A>, A>,
	/// Instance list registry.
	instance_list_registry: ListRegistry<instance::Instance<A>, A>,
	/// Thread registry.
	thread_registry:        Registry<thread::Thread<A>, A>,
	/// Thread list registry.
	thread_list_registry:   ListRegistry<thread::Thread<A>, A>,
	/// Port registry.
	#[expect(dead_code)]
	port_registry:          Registry<port::Port, A>,
	/// Port list registry.
	port_list_registry:     ListRegistry<port::Port, A>,
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
	pub unsafe fn init(
		this: &'static mut MaybeUninit<Self>,
		pat: A::Pat,
		pfa: UnfairCriticalSpinlock<A::Pfa>,
	) -> Result<(), MapError> {
		#[expect(clippy::missing_docs_in_private_items)]
		macro_rules! init_registries {
			($($id:ident => $(($listregfn:ident, $itemregfn:ident))? $($regfn:ident)?),* $(,)?) => {
				$(
					let $id = {
						let mut pfa_lock = pfa.lock::<A::IntCtrl>();
						let reg = init_registries!(@inner pfa_lock, $(($listregfn, $itemregfn))? $($regfn)?);
						let _ = pfa_lock;

						reg
					};
				)*
			};

			(@inner $pfa_lock:expr, ( $listregfn:ident , $itemregfn:ident )) => {
				ListRegistry::new(
					pat.clone(),
					&mut *$pfa_lock,
					A::AddrSpace::$listregfn(),
					A::AddrSpace::$itemregfn(),
				)?
			};

			(@inner $pfa_lock:expr, $regfn:ident) => {
				Registry::new(
					pat.clone(),
					&mut *$pfa_lock,
					A::AddrSpace::$regfn(),
				)?
			};
		}

		init_registries! {
			ring_registry => kernel_ring_registry,
			ring_list_registry => (kernel_ring_list_registry, kernel_ring_item_registry),
			module_registry => kernel_module_registry,
			module_list_registry => (kernel_module_list_registry, kernel_module_item_registry),
			instance_registry => kernel_instance_registry,
			instance_list_registry => (kernel_instance_list_registry, kernel_instance_item_registry),
			thread_registry => kernel_thread_registry,
			thread_list_registry => (kernel_thread_list_registry, kernel_thread_item_registry),
			port_registry => kernel_port_registry,
			port_list_registry => (kernel_port_list_registry, kernel_port_item_registry),
		}

		let supervisor_space = AddrSpace::<A>::current_supervisor_space(&pat);
		let user_mapper =
			AddrSpace::<A>::new_user_space(&supervisor_space, &mut *pfa.lock::<A::IntCtrl>(), &pat)
				.ok_or(MapError::OutOfMemory)?;

		this.write(Self {
			pfa,
			pat,
			user_mapper,
			root_ring: None,
			modules: None,
			rings: None,
			instances: None,
			threads: None,
			ring_registry,
			ring_list_registry,
			module_registry,
			module_list_registry,
			instance_registry,
			instance_list_registry,
			thread_registry,
			thread_list_registry,
			port_registry,
			port_list_registry,
		});

		let this = this.assume_init_mut();

		let root_ring = this.ring_registry.insert(
			&this.pfa,
			ring::Ring {
				id:        0,
				parent:    None,
				instances: this.instance_list_registry.create_list(&this.pfa)?,
			},
		)?;
		assert_eq!(root_ring.id(), 0, "root ring ID must be 0");

		let modules = this.module_list_registry.create_list(&this.pfa)?;
		let rings = this.ring_list_registry.create_list(&this.pfa)?;
		let instances = this.instance_list_registry.create_list(&this.pfa)?;
		let threads = this.thread_list_registry.create_list(&this.pfa)?;

		let _ = rings.append(&this.pfa, root_ring.clone())?;

		this.root_ring = Some(root_ring);
		this.rings = Some(rings);
		this.modules = Some(modules);
		this.instances = Some(instances);
		this.threads = Some(threads);

		Ok(())
	}

	/// Returns the underlying PFA belonging to the kernel state.
	pub fn pfa(&'static self) -> &'static UnfairCriticalSpinlock<A::Pfa> {
		&self.pfa
	}

	/// Returns the underlying physical address translator belonging to the kernel state.
	pub fn pat(&'static self) -> &'static A::Pat {
		&self.pat
	}

	/// Returns a [`Handle`] to the root ring.
	#[expect(clippy::missing_panics_doc)]
	pub fn root_ring(&'static self) -> Handle<ring::Ring<A>> {
		// SAFETY(qix-): We always assume the root ring is initialized.
		self.root_ring.clone().unwrap()
	}

	/// Returns a [`Handle`] to the list of all threads.
	#[expect(clippy::missing_panics_doc)]
	pub fn threads(&'static self) -> Handle<List<thread::Thread<A>, A>> {
		// SAFETY(qix-): We always assume the threads list is initialized.
		self.threads.clone().unwrap()
	}

	/// Creates a new ring and returns a [`Handle`] to it.
	#[expect(clippy::missing_panics_doc)]
	pub fn create_ring(
		&'static self,
		parent: Handle<ring::Ring<A>>,
	) -> Result<Handle<ring::Ring<A>>, MapError> {
		let ring = self.ring_registry.insert(
			&self.pfa,
			ring::Ring::<A> {
				id:        usize::MAX, // placeholder
				parent:    Some(parent),
				instances: self.instance_list_registry.create_list(&self.pfa)?,
			},
		)?;

		// SAFETY(qix-): Will not panic.
		unsafe {
			ring.lock::<A::IntCtrl>().id = ring.id();
		}

		// SAFETY(qix-): As long as the kernel state has been initialized,
		// SAFETY(qix-): this won't panic.
		let _ = self.rings.as_ref().unwrap().append(&self.pfa, ring.clone());

		Ok(ring)
	}

	/// Creates a new module and returns a [`Handle`] to it.
	#[expect(clippy::missing_panics_doc)]
	pub fn create_module(
		&'static self,
		id: Id<{ IdType::Module }>,
	) -> Result<Handle<module::Module<A>>, MapError> {
		let instance_list = self.instance_list_registry.create_list(&self.pfa)?;

		let mapper = {
			// SAFETY(qix-): We don't panic here.
			let mut pfa = unsafe { self.pfa.lock::<A::IntCtrl>() };
			AddrSpace::<A>::duplicate_user_space_shallow(&self.user_mapper, &mut *pfa, &self.pat)
				.ok_or(MapError::OutOfMemory)?
		};

		let module = self.module_registry.insert(
			&self.pfa,
			module::Module::<A> {
				id: 0,
				module_id: id,
				instances: instance_list,
				mapper,
			},
		)?;

		// SAFETY(qix-): Will not panic.
		unsafe {
			module.lock::<A::IntCtrl>().id = module.id();
		}

		// SAFETY(qix-): As long as the kernel state has been initialized,
		// SAFETY(qix-): this won't panic.
		let _ = self
			.modules
			.as_ref()
			.unwrap()
			.append(&self.pfa, module.clone());

		Ok(module)
	}

	/// Creates a new instance and returns a [`Handle`] to it.
	#[expect(clippy::needless_pass_by_value)]
	pub fn create_instance(
		&'static self,
		module: Handle<module::Module<A>>,
		ring: Handle<ring::Ring<A>>,
	) -> Result<Handle<instance::Instance<A>>, MapError> {
		let thread_list = self.thread_list_registry.create_list(&self.pfa)?;
		let port_list = self.port_list_registry.create_list(&self.pfa)?;

		// SAFETY(qix-): We don't panic here.
		let mapper = unsafe {
			let module_lock = module.lock::<A::IntCtrl>();
			let mut pfa = self.pfa.lock::<A::IntCtrl>();
			AddrSpace::<A>::duplicate_user_space_shallow(module_lock.mapper(), &mut *pfa, &self.pat)
				.ok_or(MapError::OutOfMemory)?
		};

		let instance = self.instance_registry.insert(
			&self.pfa,
			instance::Instance {
				id: 0,
				module: module.clone(),
				ring: ring.clone(),
				threads: thread_list,
				ports: port_list,
				mapper,
			},
		)?;

		// SAFETY(qix-): Will not panic.
		unsafe {
			instance.lock::<A::IntCtrl>().id = instance.id();

			// SAFETY(qix-): As long as the kernel state has been initialized,
			// SAFETY(qix-): this won't panic.
			let _ = module
				.lock::<A::IntCtrl>()
				.instances
				.append(&self.pfa, instance.clone());
			let _ = ring
				.lock::<A::IntCtrl>()
				.instances
				.append(&self.pfa, instance.clone());
		}

		Ok(instance)
	}

	/// Creates a new thread and returns a [`Handle`] to it.
	///
	/// `stack_size` is rounded up to the next page size.
	#[expect(clippy::missing_panics_doc, clippy::needless_pass_by_value)]
	pub fn create_thread(
		&'static self,
		instance: Handle<instance::Instance<A>>,
		stack_size: usize,
		mut thread_state: A::ThreadState,
	) -> Result<Handle<thread::Thread<A>>, MapError> {
		// SAFETY(qix-): We don't panic here.
		let mapper = unsafe {
			let instance_lock = instance.lock::<A::IntCtrl>();
			let mut pfa = self.pfa.lock::<A::IntCtrl>();
			AddrSpace::<A>::duplicate_user_space_shallow(
				instance_lock.mapper(),
				&mut *pfa,
				&self.pat,
			)
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
				let mut pfa = self.pfa.lock::<A::IntCtrl>();

				for virt in (stack_first_page..stack_high_guard).step_by(0x1000) {
					let phys = pfa.allocate().ok_or(MapError::OutOfMemory)?;
					stack_segment.map(&mapper, &mut *pfa, &self.pat, virt, phys)?;
				}

				// Make sure the guard pages are unmapped.
				// This is more of an assertion to make sure the thread
				// state is not in a bad state, but should never be the
				// case outside of bugs.
				#[cfg(debug_assertions)]
				{
					match stack_segment.unmap(&mapper, &mut *pfa, &self.pat, stack_low_guard) {
						Ok(phys) => {
							panic!("a module's thread stack low guard page was mapped: {phys:016X}")
						}
						Err(UnmapError::NotMapped) => {}
						Err(e) => {
							panic!("failed to unmap a module's thread stack low guard page: {e:?}")
						}
					}

					match stack_segment.unmap(&mapper, &mut *pfa, &self.pat, stack_high_guard) {
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
				A::initialize_thread_mappings(&mapper, &mut thread_state, &mut *pfa, &self.pat)?;

				// Kill the PFA lock so that we can safely pass the reference
				// to the lock itself to the `.insert()`/`.append()` functions
				// without deadlocking.
				drop(pfa);

				let thread = self.thread_registry.insert(
					&self.pfa,
					thread::Thread::<A> {
						id: 0,
						instance: instance.clone(),
						// We set it in a moment.
						mapper: MaybeUninit::uninit(),
						// We set it in a moment.
						thread_state: MaybeUninit::uninit(),
						run_on_id: None,
						running_on_id: None,
					},
				)?;

				thread.lock::<A::IntCtrl>().id = thread.id();

				let _ = instance
					.lock::<A::IntCtrl>()
					.threads
					.append(&self.pfa, thread.clone())?;

				let _ = self
					.threads
					.as_ref()
					.unwrap()
					.append(&self.pfa, thread.clone())?;

				Ok(thread)
			};

			// Try to reclaim the memory we just allocated, if any.
			match map_result {
				Err(err) => {
					let mut pfa = self.pfa.lock::<A::IntCtrl>();

					if let Err(err) =
						A::reclaim_thread_mappings(&mapper, &mut thread_state, &mut *pfa, &self.pat)
					{
						dbg_err!(
							"failed to reclaim architecture thread mappings - MEMORY MAY LEAK: \
							 {err:?}"
						);
					}

					if let Err(err) =
						stack_segment.unmap_all_and_reclaim(&mapper, &mut *pfa, &self.pat)
					{
						dbg_err!("failed to reclaim thread stack - MEMORY MAY LEAK: {err:?}");
					}

					AddrSpace::<A>::free_user_space(mapper, &mut *pfa, &self.pat);

					return Err(err);
				}
				Ok(thread) => thread,
			}
		};

		// SAFETY(qix-): We don't panic here.
		unsafe {
			let mut thread_lock = thread.lock::<A::IntCtrl>();
			thread_lock.mapper.write(mapper);
			thread_lock.thread_state.write(thread_state);
		}

		Ok(thread)
	}
}

/// A trait for architectures to list commonly used types
/// to be passed around the kernel.
pub trait Arch: 'static {
	/// The physical address translator (PAT) the architecture
	/// uses.
	type Pat: Translator;
	/// The type of page frame allocator (PFA) the architecture
	/// uses.
	type Pfa: Alloc;
	/// The type of interrupt controller.
	type IntCtrl: InterruptController;
	/// The address space layout the architecture uses.
	type AddrSpace: AddressSpace;
	/// Architecture-specific thread state to be stored alongside
	/// each thread.
	type ThreadState: Sized = ();
	/// The core-local state type.
	type CoreState: Sized + 'static = ();

	/// Allows the architecture to further initialize an instance
	/// thread's mappings when threads are created.
	///
	/// This guarantees that, no matter from where the thread is
	/// created, the thread's address space will be initialized
	/// correctly for the architecture.
	fn initialize_thread_mappings(
		_thread: &<Self::AddrSpace as AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
		_pfa: &mut Self::Pfa,
		_pat: &Self::Pat,
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
		_pfa: &mut Self::Pfa,
		_pat: &Self::Pat,
	) -> Result<(), UnmapError> {
		Ok(())
	}
}

/// Helper trait association type for `Arch::AddrSpace`.
pub(crate) type AddrSpace<A> = <A as Arch>::AddrSpace;
/// Helper trait association type for `Arch::AddrSpace::SupervisorSegment`.
pub(crate) type SupervisorSegment<A> = <AddrSpace<A> as AddressSpace>::SupervisorSegment;
/// Helper trait association type for `Arch::AddrSpace::SupervisorHandle`.
pub(crate) type SupervisorHandle<A> = <AddrSpace<A> as AddressSpace>::SupervisorHandle;
/// Helper trait association type for `Arch::AddrSpace::UserHandle`.
pub(crate) type UserHandle<A> = <AddrSpace<A> as AddressSpace>::UserHandle;
