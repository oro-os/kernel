//! Assembly instruction stubs for the x86_64 architecture.
#![expect(clippy::inline_always, unused_unsafe)]

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

/// Returns the current value of the `cr2` register
#[inline(always)]
#[must_use]
pub fn cr2() -> u64 {
	let cr2: u64;
	unsafe {
		asm!("mov {}, cr2", out(reg) cr2, options(nostack, nomem, preserves_flags));
	}
	cr2
}

/// Disables the 8259 PIC by masking off all interrupts.
///
/// # Safety
/// If `disconnect_imcr` is true, the IMCR (Interrupt Mode Control Register)
/// must have been detected beforehand. Calling this function with `true`
/// when the IMCR is not present is undefined behavior.
#[inline(always)]
pub unsafe fn disable_8259(disconnect_imcr: bool) {
	// SAFETY: This is always safe.
	unsafe {
		outb(0x21, 0xFF); // Mask interrupt vectors 0-7
		outb(0xA1, 0xFF); // Mask interrupt vectors 8-15
	}

	if disconnect_imcr {
		// SAFETY: Safety is offloaded to caller.
		unsafe {
			// See "Intel MultiProcessor Specification Version 1.4 (1997)"
			// page 3-8 for more information.
			outb(0x22, 0x70); // Select IMCR
			outb(0x23, 0x01); // Disconnect
		}
	}
}

/// Disables all interrupts.
#[inline(always)]
pub fn disable_interrupts() {
	unsafe {
		asm!("cli", options(nostack, preserves_flags));
	}
}

/// Enables all interrupts.
#[inline(always)]
pub fn enable_interrupts() {
	unsafe {
		asm!("sti", options(nostack, preserves_flags));
	}
}

/// Returns whether or not interrupts are enabled.
#[inline]
#[must_use]
pub fn interrupts_enabled() -> bool {
	let flags: u64;
	unsafe {
		asm!("pushfq", "pop rax", out("rax") flags, options(nostack, preserves_flags));
	}
	flags & (1 << 9) != 0
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

/// Performs a strong memory serialization fence.
#[inline(always)]
pub fn strong_memory_fence() {
	unsafe {
		asm!("mfence", options(nostack, preserves_flags),);
	}
}

/// Reads the value of an MSR
#[inline(always)]
#[must_use]
pub fn rdmsr(msr: u32) -> u64 {
	let val_a: u32;
	let val_d: u32;
	unsafe {
		asm!(
			"rdmsr",
			in("ecx") msr,
			out("eax") val_a,
			out("edx") val_d,
			options(nostack, preserves_flags)
		);
	}

	(u64::from(val_d) << 32) | u64::from(val_a)
}

/// Writes a value to an MSR
#[inline(always)]
pub fn wrmsr(msr: u32, value: u64) {
	let val_a = value as u32;
	let val_d = (value >> 32) as u32;
	unsafe {
		asm!(
			"wrmsr",
			in("ecx") msr,
			in("eax") val_a,
			in("edx") val_d,
			options(nostack, preserves_flags)
		);
	}
}

/// Loads (sets) the given GDT offset as the TSS (Task State Segment) for the current core.
#[inline(always)]
pub fn load_tss(offset: u16) {
	unsafe {
		asm!(
			"ltr ax",
			in("ax") offset,
			options(nostack, preserves_flags)
		);
	}
}

/// Returns the current RFLAGS value
#[inline(always)]
#[must_use]
pub fn rflags() -> u64 {
	let rflags: u64;
	unsafe {
		asm!("pushfq", "pop rax", out("rax") rflags, options(nostack, preserves_flags));
	}
	rflags
}

/// Sets the FS base pointer MSR to the given `value`.
#[inline(always)]
pub fn set_fs_msr(value: u64) {
	wrmsr(0xC000_0100, value);
}

/// Sets the GS base pointer MSR to the given `value`.
#[inline(always)]
pub fn set_gs_msr(value: u64) {
	wrmsr(0xC000_0101, value);
}

/// Gets the FS base pointer MSR.
#[inline(always)]
#[must_use]
pub fn get_fs_msr() -> u64 {
	rdmsr(0xC000_0100)
}

/// Gets the GS base pointer MSR.
#[inline(always)]
#[must_use]
pub fn get_gs_msr() -> u64 {
	rdmsr(0xC000_0101)
}
