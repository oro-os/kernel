//! Implements the CPUID 07:2 lookup structure, _Extended Features (`ecx=2`)_.

use oro_macro::bitstruct;

bitstruct! {
	/// Gets the `edx` register values for the CPUID `eax=07, ecx=07` leaf.
	pub struct Edx(u32) {
		/// Fast Store Forwarding Predictor disable supported. (SPEC_CTRL (MSR 48h) bit 7)
		pub psfd[0] => as bool,
		/// IPRED_DIS controls supported. (SPEC_CTRL bits 3 and 4)
		pub ipred_ctrl[1] => as bool,
		/// RRSBA behavior disable supported. (SPEC_CTRL bits 5 and 6)
		pub rrsba_ctrl[2] => as bool,
		/// Data Dependent Prefetcher disable supported. (SPEC_CTRL bit 8)
		pub ddpd_u[3] => as bool,
		/// BHI_DIS_S behavior enable supported. (SPEC_CTRL bit 10)
		pub bhi_ctrl[4] => as bool,
		/// If set, the processor does not exhibit MXCSR configuration dependent timing.
		pub mcdt_no[5] => as bool,
		// Bit 6 is skipped due to missing mnemonic.
		/// If set, indicates that the MONITOR/UMONITOR instructions are not affected by performance/power issues
		/// caused by exceeding the capacity of an internal monitor tracking table.
		pub monitor_mitg_no[7] => as bool,
	}
}

/// Extended Features (`ecx=2`)
pub struct CpuidA07C2 {
	/// The `edx` register of the cpuid call.
	pub edx: Edx,
}

impl CpuidA07C2 {
	/// Executes CPUID with `eax=07, ecx=02`, which provides extended features.
	///
	/// Returns `None` if either CPUID is not supported or the leaf is not supported.
	///
	/// # Performance
	/// This is an incredibly slow and **serializing** operation. If used frequently, its
	/// result should be cached.
	#[must_use]
	pub fn get() -> Option<Self> {
		super::cpuid(0x07, 0x02).map(|r| Self { edx: Edx(r.edx) })
	}
}
