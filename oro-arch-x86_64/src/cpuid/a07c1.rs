//! Implements the CPUID 07:1 lookup structure, _Extended Features (`ecx=1`)_.

use oro_kernel_macro::bitstruct;

bitstruct! {
	/// Gets the `eax` register values for the CPUID `eax=07, ecx=1` leaf.
	pub struct Eax(u32) {
		/// SHA-512 extensions
		pub sha512[0] => as bool,
		/// SM3 hash extensions
		pub sm3[1] => as bool,
		/// SM4 cipher extensions
		pub sm4[2] => as bool,
		/// Remote Atomic Operations on integers: AADD, AAND, AOR, AXOR instructions
		pub rao_int[3] => as bool,
		/// AVX Vector Neural Network Instructions (VNNI) (VEX encoded)
		pub avx_vnni[4] => as bool,
		/// AVX-512 instructions for bfloat16 numbers
		pub avx512_bf16[5] => as bool,
		/// Linear Address Space Separation (CR4 bit 27)
		pub lass[6] => as bool,
		/// `CMPccXADD` instructions
		pub cmpccxadd[7] => as bool,
		/// Architectural Performance Monitoring Extended Leaf (EAX=23h)
		pub archperf_monext[8] => as bool,
		/// Fast zero-length REP MOVSB
		pub fzrm[10] => as bool,
		/// Fast short REP STOSB
		pub fsrs[11] => as bool,
		/// Fast short REP CMPSB and REP SCASB
		pub rsrcs[12] => as bool,
		/// Flexible Return and Event Delivery
		pub fred[17] => as bool,
		/// LKGS Instruction
		pub lkgs[18] => as bool,
		/// WRMSRNS instruction (non-serializing write to MSRs)
		pub wrmsrns[19] => as bool,
		/// NMI source reporting
		pub nmi_src[20] => as bool,
		/// AMX instructions for FP16 numbers
		pub amx_fp16[21] => as bool,
		/// HRESET instruction, `IA32_HRESET_ENABLE` (17DAh) MSR, and Processor History Reset Leaf (EAX=20h)
		pub hreset[22] => as bool,
		/// AVX IFMA instructions
		pub avx_ifma[23] => as bool,
		/// Linear Address Masking
		pub lam[26] => as bool,
		/// RDMSRLIST and WRMSRLIST instructions, and the `IA32_BARRIER` (02Fh) MSR
		pub msrlist[27] => as bool,
		/// If 1, supports INVD instruction execution prevention after BIOS Done.
		pub invd_disable_post_bios_done[30] => as bool,
		/// MOVRS and PREFETCHRST2 instructions supported (memory read/prefetch with read-shared hint)
		pub movrs[31] => as bool,
	}
}

bitstruct! {
	/// Gets the `ebx` register values for the CPUID `eax=07, ecx=1` leaf.
	pub struct Ebx(u32) {
		/// Total Storage Encryption: PBNDKB instruction and `TSE_CAPABILITY` (9F1h) MSR.
		pub pbndkb[1] => as bool,
		/// If 1, then bit 22 of `IA32_MISC_ENABLE` cannot be set to 1 to limit the value returned by `CPUID.(EAX=0):EAX[7:0]`.
		pub cpuid_maxval_lim_rmv[3] => as bool,
	}
}

bitstruct! {
	/// Gets the `ecx` register values for the CPUID `eax=07, ecx=1` leaf.
	pub struct Ecx(u32) {
		/// X86S, cancelled
		pub legacy_reduced_isa[2] => as bool,
		/// 64-bit SIPI (Startup InterProcessor Interrupt) (part of cancelled X86S)
		pub sipi64[4] => as bool,
		/// Immediate forms of the RDMSR and WRMSRNS instructions
		pub msr_imm[5] => as bool,
	}
}

bitstruct! {
	/// Gets the `edx` register values for the CPUID `eax=07, ecx=1` leaf.
	pub struct Edx(u32) {
		/// AVX VNNI INT8 instructions
		pub avx_vnni_int8[4] => as bool,
		/// AVX no-exception FP conversion instructions (bfloat16↔FP32 and FP16→FP32)
		pub avx_ne_convert[5] => as bool,
		/// AMX support for "complex" tiles (`TCMMIMFP16PS` and `TCMMRLFP16PS`)
		pub amx_complex[8] => as bool,
		/// AVX VNNI INT16 instructions
		pub avx_vnni_int16[10] => as bool,
		/// User-timer events: `IA32_UINTR_TIMER` (1B00h) MSR
		pub utmr[13] => as bool,
		/// Instruction-cache prefetch instructions (`PREFETCHIT0` and `PREFETCHIT1`)
		pub prefetchi[14] => as bool,
		/// User-mode MSR access instructions (`URDMSR` and `UWRMSR`)
		pub user_msr[15] => as bool,
		/// UIRET (User Interrupt Return) sets UIF from bit 1 of RFLAGS
		pub uiret_uif_from_rflags[17] => as bool,
		/// If 1, then CET Supervisor Shadow Stacks (SSS) are not prematurely busy
		pub cet_sss[18] => as bool,
		/// AVX10 Converged Vector ISA (see also leaf 24h)
		pub avx10[19] => as bool,
		/// Advanced Performance Extensions, Foundation (adds REX2 and extended EVEX prefix encodings to support 32 GPRs, as well as some new instructions)
		pub apx_f[21] => as bool,
		/// MWAIT instruction
		pub mwait[23] => as bool,
	}
}

/// Extended Features (`ecx=1`)
#[derive(Debug)]
pub struct CpuidA07C1 {
	/// The `eax` register of the cpuid call.
	pub eax: Eax,
	/// The `ebx` register of the cpuid call.
	pub ebx: Ebx,
	/// The `ecx` register of the cpuid call.
	pub ecx: Ecx,
	/// The `edx` register of the cpuid call.
	pub edx: Edx,
}

impl CpuidA07C1 {
	/// Executes CPUID with `eax=7, ecx=1`, which provides additional extended features.
	///
	/// Returns `None` if either CPUID is not supported or the leaf is not supported.
	///
	/// # Performance
	/// This is an incredibly slow and **serializing** operation. If used frequently, its
	/// result should be cached.
	#[must_use]
	pub fn get() -> Option<Self> {
		super::cpuid(0x07, 0x01).map(|r| {
			Self {
				eax: Eax(r.eax),
				ebx: Ebx(r.ebx),
				ecx: Ecx(r.ecx),
				edx: Edx(r.edx),
			}
		})
	}
}
