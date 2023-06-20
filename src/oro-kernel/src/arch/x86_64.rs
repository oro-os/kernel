use lazy_static::lazy_static;
use oro_boot::x86_64 as boot;
use spin::mutex::{spin::SpinMutex, ticket::TicketMutex};
use uart_16550::SerialPort;
use x86_64::{
	structures::{
		gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
		idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
		tss::TaskStateSegment,
	},
	VirtAddr,
};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
	static ref SERIAL: SpinMutex<SerialPort> = {
		let mut serial_port = unsafe { SerialPort::new(0x3F8) };
		serial_port.init();
		SpinMutex::new(serial_port)
	};
	static ref IDT: InterruptDescriptorTable = {
		let mut idt = InterruptDescriptorTable::new();
		idt.page_fault.set_handler_fn(irq_page_fault);
		idt.breakpoint.set_handler_fn(irq_breakpoint);
		idt
	};
	static ref TSS: TaskStateSegment = {
		let mut tss = TaskStateSegment::new();
		tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
			const STACK_SIZE: usize = 4096 * 5;
			static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

			let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
			stack_start + STACK_SIZE
		};
		tss
	};
	static ref GDT: (GlobalDescriptorTable, Selectors) = {
		let mut gdt = GlobalDescriptorTable::new();
		let cs = gdt.add_entry(Descriptor::kernel_code_segment());
		let tss = gdt.add_entry(Descriptor::tss_segment(&TSS));
		(gdt, Selectors { cs, tss })
	};
}

struct Selectors {
	cs: SegmentSelector,
	tss: SegmentSelector,
}

pub unsafe fn halt() -> ! {
	use core::arch::asm;
	asm!("cli");
	loop {
		asm!("hlt");
	}
}

pub fn print_args(args: core::fmt::Arguments) {
	use core::fmt::Write;
	SERIAL.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
	($($arg:tt)*) => ($crate::arch::print_args(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
	() => ($crate::print!("\n"));
	($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

extern "x86-interrupt" fn irq_page_fault(_frm: InterruptStackFrame, _err_code: PageFaultErrorCode) {
	use core::fmt::Write;
	unsafe {
		SERIAL.force_unlock();
	}
	SERIAL.lock().write_str("PAGE FAULT").unwrap();
	unsafe {
		halt();
	}
}

extern "x86-interrupt" fn irq_breakpoint(_frm: InterruptStackFrame) {
	use core::fmt::Write;
	unsafe {
		SERIAL.force_unlock();
	}
	SERIAL.lock().write_str("BREAKPOINT").unwrap();
	unsafe {
		halt();
	}
}

pub fn init() {
	use x86_64::instructions::segmentation::{Segment, CS};
	use x86_64::instructions::tables::load_tss;

	GDT.0.load();
	unsafe {
		CS::set_reg(GDT.1.cs);
		load_tss(GDT.1.tss);
	}
	IDT.load();

	let boot_config = unsafe {
		use oro_boot::{x86_64 as boot, Fake, Proxy};

		&*(boot::l4_to_range_48(boot::ORO_BOOT_PAGE_TABLE_INDEX).0
			as *const Proxy![boot::BootConfig<Fake<boot::MemoryRegion>>])
	};

	// Validate the magic number
	if boot_config.magic != oro_boot::BOOT_MAGIC {
		panic!("boot error (kernel): boot config magic number mismatch");
	}
	if boot_config.nonce_xor_magic != (oro_boot::BOOT_MAGIC ^ boot_config.nonce) {
		panic!("boot error (kernel): boot config magic^nonce mismatch");
	}
}
