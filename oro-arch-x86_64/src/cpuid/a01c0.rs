//! Implements the CPUID 01:0 lookup structure, _Processor Info and Feature Bits_.

use oro_macro::bitstruct;

bitstruct! {
	/// Gets the `ebx` register values for the CPUID `eax=01, ecx=0` leaf.
	pub struct Ebx(u32) {
		/// The local APIC ID. This **is not reliable** but may be useful
		/// for debugging purposes on a best-effort basis. For reliable APIC
		/// ID fetching, use APIC tables.
		pub local_apic_id[31:24] => as u8,
	}
}

/// Processor Info and Feature Bits
pub struct CpuidA01C0 {
	/// The `ebx` register of the cpuid call.
	pub ebx: Ebx,
}

impl CpuidA01C0 {
	/// Executes CPUID with `eax=1, ecx=0`, which provides processor information
	/// and feature bits.
	///
	/// Returns `None` if either `cpuid` is not supported or the leaf is not supported.
	///
	/// # Performance
	/// This is an incredibly slow and **serializing** operation. If used frequently, its
	/// result should be cached.
	#[must_use]
	pub fn get() -> Option<Self> {
		super::cpuid(0x01, 0x00).map(|r| Self { ebx: Ebx(r.ebx) })
	}
}
