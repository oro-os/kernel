//! Implements core halting for the AArch64 architecture.

impl orok_arch_base::Halt for super::Arch {
	unsafe fn halt_once() {
		// SAFETY: Halting is only possible with inline assembly.
		unsafe {
			::core::arch::asm! {
				"wfi"
			};
		}
	}
}
