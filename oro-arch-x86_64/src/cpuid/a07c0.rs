//! Implements the CPUID 07:0 lookup structure, _Extended Features (`ecx=0`)_.

use oro_macro::bitstruct;

bitstruct! {
	/// Gets the `ebx` register values for the CPUID `eax=07, ecx=0` leaf.
	pub struct Ebx(u32) {
		/// Whether or not FSGSBASE instructions are supported.
		pub fsgsbase[0] => as bool,
		/// Software Guard Extensions
		pub sgx[2] => as bool,
		/// Bit Manipulation Instruction Set 1
		pub bmi1[3] => as bool,
		/// TSX Hardware Lock Elision
		pub hle[4] => as bool,
		/// Advanced Vector Extensions 2
		pub avx2[5] => as bool,
		/// x87 FPU data pointer register updated on exceptions only
		pub fdb_excptn_only[6] => as bool,
		/// Supervisor Mode Execution Prevention
		pub smep[7] => as bool,
		/// Bit Manipulation Instruction Set 2
		pub bmi2[8] => as bool,
		/// Enhanced `REP MOVSB` / `STOSB`
		pub erms[9] => as bool,
		/// `INVPCID` instruction
		pub invpcid[10] => as bool,
		/// TSX Restricted Transactional Memory
		pub rtm[11] => as bool,
		/// Intel Resource Director (RDT) Monitoring or AMD Platform QOS Monitoring
		pub rdt_m_pqm[12] => as bool,
		/// Intel MPX (Memory Protection Extensions)
		pub mpx[14] => as bool,
		/// Intel Resource Director (RDT) Allocation or AMD Platform QOS Enforcement
		pub rdt_a_pqe[15] => as bool,
		/// AVX-512 Foundation
		pub avx512_f[16] => as bool,
		/// AVX-512 Doubleword and Quadword Instructions
		pub avx512_dq[17] => as bool,
		/// `RDSEED` instruction
		pub rdseed[18] => as bool,
		/// Intel ADX (Multi-Precision Add-Carry Instruction Extensions)
		pub adx[19] => as bool,
		/// Supervisor Mode Access Prevention
		pub smap[20] => as bool,
		/// AVX-512 Integer Fused Multiply-Add Instructions
		pub avx512_ifma[21] => as bool,
		/// `PCOMMIT` instruction (deprecated)
		pub pcommit[22] => as bool,
		/// `CLFLUSHOPT` instruction
		pub clflushopt[23] => as bool,
		/// `CLWB` (Cache line writeback) instruction
		pub clwb[24] => as bool,
		/// Intel Processor Trace
		pub pt[25] => as bool,
		/// AVX-512 Prefetch Instructions
		pub avx512_pf[26] => as bool,
		/// AVX-512 Exponential and Reciprocal Instructions
		pub avx512_er[27] => as bool,
		/// AVX-512 Conflict Detection Instructions
		pub avx512_cd[28] => as bool,
		/// SHA-1 and SHA-256 extensions
		pub sha[29] => as bool,
		/// AVX-512 Byte and Word Instructions
		pub avx512_bw[30] => as bool,
		/// AVX-512 Vector Length Extensions
		pub avx512_vl[31] => as bool,
	}
}

bitstruct! {
	/// Gets the `ecx` register values for the CPUID `eax=07, ecx=0` leaf.
	pub struct Ecx(u32) {
		/// `PREFETCHWT1` instruction
		pub prefetchwt1[0] => as bool,
		/// AVX-512 Vector Bit Manipulation Instructions
		pub avx512_vbmi[1] => as bool,
		/// User-mode Instruction Prevention
		pub umip[2] => as bool,
		/// Memory Protection Keys for User-mode pages
		pub pku[3] => as bool,
		/// PKU enabled by OS
		pub ospke[4] => as bool,
		/// Timed pause and user-level monitor/wait instructions (TPAUSE, UMONITOR, UMWAIT)
		pub waitpkg[5] => as bool,
		/// Control flow enforcement (CET): shadow stack (SHSTK alternative name)
		pub cet_ss_shstk[7] => as bool,
		/// Galois Field instructions
		pub gfni[8] => as bool,
		/// Vector AES instruction set
		pub vaes[9] => as bool,
		/// CLMUL instruction set (VEX-256/EVEX)
		pub vpclmulqdq[10] => as bool,
		/// AVX-512 Vector Neural Network Instructions
		pub avx512_vnni[11] => as bool,
		/// AVX-512 BITALG instructions
		pub avx512_bitalg[12] => as bool,
		/// Total Memory Encryption MSRs available
		pub tme_en[13] => as bool,
		/// AVX-512 Vector Population Count Double and Quad-word
		pub avx512_vpopcntdq[14] => as bool,
		/// 5-level paging (57 address bits)
		pub la57[16] => as bool,
		/// `RDPID` (Read Processor ID) instruction and IA32_TSC_AUX MSR
		pub rdpid[22] => as bool,
		/// AES Key Locker
		pub kl[23] => as bool,
		/// Bus lock debug exceptions
		pub bus_lock_detect[24] => as bool,
		/// `CLDEMOTE` (Cache line demote) instruction
		pub cldemote[25] => as bool,
		/// `MOVDIRI` instruction
		pub movdiri[27] => as bool,
		/// `MOVDIR64B` (64-byte direct store) instruction
		pub movdir64b[28] => as bool,
		/// Enqueue Stores and `EMQCMD`/`EMQCMDS` instructions
		pub enqcmd[29] => as bool,
		/// `SGX` Launch Configuration
		pub sgx_lc[30] => as bool,
		/// Protection keys for supervisor-mode pages
		pub pks[31] => as bool,
	}
}

bitstruct! {
	/// Gets the `edx` register values for the CPUID `eax=07, ecx=0` leaf.
	pub struct Edx(u32) {
		/// Attestation Services for Intel SGX
		pub sgx_keys[1] => as bool,
		/// AVX-512 4-register Neural Network Instructions
		pub avx512_4vnniw[2] => as bool,
		/// AVX-512 4-register Multiply Accumulation Single precision
		pub avx512_4fmaps[3] => as bool,
		/// Fast Short `REP MOVSB`
		pub fsrm[4] => as bool,
		/// User Inter-processor Interrupts
		pub uintr[5] => as bool,
		/// AVX-512 vector intersection instructions on 32/64-bit integers
		pub avx512_vp2intersect[8] => as bool,
		/// Special Register Buffer Data Sampling Mitigations
		pub srbds_ctrl[9] => as bool,
		/// `VERW` instruction clears CPU buffers
		pub md_clear[10] => as bool,
		/// All TSX transactions are aborted
		pub rtm_always_abort[11] => as bool,
		/// TSX_FORCE_ABORT (MSR `0x10f`) is available
		pub rtm_force_abort[13] => as bool,
		/// `SERIALIZE` instruction
		pub serialize[14] => as bool,
		/// Mixture of CPU types in processor topology (e.g. Alder Lake)
		pub hybrid[15] => as bool,
		/// TSX load address tracking suspend/resume instructions (`TSUSLDTRK` and `TRESLDTRK`)
		pub tsxldtrk[16] => as bool,
		/// Platform configuration (Memory Encryption Technologies Instructions)
		pub pconfig[18] => as bool,
		/// Architectural Last Branch Records
		pub lbr[19] => as bool,
		/// Control flow enforcement (CET): indirect branch tracking
		pub cet_ibt[20] => as bool,
		/// AMX tile computation on bfloat16 numbers
		pub amx_bf16[22] => as bool,
		/// AMX tile load/store instructions
		pub amx_tile[24] => as bool,
		/// AMX tile computation on 8-bit integers
		pub amx_int8[25] => as bool,
		/// Speculation Control, part of IBC: Indirect Branch Restricted Speculation (IBRS) and Indirect Branch Prediction Barrier (IBPB)
		pub ibrs_spec_ctrl[26] => as bool,
		/// Single Thread Indirect Branch Predictor, part of IBC
		pub stibp[27] => as bool,
		/// IA32_FLUSH_CMD MSR
		pub l1d_flush[28] => as bool,
		/// Speculative Store Bypass Disable, as mitigation for Speculative Store Bypass (IA32_SPEC_CTRL)
		pub ssbd[31] => as bool,
	}
}

/// Extended Features (`ecx=0`)
#[derive(Debug)]
pub struct CpuidA07C0 {
	/// The `ebx` register of the cpuid call.
	pub ebx: Ebx,
	/// The `ecx` register of the cpuid call.
	pub ecx: Ecx,
	/// The `edx` register of the cpuid call.
	pub edx: Edx,
}

impl CpuidA07C0 {
	/// Executes CPUID with `eax=7, ecx=0`, which provides extended features
	/// about the CPU.
	///
	/// Returns `None` if either `cpuid` is not supported or the leaf is not supported.
	///
	/// # Performance
	/// This is an incredibly slow and **serializing** operation. If used frequently, its
	/// result should be cached.
	#[must_use]
	pub fn get() -> Option<Self> {
		super::cpuid(0x07, 0x00).map(|r| {
			Self {
				ebx: Ebx(r.ebx),
				ecx: Ecx(r.ecx),
				edx: Edx(r.edx),
			}
		})
	}
}
