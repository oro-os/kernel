//! Implements various CPUID lookup structures and functions.

mod a07c0;

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
