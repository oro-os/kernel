//! Syscall handler implementation for x86_64.

use core::arch::naked_asm;

use crate::asm::{rdmsr, wrmsr};

/// Installs the syscall handler.
pub fn install_syscall_handler() {
	// Enable the IA32_EFER.SCE bit, which enables syscall/sysret.
	// Otherwise, #UD will be raised when executing SYSCALL on
	// intel processors.
	let mut ia32_efer = rdmsr(0xC000_0080);
	ia32_efer |= 1; // Set the SCE bit
	wrmsr(0xC000_0080, ia32_efer);

	// Tell the CPU to clear the interrupt enable flag (IF) and trap flag (TF) when
	// executing a syscall via the SFMASK MSR.
	//
	// TODO(qix-): Make an RFLAGS register in crate::reg and use that instead
	// TODO(qix-): of hardcoding the value.
	wrmsr(0xC000_0084, 0x0200 | 0x0100);

	// Tell the CPU which CS and SS selectors to use when executing a syscall.
	// See the `STAR` constant in the `gid` module for more information.
	const STAR: u64 = ((must_be_u16(crate::gdt::STAR_KERNEL) as u64) << 32)
		| ((must_be_u16(crate::gdt::STAR_USER) as u64) << 48);

	wrmsr(0xC000_0081, STAR);

	// Tell the CPU where to jump to when a syscall is executed (LSTAR MSR).
	wrmsr(0xC000_0082, syscall_enter_non_compat as *const () as u64);
}

/// Entry point for 64-bit, non-compatibility-mode syscalls.
///
/// # Safety
/// **This is an incredibly sensitive security boundary.**
/// Maintenance on this function should be done with extreme caution and care.
///
/// Not to be called directly by kernel code.
#[naked]
#[no_mangle]
unsafe extern "C" fn syscall_enter_non_compat() {
	// XXX(qix-): Just a placeholder for now.
	naked_asm! {
		"4: hlt",
		"jmp 4b",
	}
}

#[doc(hidden)]
const fn must_be_u16(x: u16) -> u16 {
	x
}
