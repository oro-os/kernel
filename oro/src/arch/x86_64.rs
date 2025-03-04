//! x86_64 specific Oro functionality.
#![expect(clippy::inline_always)]

use core::{arch::asm, mem::transmute};

use crate::syscall;

/// Lowest level system call for x86_64.
///
/// # Safety
/// Inherently unsafe; this function call can do anything
/// from nothing, to shutting the machine down, to completely
/// changing the memory map from under you.
#[inline(always)]
pub unsafe fn syscall(
	opcode: syscall::Opcode,
	arg1: u64,
	arg2: u64,
	arg3: u64,
	arg4: u64,
) -> (syscall::Error, u64) {
	let mut err: u64 = opcode as u64;
	let mut ret: u64 = arg4;

	// SAFETY: Inline assembly is unavoidable to perform a syscall.
	unsafe {
		asm!(
			"syscall",
			inlateout("rax") err,
			in("rsi") arg1,
			in("rdi") arg2,
			in("rdx") arg3,
			inlateout("r9") ret,
			lateout("rsi") _,
			lateout("rdi") _,
			lateout("rdx") _,
			lateout("rcx") _,
			lateout("r8") _,
			lateout("r10") _,
			lateout("r11") _,
			lateout("zmm0") _,
			lateout("zmm1") _,
			lateout("zmm2") _,
			lateout("zmm3") _,
			lateout("zmm4") _,
			lateout("zmm5") _,
			lateout("zmm6") _,
			lateout("zmm7") _,
			lateout("zmm8") _,
			lateout("zmm9") _,
			lateout("zmm10") _,
			lateout("zmm11") _,
			lateout("zmm12") _,
			lateout("zmm13") _,
			lateout("zmm14") _,
			lateout("zmm15") _,
		);
	}

	// TODO(qix-): Don't transmute here; do something a bit safer.
	// SAFETY: We assume that the error code is valid.
	(unsafe { transmute::<u64, syscall::Error>(err) }, ret)
}
