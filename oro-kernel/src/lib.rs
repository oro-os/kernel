//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate includes both a library, with code common to all architectures,
//! and individual, architecture-specific binaries located in `bin/`.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(
	clippy::module_name_repetitions,
	clippy::struct_field_names,
	clippy::too_many_lines
)]
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
pub(crate) mod module;
pub(crate) mod port;
pub(crate) mod registry;
pub(crate) mod ring;

use core::{mem::MaybeUninit, str::FromStr};
use oro_arch::Target;
use oro_common::{
	arch::Arch,
	boot::BootConfig,
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

/// Core-specific boot information.
///
/// It is up to the architecture-specific implementations
/// to properly initialize this structure and pass it to
/// [`boot()`].
///
/// All general, system-wide configuration should be stored
/// in the boot protocol configuration otherwise.
#[repr(C, align(16))]
pub struct CoreConfig {
	/// The core ID.
	pub core_id:     u64,
	/// The core type.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	pub core_type:   CoreType,
	/// The boot protocol configuration.
	pub boot_config: &'static BootConfig,
	/// The head of the page frame allocator directly
	/// before the transfer.
	pub pfa_head:    u64,
}

/// The core type.
#[derive(PartialEq, Eq, Copy, Clone)]
pub enum CoreType {
	/// The core is the primary core.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	Primary,
	/// The core is a secondary core.
	Secondary,
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
pub unsafe fn boot(core_config: &CoreConfig) -> ! {
	#[allow(clippy::missing_docs_in_private_items)]
	macro_rules! wait_for_all_cores {
		() => {{
			static BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();

			if core_config.core_type == CoreType::Primary {
				BARRIER.set_total::<Target>(core_config.boot_config.core_count);
			}

			BARRIER.wait();
		}};
		(primary $primary:block secondary $secondary:block) => {{
			static BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();
			static AFTER_BARRIER: ::oro_common::sync::barrier::SpinBarrier =
				::oro_common::sync::barrier::SpinBarrier::new();

			if core_config.core_type == CoreType::Primary {
				$primary
				BARRIER.set_total::<Target>(core_config.boot_config.core_count);
				AFTER_BARRIER.set_total::<Target>(core_config.boot_config.core_count);
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
	let translator =
		OffsetPhysicalAddressTranslator::new(core_config.boot_config.linear_map_offset);
	let kernel_addr_space = <Target as Arch>::AddressSpace::current_supervisor_space(&translator);

	if core_config.core_type == CoreType::Primary {
		PFA.write(UnfairCriticalSpinlock::new(
			FiloPageFrameAllocator::with_last_free(translator.clone(), core_config.pfa_head),
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
			core_config.core_type == CoreType::Primary,
		);
	}}

	if core_config.core_type == CoreType::Primary {
		dbg!("kernel", "kernel transfer ok");

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

	// XXX DEBUG
	let module = module::ModuleInstance {
		id:        0,
		module_id: id::Id::from_str("M-1234ABCD5678EFGH9012IJKL3").unwrap(),
	};
	let _ref_module: self::registry::Ref<module::ModuleInstance> =
		self::registry::Ref::from(module).expect("failed to allocate");

	Target::halt()
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
