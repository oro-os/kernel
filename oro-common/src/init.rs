use crate::{
	boot::{BootConfig, BootInstanceType, BootMemoryRegion},
	dbg,
	sync::SpinBarrier,
	Arch,
};
use oro_ser2mem::CloneIterator;

/// Initializes the Oro kernel. This is to be called by the bootloader,
/// once per core, with the `instance_type` appropriately set.
///
/// The `core_id` places no restrictions on the value, but it must be unique
/// for each core.
///
/// # Safety
/// A number of safety invariants that must be upheld:
///
/// 1. The `config` parameter must be a valid reference to a `BootConfig` structure.
///    It must be properly static and accessible to all cores, and must be the same for all cores.
///    It must survive non-volatile reads.
///
/// 2. Only the main CPU core (the "primary" core, for whatever definition of "primary" is
///    applicable to the architecture) should call this function with `instance_type` set to
///    [`BootInstanceType::Primary`]. All other cores must call this function with `instance_type`
///    set to [`BootInstanceType::Secondary`]. Not doing so will put the system in an undefined
///    and potentially dangerous state.
///
/// 3. This function must be called only once per core. It is not safe to call this function
///    multiple times on the same core, and could lead to undefined behavior or put the system
///    in a dangerous state.
///
/// 4. The `core_id` must be unique for each core.
///
/// 5. Be advised that interrupts are disabled when this function is called. No code should rely on
///    them being enabled beyond the point of executing this function.
///
/// There are debug assertions in place to catch these conditions during development,
/// but they are not guaranteed to catch all cases.
pub unsafe fn boot_to_kernel<A: Arch, M: CloneIterator<Item = BootMemoryRegion> + Clone>(
	config: &'static BootConfig<M>,
	preboot_config: &PrebootConfig,
) -> ! {
	A::disable_interrupts();

	dbg!(
		A,
		"boot_to_kernel",
		"booting to kernel (core_id = {}, instance_type = {:?})",
		preboot_config.core_id,
		preboot_config.instance_type
	);

	// Wait for all cores to come online
	{
		static BARRIER: SpinBarrier = SpinBarrier::new();
		if preboot_config.instance_type == BootInstanceType::Primary {
			BARRIER.set_total::<A>(config.num_instances);
		}
		BARRIER.wait();
		if preboot_config.instance_type == BootInstanceType::Primary {
			dbg!(A, "boot_to_kernel", "all cores online");
		}
	}

	A::halt()
}

/// Configures how the boot initialization sequence should proceed
/// based on the environment set up by the bootloader.
#[derive(Debug, Clone)]
pub struct PrebootConfig {
	/// The unique identifier for this core.
	///
	/// # Safety
	/// Must be unique between all cores.
	pub core_id: u64,
	/// The type of instance this core is.
	///
	/// # Safety
	/// Only one core may specify itself as the [`BootInstanceType::Primary`].
	/// All others must specify themselves as [`BootInstanceType::Secondary`].
	pub instance_type: BootInstanceType,
	/// Memory layout of the pre-boot environment.
	///
	/// # Safety
	/// This MUST be correct as it is used to reference physical pages
	/// and other memory regions in order to map them to the new kernel
	/// virtual memory map.
	pub memory_layout_type: MemoryLayoutType,
}

/// The type of memory layout of the pre-boot environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLayoutType {
	/// The pre-boot environment has mapped all physical memory
	/// regions to a contiguous virtual address space starting
	/// at the given offset.
	LinearMapped {
		/// This offset is added to all physical addresses to find
		/// the corresponding virtual address mapped into linear memory
		/// by the bootloader when performing reads and writes.
		offset: usize,
	},
}
