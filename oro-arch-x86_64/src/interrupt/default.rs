//! Default, low-level, early-stage interrupt routines
//! for panicking on exceptions.

use core::cell::UnsafeCell;

use oro_debug::dbg;

crate::default_isr_table! {
	/// The default ISR table, used for early stage
	/// interrupt handling and error reporting.
	fn get_default_isr_table() -> &'static [IdtEntry; 32] = [
		// Divide by zero
		noerror,
		// Debug
		noerror,
		// NMI
		noerror,
		// Breakpoint
		noerror,
		// Overflow
		noerror,
		// Bound range exceeded
		noerror,
		// Invalid opcode
		noerror,
		// Device not available
		noerror,
		// Double fault
		error,
		// Coprocessor segment overrun
		noerror,
		// Invalid TSS
		error,
		// Segment not present
		error,
		// Stack-segment fault
		error,
		// General protection fault
		error,
		// Page fault
		error,
		// Reserved
		noerror,
		// x87 FPU floating-point error
		noerror,
		// Alignment check
		error,
		// Machine check
		noerror,
		// SIMD floating-point exception
		noerror,
		// Virtualization exception
		noerror,
		// Control protection exception
		error,
		// Reserved
		noerror,
		// Reserved
		noerror,
		// Reserved
		noerror,
		// Reserved
		noerror,
		// Reserved
		noerror,
		// Reserved
		noerror,
		// Hypervisor injection exception
		noerror,
		// VMM communication exception
		error,
		// Security exception
		error,
		// Reserved
		noerror,
	];
}

/// Installs the default interrupt handlers.
///
/// # Safety
/// Inherently unsafe; modifies the system state.
/// Use with caution.
pub unsafe fn install_default_idt() {
	crate::interrupt::install_idt(get_default_isr_table());
}

/// A stack frame for an interrupt handler.
#[expect(clippy::missing_docs_in_private_items)]
#[derive(Debug)]
#[repr(C)]
struct StackFrame {
	lapic_id_u8: u64,
	r15:         u64,
	r14:         u64,
	r13:         u64,
	r12:         u64,
	r11:         u64,
	r10:         u64,
	r9:          u64,
	r8:          u64,
	rbp:         u64,
	rsi:         u64,
	rdx:         u64,
	rcx:         u64,
	rbx:         u64,
	rax:         u64,
	cr4:         u64,
	cr3:         u64,
	cr2:         u64,
	cr0:         u64,
	rdi:         u64,
	iv:          u64,
	err:         u64,
	ip:          u64,
	cs:          u64,
	flags:       u64,
	sp:          u64,
	ss:          u64,
}

/// Common handler for all interrupts.
#[allow(unused_variables)]
#[inline(never)]
fn handle_interrupt(fr: &'static StackFrame) -> ! {
	dbg!("unhandled exception; core is dead");

	#[cfg(debug_assertions)]
	{
		use core::fmt::Write;

		const HEX: &[u8] = b"0123456789ABCDEF";

		macro_rules! log_hex {
			($v:expr) => {
				let b = [
					HEX[(($v >> 60) & 0xF) as usize],
					HEX[(($v >> 56) & 0xF) as usize],
					HEX[(($v >> 52) & 0xF) as usize],
					HEX[(($v >> 48) & 0xF) as usize],
					HEX[(($v >> 44) & 0xF) as usize],
					HEX[(($v >> 40) & 0xF) as usize],
					HEX[(($v >> 36) & 0xF) as usize],
					HEX[(($v >> 32) & 0xF) as usize],
					HEX[(($v >> 28) & 0xF) as usize],
					HEX[(($v >> 24) & 0xF) as usize],
					HEX[(($v >> 20) & 0xF) as usize],
					HEX[(($v >> 16) & 0xF) as usize],
					HEX[(($v >> 12) & 0xF) as usize],
					HEX[(($v >> 8) & 0xF) as usize],
					HEX[(($v >> 4) & 0xF) as usize],
					HEX[($v & 0xF) as usize],
				];

				// SAFETY: We know the string is valid UTF-8.
				let _ =
					oro_debug::DebugWriter.write_str(unsafe { core::str::from_utf8_unchecked(&b) });
			};
		}

		macro_rules! log_field {
			($label:literal, $f:ident) => {
				let _ = oro_debug::DebugWriter.write_str(concat!("\n", $label, ":\t"));
				log_hex!(fr.$f);
			};
		}

		log_field!("IV", iv);
		log_field!("IP", ip);
		log_field!("SP", sp);
		log_field!("SS", ss);
		log_field!("ERR", err);
		log_field!("FLAGS", flags);
		log_field!("CR0", cr0);
		log_field!("CR2", cr2);
		log_field!("CR3", cr3);
		log_field!("CR4", cr4);
		log_field!("RAX", rax);
		log_field!("RBX", rbx);
		log_field!("RCX", rcx);
		log_field!("RDX", rdx);
		log_field!("RSI", rsi);
		log_field!("RDI", rdi);
		log_field!("RBP", rbp);
		log_field!("R8", r8);
		log_field!("R9", r9);
		log_field!("R10", r10);
		log_field!("R11", r11);
		log_field!("R12", r12);
		log_field!("R13", r13);
		log_field!("R14", r14);
		log_field!("R15", r15);
		log_field!("LAPIC ID (<=255)", lapic_id_u8);

		let _ = oro_debug::DebugWriter.write_str("\n\nEND OF CORE DUMP\n");
	}

	crate::asm::hang();
}

/// The non-error code ISR for the default IDT.
#[unsafe(no_mangle)]
extern "C" fn _oro_default_isr_handler(fr: *const UnsafeCell<StackFrame>) -> ! {
	// SAFETY: The stubs have pushed a valid pointer into arg1.
	handle_interrupt(unsafe { &*(*fr).get() });
}
