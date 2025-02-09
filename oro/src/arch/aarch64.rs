//! aarch64 specific Oro functionality.
#![expect(clippy::inline_always)]

use crate::syscall;

/// The top address (exclusive; one byte higher past the end)
/// of the heap. The heap's pages grow downwards from this address.
pub const fn heap_top() -> u64 {
	// TODO(qix-): This is a temporary solution, and will definitely change.
	// TODO(qix-): See <https://github.com/oro-os/kernel/issues/49> (while that
	// TODO(qix-): issue is mostly about x86_64 LA57, the question of how address
	// TODO(qix-): space is laid out, especially w.r.t. ASLR, is still TBD).
	230 << (12 + 9 * 3)
}

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
