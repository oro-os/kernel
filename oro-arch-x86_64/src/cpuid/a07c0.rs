//! Implements the CPUID 07:0 lookup structure.

use oro_macro::bitstruct;

bitstruct! {
	/// Gets the `ebx` register values for the CPUID `eax=07, ecx=0` leaf.
	pub struct CpuidA07C0B(u32) {
		// NOTE(qix-): Not all bits are defined here, only the ones we care about.

		/// Whether or not FSGSBASE instructions are supported.
		pub fsgsbase[0] => as bool,
	}
}

impl CpuidA07C0B {
	/// Returns the `ebx` register value for the CPUID `eax=07, ecx=0` leaf.
	///
	/// Returns `None` if either `cpuid` is not supported or the leaf is not supported.
	#[must_use]
	pub fn get() -> Option<Self> {
		// NOTE(qix-): `highest_leaf` returns None if `cpuid` is not supported.
		if super::highest_leaf()? < 0x07 {
			return None;
		}

		let r: u32;
		unsafe {
			core::arch::asm!(
				// NOTE(qix-): LLVM uses `rbx` internally, so we have to preserve it.
				"push rbx",
				"mov eax, 0x07",
				"mov ecx, 0",
				"cpuid",
				"mov eax, ebx",
				"pop rbx",
				lateout("eax") r,
				out("ecx") _,
				out("edx") _,
				options(nostack, preserves_flags),
			);
		}

		Some(Self(r))
	}
}
