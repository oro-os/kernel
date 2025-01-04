//! System call handling for the AArch64 architecture.

/// Holds the data related to a system call frame.
pub struct SystemCallFrame;

impl oro_kernel::arch::SystemCallHandle for SystemCallFrame {
	#[inline]
	fn opcode(&self) -> oro_sysabi::syscall::Opcode {
		todo!();
	}

	#[inline]
	fn table_id(&self) -> u64 {
		todo!();
	}

	#[inline]
	fn key(&self) -> u64 {
		todo!();
	}

	#[inline]
	fn value(&self) -> u64 {
		todo!();
	}

	#[inline]
	fn entity_id(&self) -> u64 {
		todo!();
	}

	#[inline]
	fn set_error(&mut self, _error: oro_sysabi::syscall::Error) {
		todo!();
	}

	#[inline]
	fn set_return_value(&mut self, _value: u64) {
		todo!();
	}
}
