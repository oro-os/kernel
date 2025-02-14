//! aarch64 specific Oro functionality.
#![expect(clippy::inline_always)]

use crate::syscall;

/// Lowest level system call for aarch64.
#[inline(always)]
pub unsafe fn syscall(
	_opcode: syscall::Opcode,
	_arg1: u64,
	_arg2: u64,
	_arg3: u64,
	_arg4: u64,
) -> (syscall::Error, u64) {
	(syscall::Error::NotImplemented, 0)
}
