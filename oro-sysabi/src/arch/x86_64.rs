//! x86_64 specific Oro functionality.
#![expect(clippy::inline_always)]

use core::{arch::asm, mem::transmute};

use crate::syscall;

#[expect(clippy::missing_docs_in_private_items)]
#[inline(always)]
pub unsafe fn syscall_reg_get_raw(entity: u64, table: u64, key: u64) -> (u64, syscall::Error) {
	let err: u64;
	let ret: u64;

	asm!(
		"syscall",
		in("r11") syscall::Opcode::RegGet as u64,
		in("r12") table,
		in("r13") key,
		in("r14") entity,
		lateout("r11") err,
		lateout("r13") ret,
	);

	(ret, transmute::<u32, syscall::Error>(err as u32))
}

#[expect(clippy::missing_docs_in_private_items)]
#[inline(always)]
pub unsafe fn syscall_reg_set_raw(entity: u64, table: u64, key: u64, value: u64) -> syscall::Error {
	let err: u64;

	asm!(
		"syscall",
		in("r11") syscall::Opcode::RegSet as u64,
		in("r12") table,
		in("r13") key,
		in("r14") entity,
		in("r15") value,
		lateout("r11") err,
	);

	transmute::<u32, syscall::Error>(err as u32)
}
