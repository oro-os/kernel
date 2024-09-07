//! Provides functionality for interacting with the Power State Coordination Interface (PSCI).

use core::arch::asm;

/// Indicates which PSCI mechanism to use when calling into the firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsciMethod {
	/// Use the HVC (Hypervisor Call) instruction.
	Hvc,
	/// Use the SMC (Secure Monitor Call) instruction.
	Smc,
}

// NOTE(qix-): I tried to make this DRY with macro_rules!
// NOTE(qix-): but the `asm!()` macro can't seem to be built up
// NOTE(qix-): in any meaningful way via macro parameters (namely
// NOTE(qix-): the `in` clauses). If you can figure out a way to
// NOTE(qix-): reduce these to simple definitions and codegen the
// NOTE(qix-): rest, please open a PR. Time spent trying: 2 hours.
impl PsciMethod {
	/// Get the version of the PSCI firmware.
	///
	/// # Safety
	/// Traps into a higher exception level. Caller must be in EL1.
	///
	/// This function is inherently unsafe as it deals with power levels
	/// and calls into the OEM's firmware.
	#[must_use]
	pub unsafe fn psci_version(&self) -> PsciVersion {
		let fnid: u32 = 0x8400_0000;
		let r: u32;

		match self {
			PsciMethod::Hvc => asm!("hvc 0", in("w0") fnid, lateout("w0") r),
			PsciMethod::Smc => asm!("smc 0", in("w0") fnid, lateout("w0") r),
		}

		r.into()
	}
}

/// Specifies the version of the PSCI interface.
#[derive(Clone, Debug)]
pub struct PsciVersion {
	/// The major version.
	pub major: u16,
	/// The minor version.
	pub minor: u16,
}

impl From<u32> for PsciVersion {
	fn from(v: u32) -> Self {
		Self {
			major: ((v >> 16) & 0x7FFF) as u16,
			minor: v as u16,
		}
	}
}
