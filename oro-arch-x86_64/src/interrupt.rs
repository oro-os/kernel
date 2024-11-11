//! Interrupt handling for x86_64 architecture.

use core::arch::{asm, naked_asm};

use oro_mem::mapper::AddressSegment;
use oro_sync::Lock;

use crate::{
	isr_store_user_task_and_jmp,
	lapic::{ApicSvr, ApicTimerConfig, ApicTimerMode},
	mem::address_space::AddressSpaceLayout,
};

/// A single IDT (Interrupt Descriptor Table) entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct IdtEntry {
	/// The lower 16 bits of the ISR (Interrupt Service Routine) address.
	isr_low:    u16,
	/// The code segment selector used when handling the interrupt.
	kernel_cs:  u16,
	/// The IST (Interrupt Stack Table) index.
	ist:        u8,
	/// Attributes and flags for the IDT entry, including type.
	attributes: u8,
	/// The middle 16 bits of the ISR address.
	isr_mid:    u16,
	/// The higher 32 bits of the ISR address.
	isr_high:   u32,
	/// Reserved.
	_reserved:  u32,
}

impl IdtEntry {
	/// Creates a new, empty IDT entry.
	pub const fn new() -> Self {
		Self {
			isr_low:    0,
			kernel_cs:  0,
			ist:        0,
			attributes: 0,
			isr_mid:    0,
			isr_high:   0,
			_reserved:  0,
		}
	}

	/// Sets the ISR address for the IDT entry.
	///
	/// # Safety
	/// Caller must ensure that the given address is
	/// a real function that is suitable for handling
	/// the interrupt.
	pub const unsafe fn with_isr_raw(mut self, isr: u64) -> Self {
		self.isr_low = isr as u16;
		self.isr_mid = (isr >> 16) as u16;
		self.isr_high = (isr >> 32) as u32;

		self
	}

	/// Sets the ISR handler as a function pointer.
	pub fn with_isr(self, isr: unsafe extern "C" fn() -> !) -> Self {
		unsafe { self.with_isr_raw(isr as usize as u64) }
	}

	/// Sets the code segment selector for the IDT entry.
	///
	/// There is no configurable index for this; it's always
	/// set to 0x08 (the kernel code segment).
	pub const fn with_kernel_cs(mut self) -> Self {
		self.kernel_cs = 0x08;
		self
	}

	/// Sets the attributes for the IDT entry.
	pub const fn with_attributes(mut self, attributes: u8) -> Self {
		self.attributes = attributes;
		self
	}
}

/// The IDT (Interrupt Descriptor Table) for the kernel.
static mut IDT: Aligned16<[IdtEntry; 256]> = Aligned16([IdtEntry::new(); 256]);

/// The ISR (Interrupt Service Routine) for the system timer.
#[no_mangle]
unsafe extern "C" fn isr_sys_timer_rust() -> ! {
	// Must be first.
	let irq_stack_ptr: u64;
	asm!("", out("rcx") irq_stack_ptr, options(nostack, preserves_flags));

	let handler = crate::handler::Handler::new();

	let mut coming_from_user = false;
	{
		let scheduler_lock = handler.kernel().scheduler().lock();

		// If this is `None`, then the kernel is currently running.
		// Otherwise it's a userspace task that we just jumped from.
		if let Some(user_task) = scheduler_lock.current_thread().as_ref() {
			user_task.lock().thread_state_mut().irq_stack_ptr = irq_stack_ptr;

			coming_from_user = true;
		} else {
			handler
				.kernel()
				.core()
				.kernel_irq_stack
				.get()
				.write(irq_stack_ptr);
		}

		drop(scheduler_lock);
	}

	handler.kernel().core().lapic.eoi();

	let maybe_user_context = handler
		.kernel()
		.scheduler()
		.lock()
		.event_timer_expired(&handler);

	if let Some(user_ctx) = maybe_user_context {
		let (thread_cr3_phys, thread_rsp) = unsafe {
			let ctx_lock = user_ctx.lock();
			let cr3 = ctx_lock.mapper().base_phys;
			let rsp = ctx_lock.thread_state().irq_stack_ptr;
			(*handler.kernel().core().tss.get())
				.rsp0
				.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
			drop(ctx_lock);
			(cr3, rsp)
		};

		asm! {
			"jmp oro_x86_64_user_to_user",
			in("rax") thread_cr3_phys,
			in("rdx") thread_rsp,
			options(noreturn),
		};
	} else {
		let kernel_irq_stack = handler.kernel().core().kernel_irq_stack.get().read();
		let kernel_stack = handler.kernel().core().kernel_stack.get().read();
		if coming_from_user {
			let kernel_cr3 = handler.kernel().mapper().base_phys;

			asm! {
				"mov cr3, rdx",
				"mov rsp, rcx",
				"jmp oro_x86_64_return_to_kernel",
				in("rcx") kernel_irq_stack,
				in("r9") kernel_stack,
				in("rdx") kernel_cr3,
				options(noreturn),
			};
		} else {
			asm! {
				"mov rsp, rcx",
				"jmp oro_x86_64_return_to_kernel",
				in("rcx") kernel_irq_stack,
				in("r9") kernel_stack,
				options(noreturn),
			};
		}
	}
}

/// The ISR (Interrupt Service Routine) trampoline stub for the system timer.
#[naked]
unsafe extern "C" fn isr_sys_timer() -> ! {
	isr_store_user_task_and_jmp!(isr_sys_timer_rust);
}

/// The ISR (Interrupt Service Routine) for the APIC spurious interrupt.
#[no_mangle]
unsafe extern "C" fn isr_apic_svr_rust() {
	crate::Kernel::get().core().lapic.eoi();
}

/// The ISR (Interrupt Service Routine) trampoline stub for the APIC spurious interrupt.
#[naked]
unsafe extern "C" fn isr_apic_svr() -> ! {
	naked_asm!("cli", "jmp isr_apic_svr_rust");
}

/// Aligns a `T` value to 16 bytes.
#[repr(C, align(16))]
struct Aligned16<T: Sized>(pub T);

/// The vector for the main system timer interrupt.
const TIMER_VECTOR: u8 = 32;
/// The vector for the APIC spurious interrupt.
const APIC_SVR_VECTOR: u8 = 255;

/// Installs the IDT (Interrupt Descriptor Table) for the kernel
/// and enables interrupts.
///
/// # Safety
/// Modifies global state, and must be called only once.
///
/// The kernel MUST be fully initialized before calling this function.
pub unsafe fn install_idt() {
	// Get the LAPIC.
	let lapic = &crate::Kernel::get().core().lapic;

	/// The IDTR (Interrupt Descriptor Table Register) structure,
	/// read in by the `lidt` instruction.
	#[repr(C, packed)]
	struct Idtr {
		/// How long the IDT is in bytes, minus 1.
		limit: u16,
		/// The base address of the IDT.
		base:  *const IdtEntry,
	}

	#[allow(static_mut_refs)]
	let idtr = Idtr {
		limit: (core::mem::size_of_val(&IDT) - 1) as u16,
		base:  IDT.0.as_ptr(),
	};

	asm!(
		"lidt [{}]",
		in(reg) &idtr,
		options(nostack, preserves_flags)
	);

	// Set up the main system timer.
	IDT.0[usize::from(TIMER_VECTOR)] = IdtEntry::new()
		.with_kernel_cs()
		.with_attributes(0x8E)
		.with_isr(isr_sys_timer);

	lapic.set_timer_divider(crate::lapic::ApicTimerDivideBy::Div128);

	// Note: this also enables the timer interrupts
	lapic.configure_timer(
		ApicTimerConfig::new()
			.with_vector(TIMER_VECTOR)
			.with_mode(ApicTimerMode::OneShot),
	);

	// Set up the APIC spurious interrupt.
	// This also enables the APIC if it isn't already.
	IDT.0[usize::from(APIC_SVR_VECTOR)] = IdtEntry::new()
		.with_kernel_cs()
		.with_attributes(0x8E)
		.with_isr(isr_apic_svr);

	lapic.set_spurious_vector(
		ApicSvr::new()
			.with_vector(APIC_SVR_VECTOR)
			.with_software_enable(),
	);
}
