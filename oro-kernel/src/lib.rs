//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate includes both a library, with code common to all architectures,
//! and individual, architecture-specific binaries located in `bin/`.
#![no_std]
// NOTE(qix-): `adt_const_params` isn't strictly necessary but is on track for acceptance,
// NOTE(qix-): and the open questions (e.g. mangling) are not of concern here.
// NOTE(qix-): https://github.com/rust-lang/rust/issues/95174
// NOTE(qix-):
// NOTE(qix-): `const_refs_to_static` is necessary for the `RegistryTarget` trait,
// NOTE(qix-): which requires a static reference to the registry as a const trait
// NOTE(qix-): item. This feature is on track for stabilization:
// NOTE(qix-): https://github.com/rust-lang/rust/issues/128183
#![allow(incomplete_features)]
#![feature(adt_const_params, const_refs_to_static)]

pub(crate) mod id;
pub(crate) mod local;
pub(crate) mod module;
pub(crate) mod port;
pub(crate) mod registry;
pub(crate) mod ring;

use core::mem::MaybeUninit;
use oro_arch::Target;
use oro_common::{
	arch::Arch,
	dbg, dbg_err,
	mem::{
		mapper::AddressSpace, pfa::filo::FiloPageFrameAllocator,
		translate::OffsetPhysicalAddressTranslator,
	},
	sync::spinlock::unfair_critical::UnfairCriticalSpinlock,
};

/// The type of the physical address translator we'll use system-wide.
type PhysicalTranslator = OffsetPhysicalAddressTranslator;
/// The type of PFA we'll use system-wide.
// NOTE(qix-): This is explicitly defined here since Rust doesn't have
// NOTE(qix-): any way to define a "generic static" of sorts. TAIT does not
// NOTE(qix-): work here. It's also why registries are defined in this file
// NOTE(qix-): and not in their own type modules.
type Pfa = FiloPageFrameAllocator<PhysicalTranslator>;

/// Holds the shared PFA.
static mut PFA: MaybeUninit<UnfairCriticalSpinlock<Pfa>> = MaybeUninit::uninit();

/// TODO(qix-): TEMPORARY SOLUTION during the boot sequence refactor.
#[doc(hidden)]
#[allow(missing_docs)]
pub mod config {
	pub static mut IS_PRIMARY_CORE: bool = false;
	pub static mut NUM_CORES: u64 = 0;
	pub static mut LINEAR_MAP_OFFSET: usize = 0;
	pub static mut PFA_HEAD: u64 = 0;
}

/// Runs the kernel.
///
/// This is the main entry point for the kernel.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// Further, all architecture-specific setup MUST have completed
/// on ALL CORES before calling this function.
///
/// The `core_config` parameter must be properly initialized.
/// Specifically, all safety requirements must be met, such as
/// marking exactly one core as primary.
#[allow(clippy::missing_panics_doc)] // XXX DEBUG
pub unsafe fn boot() -> ! {
	let is_primary_core = config::IS_PRIMARY_CORE;
	let core_count = config::NUM_CORES;

	#[allow(clippy::missing_docs_in_private_items)]
	macro_rules! wait_for_all_cores {
		() => {{
			static BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();

			if is_primary_core {
				BARRIER.set_total::<Target>(core_count);
			}

			BARRIER.wait();
		}};
		(primary $primary:block secondary $secondary:block) => {{
			static BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();
			static AFTER_BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();

			if is_primary_core {
				$primary
				BARRIER.set_total::<Target>(core_count);
				AFTER_BARRIER.set_total::<Target>(core_count);
				BARRIER.wait();
			} else {
				BARRIER.wait();
				$secondary
			}

			AFTER_BARRIER.wait();
		}};
		($($t:stmt)*) => {{
			// TODO(qix-): Simplify this such that we don't duplicate code.
			wait_for_all_cores! {
				primary {
					$( $t )*
				}
				secondary {
					$( $t )*
				}
			}
		}};
	}

	wait_for_all_cores!();

	// Set up the PFA.
	let translator = OffsetPhysicalAddressTranslator::new(config::LINEAR_MAP_OFFSET);
	let kernel_addr_space = <Target as Arch>::AddressSpace::current_supervisor_space(&translator);

	if is_primary_core {
		PFA.write(UnfairCriticalSpinlock::new(
			FiloPageFrameAllocator::with_last_free(translator.clone(), config::PFA_HEAD),
		));

		Target::strong_memory_barrier();
	}

	wait_for_all_cores!();

	// SAFETY(qix-): Since we lockstep initialize the shared PFA, it is safe to
	// SAFETY(qix-): assume that it is initialized here.
	let pfa: &'static _ = &*core::ptr::from_ref(PFA.assume_init_ref());

	wait_for_all_cores! {{
		let mut pfa = pfa.lock::<Target>();
		Target::after_transfer(
			&kernel_addr_space,
			&translator,
			&mut *pfa,
			is_primary_core,
		);
	}}

	if is_primary_core {
		// Initialize the registries.
		{
			use crate::registry::RegistryTarget;

			#[allow(clippy::missing_docs_in_private_items)]
			macro_rules! create_registry {
				($ty:ty, $segment:ident) => {
					#[allow(clippy::missing_docs_in_private_items)]
					const _: () = {
						// SAFETY(qix-): While this is "mutable", it's only ever able to be mutated
						// SAFETY(qix-): by the `initialize_registry()` function, which is called exactly
						// SAFETY(qix-): once per registry.
						static mut REGISTRY: core::mem::MaybeUninit<
							crate::registry::Registry<$ty, PhysicalTranslator, Pfa>,
						> = core::mem::MaybeUninit::uninit();

						unsafe impl crate::registry::RegistryTarget for $ty {
							type Alloc = Pfa;
							type PhysicalAddressTranslator = PhysicalTranslator;

							// SAFETY(qix-): This is not used by the registry until it is initialized.
							const REGISTRY_PTR: *const crate::registry::Registry<
								Self,
								Self::PhysicalAddressTranslator,
								Self::Alloc,
							> = unsafe { REGISTRY.as_ptr() };

							unsafe fn initialize_registry(
								segment: <<Target as Arch>::AddressSpace as AddressSpace>::SupervisorSegment,
								translator: Self::PhysicalAddressTranslator,
							) {
								REGISTRY.write(crate::registry::Registry::new(&*PFA.as_ptr(), translator, segment));
							}
						}
					};

					// SAFETY(qix-): Assuming the address space is implemented correctly
					// SAFETY(qix-): by the architecture-specific crates, this is safe
					// SAFETY(qix-): because cloning the address space handle prescribes
					// SAFETY(qix-): that each new handle only touches a non-overlapping
					// SAFETY(qix-): segment, and no others, exclusively. This is the case
					// SAFETY(qix-): as all registry segments are prescribed by the architecture
					// SAFETY(qix-): address space traits as non-overlapping.
					<$ty>::initialize_registry(
						<<Target as ::oro_common::arch::Arch>::AddressSpace as ::oro_common::mem::mapper::AddressSpace>::$segment(),
						translator.clone(),
					);
				};
			}

			create_registry!(
				crate::module::ModuleInstance,
				kernel_module_instance_registry
			);
			create_registry!(crate::port::Port, kernel_port_registry);
			create_registry!(crate::ring::Ring, kernel_ring_registry);
		}

		Target::strong_memory_barrier();
	}

	wait_for_all_cores!();

	// SAFETY(qix-): Only called once, here, and before we register the interrupt handlers.
	// SAFETY(qix-): We also barrier afterward, before we register the interrupt handlers.
	self::local::state::initialize_core_state(
		unsafe { &mut *pfa.lock::<Target>() },
		&translator,
		is_primary_core,
	)
	.expect("failed to initialize core local state");
	wait_for_all_cores!();
	Target::initialize_interrupts::<self::local::interrupt::KernelInterruptHandler>();
	wait_for_all_cores!();

	if is_primary_core {
		dbg!("kernel", "kernel transfer ok");
	}

	self::local::main::run()
}

/// Panic handler for the kernel.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	dbg_err!("kernel", "panic: {:?}", info);
	Target::halt()
}
