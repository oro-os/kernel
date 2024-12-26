//! x86_64 specific Oro functionality.
#![expect(clippy::inline_always)]

use core::{arch::asm, mem::transmute};

use crate::syscall;

#[expect(clippy::missing_docs_in_private_items)]
#[inline(always)]
pub unsafe fn syscall_reg_get_raw(entity: u64, table: u64, key: u64) -> (u64, syscall::Error) {
	let err: u32;
	let ret: u64;

	asm!(
		"syscall",
		in("rax") syscall::Opcode::RegGet as u64,
		in("rsi") table,
		in("rdi") key,
		in("r9") entity,
		lateout("eax") err,
		out("rdx") ret,
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

	(ret, transmute::<u32, syscall::Error>(err))
}

#[expect(clippy::missing_docs_in_private_items)]
#[inline(always)]
pub unsafe fn syscall_reg_set_raw(entity: u64, table: u64, key: u64, value: u64) -> syscall::Error {
	let err: u32;

	asm!(
		"syscall",
		in("rax") syscall::Opcode::RegSet as u64,
		in("rsi") table,
		in("rdi") key,
		in("r9") entity,
		in("rdx") value,
		lateout("eax") err,
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

	transmute::<u32, syscall::Error>(err)
}
