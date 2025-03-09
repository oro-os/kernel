//! Implements the CPUID 01:0 lookup structure.

use oro_macro::bitstruct;

bitstruct! {
	/// Gets the `ebx` register values for the CPUID `eax=01, ecx=0` leaf.
	pub struct CpuidA01C0B(u32) {
		// NOTE(qix-): Not all bits are defined here, only the ones we care about.

		/// The local APIC ID. This **is not reliable** but may be useful
		/// for debugging purposes on a best-effort basis. For reliable APIC
		/// ID fetching, use APIC tables.
		pub local_apic_id[31:24] => as u8,
	}
}

impl CpuidA01C0B {
	/// Returns the `ebx` register value for the CPUID `eax=01, ecx=0` leaf.
	///
	/// Returns `None` if either `cpuid` is not supported or the leaf is not supported.
	#[must_use]
	pub fn get() -> Option<Self> {
		// NOTE(qix-): `highest_leaf` returns None if `cpuid` is not supported.
		if super::highest_leaf()? < 0x01 {
			return None;
		}

		let r: u32;
		unsafe {
			core::arch::asm!(
				// NOTE(qix-): LLVM uses `rbx` internally, so we have to preserve it.
				"push rbx",
				"mov eax, 0x01",
				"xor ecx, ecx",
				// NOTE(qix-): On older CPUs, unused output registers for `cpuid` were
				// NOTE(qix-): left untouched in some cases and not others, namely on
				// NOTE(qix-): capabilities checks. It's always a good idea to zero them
				// NOTE(qix-): out before calling `cpuid`.
				"xor edx, edx",
				"xor ebx, ebx",
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
