//! Assembly instruction stubs for the x86_64 architecture.
#![expect(clippy::inline_always)]

use core::arch::asm;

/// Invalidates a single page in the Translation Lookaside Buffer (TLB)
/// given a `virtual_address`.
#[inline(always)]
pub fn invlpg<T>(virtual_address: *const T) {
	unsafe {
		asm!(
			"invlpg [{}]",
			in(reg) virtual_address,
			options(nostack, preserves_flags)
		);
	}
}

/// Flushes the Translation Lookaside Buffer (TLB) for the current CPU.
///
/// This is *very* expensive and should be used sparingly.
///
/// Assumes there's a stack.
#[inline(always)]
pub fn flush_tlb() {
	unsafe {
		asm!(
			// Store and disable the interrupts
			// We do this because there's a race condition where,
			// in a very unlikely event, an interrupt could be
			// triggered between the `mov` instructions and we
			// end up restoring an old `cr3` value. So we
			// disable interrupts to prevent this.
			"pushfq",
			"cli",
			// Read and write back the CR3 value,
			// which triggers a full TLB flush on x86.
			"mov rax, cr3",
			"mov cr3, rax",
			// Restore interrupts.
			"popfq",
			// Mark that we clobbered the `rax` register.
			out("rax") _,
			options(nostack, preserves_flags, nomem)
		);
	}
}

/// Returns whether or not 5-level paging is enabled.
#[inline(always)]
#[must_use]
pub fn is_5_level_paging_enabled() -> bool {
	let cr4: usize;
	unsafe {
		asm!("mov {}, cr4", out(reg) cr4, options(nostack, nomem, preserves_flags));
	}
	cr4 & (1 << 12) != 0
}

/// Returns the current value of the `cr3` register
#[inline(always)]
#[must_use]
pub fn cr3() -> u64 {
	let cr3: u64;
	unsafe {
		asm!("mov {}, cr3", out(reg) cr3, options(nostack, nomem, preserves_flags));
	}
	cr3
}

/// Sets the value of the `cr3` register to `value`.
///
/// # Safety
/// Callers must be prepared for the consequences of changing the
/// page table base address.
#[inline(always)]
pub unsafe fn _set_cr3(value: u64) {
	asm!("mov cr3, {}", in(reg) value, options(nostack, preserves_flags));
}

/// Disables the 8259 PIC by masking off all interrupts.
#[inline(always)]
pub fn disable_8259() {
	unsafe {
		asm!(
			"mov al, 0xFF",
			"out 0x21, al",
			"out 0xA1, al",
			options(nostack, preserves_flags)
		);
	}
}

/// Disables all interrupts.
#[inline(always)]
pub fn disable_interrupts() {
	unsafe {
		asm!("cli", options(nostack, preserves_flags));
	}
}

/// Sends a byte to the specified I/O port.
#[inline(always)]
pub fn outb(port: u16, value: u8) {
	unsafe {
		asm!(
			"out dx, al",
			in("dx") port,
			in("al") value,
			options(nostack, preserves_flags)
		);
	}
}

/// Reads a word from the specified I/O port.
#[inline(always)]
#[must_use]
pub fn inw(port: u16) -> u16 {
	let value: u16;
	unsafe {
		asm!(
			"in ax, dx",
			out("ax") value,
			in("dx") port,
			options(nostack, preserves_flags)
		);
	}
	value
}

/// Reads the current CR4 register value.
#[inline(always)]
#[must_use]
pub fn cr4() -> u64 {
	let cr4: u64;
	unsafe {
		asm!("mov {}, cr4", out(reg) cr4, options(nostack, nomem, preserves_flags));
	}
	cr4
}

/// Halts, indefinitely, the CPU (disabling interrupts).
pub fn hang() -> ! {
	unsafe {
		asm!("cli");
	}
	loop {
		halt_once();
	}
}

/// Halts the CPU once and waits for an interrupt.
pub fn halt_once() {
	unsafe {
		asm!("hlt");
	}
}

/// Performs a strong memory serialization barrier.
#[inline(always)]
pub fn strong_memory_barrier() {
	unsafe {
		asm!("mfence", options(nostack, preserves_flags),);
	}
}
