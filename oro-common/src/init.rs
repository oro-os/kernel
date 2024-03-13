use crate::{
	boot::{BootConfig, BootInstanceType, BootMemoryRegion},
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
	_config: &'static BootConfig<M>,
	core_id: u64,
	instance_type: BootInstanceType,
) -> ! {
	A::disable_interrupts();

	crate::dbg!(
		A,
		"boot_to_kernel",
		"booting to kernel (core_id = {core_id}, instance_type = {instance_type:?})"
	);

	A::halt()
}
