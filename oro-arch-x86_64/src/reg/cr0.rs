//! Implements the cr0 register on x86_64.

use core::arch::asm;

use oro_kernel_macro::bitstruct;

bitstruct! {
	/// The CR0 register on X86_64.
	pub struct Cr0(u64) {
		/// Enables protected mode.
		pub protected_mode_enable[0] => as bool,
		/// Enables the coprocessor monitor bit, which determines if WAIT/FWAIT causes an exception
		/// (CR0.MP=1, CR0.TS=1) or not (CR0.MP=0 _OR_ CR0.TS=0).
		pub monitor_coprocessor[1] => as bool,
		/// Enables emulation. If 1, #UD is raised when executing x87 instructions such that they can be emulated.
		pub emulation[2] => as bool,
		/// Enables context-saving on x87-related task switching.
		///
		/// CR0.MP controls if WAIT/FWAIT causes an #NM when CR0.TS=1.
		pub task_switch[3] => as bool,
		/// Enables write protection for read-only pages at the supervisor level.
		pub write_protect[16] => as bool,
		/// Enables alignment checking.
		///
		/// Under CPL3, alignment checks are performed on every data reference and raise #AC if they fail.
		pub alignment_mask[18] => as bool,
		/// Not write through.
		///
		/// When enabled, disables write-through caching.
		pub not_writethrough[29] => as bool,
		/// When DISABLED, the CPU cache is ENABLED.
		///
		/// When disabled (CR0.CD=1), the CPU does not bring new instructions/data into caches.
		pub cache_disable[30] => as bool,
		/// Enables paging. If disabled, the CPU operates in real mode.
		pub paging_enable[31] => as bool,
	}
}

impl Cr0 {
	/// Stores the CR0 register.
	///
	/// # Safety
	/// This function is unsafe because it can break the system if used incorrectly.
	///
	/// Several reserved bits must remain the same as they were before the modification
	/// under specific environments. The caller must ensure that these bits are not modified.
	pub unsafe fn store(self) {
		// SAFETY: Safety considerations have been offloaded to the caller.
		unsafe {
			asm!("mov cr0, {}", in(reg) self.0);
		}
	}

	/// Gets the current CR0 register.
	#[must_use]
	pub fn load() -> Self {
		let cr0: u64;
		// SAFETY: Always safe.
		unsafe {
			asm!("mov {}, cr0", out(reg) cr0);
		}
		Self(cr0)
	}
}
