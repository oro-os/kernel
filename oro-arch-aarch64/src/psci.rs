//! Provides functionality for interacting with the Power State Coordination Interface (PSCI).

use core::arch::asm;

/// Error codes returned by PSCI
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i64)]
pub enum Error {
	/// The function or types of arguments to the function
	/// is/are not supported.
	NotSupported  = -1,
	/// One or more parameters are invalid.
	InvalidParams = -2,
	/// The firmware denied the PSCI request.
	Denied        = -3,
	/// The given target is already on.
	AlreadyOn     = -4,
	/// The target is already pending being woken up.
	OnPending     = -5,
	/// An internal failure occurred.
	Internal      = -6,
	/// The target is not present.
	NotPresent    = -7,
	/// The target is disabled.
	Disabled      = -8,
	/// A given address is invalid.
	InvalidAddr   = -9,
}

impl Error {
	/// Checks the given code and returns an `Err` if it matches
	/// one of the error codes.
	fn check32<T: From<u32>>(e: u32) -> Result<T> {
		#[expect(clippy::cast_possible_wrap)]
		let i = e as i32;
		if i < 0 && i > -10 {
			// SAFETY: We check the valid range of error codes prior to transmutation.
			Err(unsafe { core::mem::transmute::<i64, Error>(i.into()) })
		} else {
			Ok(T::from(e))
		}
	}

	/// Checks the given code and returns an `Err` if it matches
	/// one of the error codes.
	fn check64<T: From<u64>>(e: u64) -> Result<T> {
		#[expect(clippy::cast_possible_wrap)]
		let i = e as i64;
		if i < 0 && i > -10 {
			// SAFETY: We check the valid range of error codes prior to transmutation.
			Err(unsafe { core::mem::transmute::<i64, Error>(i) })
		} else {
			Ok(T::from(e))
		}
	}
}

/// Results returned by PSCI functions.
pub type Result<T> = core::result::Result<T, Error>;

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
	pub unsafe fn psci_version(&self) -> Result<PsciVersion> {
		let fnid: u32 = 0x8400_0000;
		let r: u32;

		unsafe {
			match self {
				PsciMethod::Hvc => asm!("hvc 0", in("w0") fnid, lateout("w0") r),
				PsciMethod::Smc => asm!("smc 0", in("w0") fnid, lateout("w0") r),
			}
		}

		Error::check32(r)
	}

	/// Brings up the given core.
	///
	/// # Safety
	/// Traps into a higher exception level. Caller must be in EL1.
	///
	/// This function is inherently unsafe as it deals with power levels
	/// and calls into the OEM's firmware.
	pub unsafe fn cpu_on(&self, target_cpu: u64, entry_point: u64, context_id: u64) -> Result<()> {
		let fnid: u32 = 0xC400_0003;
		let r: u64;

		unsafe {
			match self {
				PsciMethod::Hvc => {
					asm!("hvc 0", in("w0") fnid, in("x1") target_cpu, in("x2") entry_point, in("x3") context_id, lateout("x0") r);
				}
				PsciMethod::Smc => {
					asm!("smc 0", in("w0") fnid, in("x1") target_cpu, in("x2") entry_point, in("x3") context_id, lateout("x0") r);
				}
			}
		}

		let _ = Error::check64::<u64>(r)?;
		Ok(())
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
