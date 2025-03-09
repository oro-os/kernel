//! Implements various CPUID lookup structures and functions.

mod a01c0;
mod a07c0;

pub use a01c0::CpuidA01C0B;
pub use a07c0::CpuidA07C0B;

/// Determines if the CPU supports the `CPUID` instruction.
#[must_use]
pub fn has_cpuid() -> bool {
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
#[must_use]
pub fn highest_leaf() -> Option<u32> {
	if !has_cpuid() {
		return None;
	}

	let r: u32;
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
