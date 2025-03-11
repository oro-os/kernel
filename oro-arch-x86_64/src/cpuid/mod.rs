//! Implements various CPUID lookup structures and functions.
#![expect(clippy::similar_names)]

mod a01c0;
mod a07c0;
mod a07c1;
mod a07c2;
mod a0dc0;

pub use a0dc0::CpuidA0DC0;
pub use a01c0::CpuidA01C0;
pub use a07c0::CpuidA07C0;
pub use a07c1::CpuidA07C1;
pub use a07c2::CpuidA07C2;

/// Determines if the CPU supports the `CPUID` instruction.
#[must_use]
#[cold]
fn has_cpuid() -> bool {
	let r: u64;
	unsafe {
		core::arch::asm!(
			"pushfq",
			"pushfq",
			"xor dword ptr [rsp], 0x200000",
			"popfq",
			"pushfq",
			"pop rax",
			"xor rax, [rsp]",
			"popfq",
			"and rax, 0x200000",
			out("rax") r,
			options(nostack, preserves_flags),
		);
	}
	r != 0
}

/// Returns the highest supported CPUID leaf.
///
/// Returns `None` if the CPU does not support the `CPUID` instruction.
#[cold]
#[must_use]
fn highest_leaf() -> Option<u32> {
	if !has_cpuid() {
		return None;
	}

	let r: u32;
	// SAFETY: This is always safe.
	unsafe {
		core::arch::asm!(
			// NOTE(qix-): LLVM uses `rbx` internally, so we have to preserve it.
			// NOTE(qix-): Makes CPUID a bit tricky to deal with.
			"push rbx",
			"mov eax, 0",
			// NOTE(qix-): On older CPUs, unused output registers for `cpuid` were
			// NOTE(qix-): left untouched in some cases and not others, namely on
			// NOTE(qix-): capabilities checks. It's always a good idea to zero them
			// NOTE(qix-): out before calling `cpuid`.
			"xor ecx, ecx",
			"xor edx, edx",
			"cpuid",
			"pop rbx",
			lateout("eax") r,
			out("ecx") _,
			out("edx") _,
			options(nostack, preserves_flags),
		);
	}

	Some(r)
}

/// Performs a CPUID call with the given `eax` and `ecx` registers
/// and returns a [`CpuidResult`].
///
/// Returns `None` if the given `eax` is higher than the maximum
/// supported or if CPUID is not supported.
///
/// # Performance
/// This function is quite slow; it will always check whether CPUID
/// is available, and what the highest leaf is. It's also a **serializing**
/// call, which has other performance implications.
///
/// If the result if a CPUID call isn't expected to change, you should cache
/// it. Note that CPUID might return different results for different cores
/// in the system.
///
/// # Discouraged
/// You probably don't want this function directly; use the helper
/// structures exposed from [`crate::cpuid`].
#[cold]
#[must_use]
pub fn cpuid(mut eax: u32, mut ecx: u32) -> Option<CpuidResult> {
	if highest_leaf()? < eax {
		return None;
	}

	let ebx: u32;
	let edx: u32;
	// SAFETY: This is always safe.
	unsafe {
		core::arch::asm!(
			// NOTE(qix-): LLVM uses `rbx` internally, so we have to preserve it.
			// NOTE(qix-): Makes CPUID a bit tricky to deal with.
			"push rbx",
			// NOTE(qix-): On older CPUs, unused output registers for `cpuid` were
			// NOTE(qix-): left untouched in some cases and not others, namely on
			// NOTE(qix-): capabilities checks. It's always a good idea to zero them
			// NOTE(qix-): out before calling `cpuid`.
			"xor ebx, ebx",
			"xor edx, edx",
			"cpuid",
			"mov rsi, rbx",
			"pop rbx",
			inlateout("eax") eax,
			inlateout("ecx") ecx,
			lateout("edx") edx,
			lateout("esi") ebx,
			options(nostack, preserves_flags),
		);
	}

	Some(CpuidResult { eax, ebx, ecx, edx })
}

/// The result of a CPUID call from [`cpuid`].
///
/// # Discouraged
/// You probably don't want this function directly; use the helper
/// structures exposed from [`crate::cpuid`].
pub struct CpuidResult {
	/// The `eax` register after the CPUID call.
	///
	/// _May have the initial value of `eax` after the call_
	/// if `eax` is not returned by the CPUID leaf.
	pub eax: u32,
	/// The `ebx` register after the CPUID call.
	///
	/// Always zeroed if not returned by CPUID.
	pub ebx: u32,
	/// The `ecx` register after the CPUID call.
	///
	/// _May have the initial value of `ecx` after the call_
	/// if `ecx` is not returned by the CPUID leaf.
	pub ecx: u32,
	/// The `edx` register after the CPUID call.
	///
	/// Always zeroed if not returned by the CPUID leaf.
	pub edx: u32,
}
