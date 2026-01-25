#[cfg(feature = "mmio")]
use core::sync::atomic::{AtomicU64, Ordering};

/// The current base offset for the vmem map.
///
/// All MMIO writes will be relative to this base address.
#[cfg(feature = "mmio")]
static VMM_BASE: AtomicU64 = AtomicU64::new(0);

/// Sets the base offset for the vmem map.
///
/// This will cause all future MMIO writes to be relative to the given
/// base address. This must be called whenever the linear mapping is
/// reconstructed (e.g. when the bootloader switches to the kernel).
///
/// # Safety
/// Caller must ensure that the provided offset is valid and that
/// writes to the MMIO bases (based on architecture) will all succeed
/// without causing undefined behavior.
#[cfg_attr(not(feature = "mmio"), inline(always))]
#[cfg_attr(
	not(feature = "mmio"),
	expect(
		unused_variables,
		reason = "vmm_base is only used when the 'mmio' feature is enabled"
	)
)]
pub unsafe fn set_vmm_base(base: u64) {
	#[cfg(feature = "mmio")]
	{
		VMM_BASE.store(base, Ordering::SeqCst);
	}
}

/// Gets the current base offset for the vmem map.
#[cfg(feature = "mmio")]
pub fn get_vmm_base() -> u64 {
	VMM_BASE.load(Ordering::SeqCst)
}
