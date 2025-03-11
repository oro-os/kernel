//! Implements the CPUID 0D:0 lookup structure, _XSAVE Features and State Components_.

use oro_macro::bitstruct;

bitstruct! {
	/// 64-bit bitmap of state-components supported by `XCR0` on this CPU.
	pub struct ComponentBits(u64) {
		/// x87 state; Enabled with: XCR0
		pub x87_state[0] => as bool,
		/// SSE state: XMM0–XMM15 and MXCSR; Enabled with: XCR0
		pub sse_state[1] => as bool,
		/// AVX state: top halves of YMM0–YMM15; Enabled with: XCR0
		pub avx_state[2] => as bool,
		/// MPX state: BND0–BND3 bounds registers; Enabled with: XCR0
		pub mpx_bounds[3] => as bool,
		/// MPX state: BNDCFGU and BNDSTATUS registers; Enabled with: XCR0
		pub mpx_regs[4] => as bool,
		/// AVX-512 state: opmask registers k0–k7; Enabled with: XCR0
		pub avx512_opmask[5] => as bool,
		/// AVX-512 "ZMM_Hi256" state: top halves of ZMM0–ZMM15; Enabled with: XCR0
		pub avx512_zmm_hi256[6] => as bool,
		/// AVX-512 "Hi16_ZMM" state: ZMM16–ZMM31; Enabled with: XCR0
		pub avx512_hi16_zmm[7] => as bool,
		/// Processor Trace state; Enabled with: IA32_XSS
		pub processor_trace_state[8] => as bool,
		/// PKRU (User Protection Keys) register; Enabled with: XCR0
		pub pkru[9] => as bool,
		/// PASID (Process Address Space ID) state; Enabled with: IA32_XSS
		pub pasid_state[10] => as bool,
		/// CET_U state (Control-flow Enforcement Technology: user-mode functionality MSRs); Enabled with: IA32_XSS
		pub cet_u_state[11] => as bool,
		/// CET_S state (CET: shadow stack pointers for rings 0,1,2); Enabled with: IA32_XSS
		pub cet_s_state[12] => as bool,
		/// HDC (Hardware Duty Cycling) state; Enabled with: IA32_XSS
		pub hdc_state[13] => as bool,
		/// UINTR (User-Mode Interrupts) state; Enabled with: IA32_XSS
		pub uintr_state[14] => as bool,
		/// LBR (Last Branch Record) state; Enabled with: IA32_XSS
		pub lbr_state[15] => as bool,
		/// HWP (Hardware P-state control) state; Enabled with: IA32_XSS
		pub hwp_state[16] => as bool,
		/// AMX tile configuration state: TILECFG; Enabled with: XCR0
		pub amx_tilecfg[17] => as bool,
		/// AMX tile data registers: tmm0–tmm7; Enabled with: XCR0
		pub amx_tile_data[18] => as bool,
		/// APX extended general-purpose registers: r16–r31; Enabled with: XCR0
		pub apx_state[19] => as bool,
		/// Lightweight Profiling (LWP) (AMD only); Enabled with: XCR0
		pub lwp_state[62] => as bool,
	}
}

/// XSAVE Features and State Components
#[derive(Debug)]
pub struct CpuidA0DC0 {
	/// Maximum size (in bytes) of XSAVE save area for the set of state-components currently set in `XCR0`.
	pub max_xsave_area:     u32,
	/// Maximum size (in bytes) of XSAVE save area if all state-components supported by `XCR0` on this CPU were enabled at the same time.
	pub max_xsave_area_all: u32,
	/// 64-bit bitmap of state-components supported by `XCR0` on this CPU.
	pub components:         ComponentBits,
}

impl CpuidA0DC0 {
	/// Executes CPUID with `eax=0x0D, ecx=0`, which provides XSAVE features and state components.
	///
	/// Returns `None` if either `cpuid` is not supported or the leaf is not supported.
	///
	/// # Performance
	/// This is an incredibly slow and **serializing** operation. If used frequently, its
	/// result should be cached.
	#[must_use]
	pub fn get() -> Option<Self> {
		super::cpuid(0x0D, 0x00).map(|r| {
			Self {
				max_xsave_area:     r.ebx,
				max_xsave_area_all: r.ecx,
				components:         ComponentBits((u64::from(r.edx) << 32) | u64::from(r.eax)),
			}
		})
	}
}
