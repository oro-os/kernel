//! Interrupt handling for x86_64 architecture.
#![expect(rustdoc::private_intra_doc_links)]

use core::{
	arch::{global_asm, naked_asm},
	cell::UnsafeCell,
	mem::MaybeUninit,
};

use idt::IdtEntry;
use oro_kernel::event::{InvalidInstruction, PageFault, PageFaultAccess, PreemptionEvent};
use oro_macro::paste;
use oro_sync::{Lock, Mutex};

use crate::{
	lapic::{ApicSvr, ApicTimerConfig, ApicTimerMode},
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
};

/// The vector number for the APIC spurious interrupt.
const APIC_SVR_VECTOR: u8 = 0xFF;
/// The vector number for the system timer interrupt.
const TIMER_VECTOR: u8 = 0x20;

pub mod idt;

/// The static IDT, used by all cores.
static IDT: Mutex<Option<UnsafeCell<[idt::IdtEntry; 256]>>> = Mutex::new(None);

/// The level of vector preservation that needs to occur
/// for the current CPU's capabilities.
#[expect(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorPreservation {
	/// AVX-512 registers (ZMMn)
	Zmm,
	/// AVX2 registers (YMMn)
	Ymm,
	/// MMX registers (XMMn)
	Xmm,
	/// No vectors
	None,
}

/// Returns the level of vector preservation that is needed for this CPU.
///
/// # Performance
/// Callers should consider this to be quite expensive.
#[cold]
fn get_vector_preservation() -> VectorPreservation {
	// TODO(qix-): do actual detection
	VectorPreservation::None
}

/// Initializes and installs the interrupt handling for the x86_64 architecture.
///
/// # Safety
/// See [`idt::install_idt`] for safety considerations.
#[expect(clippy::missing_panics_doc)]
pub unsafe fn install() {
	let mut idt = IDT.lock();

	if idt.is_none() {
		// Determine the level of vector register preservation we need.
		let vector_preservation = get_vector_preservation();

		macro_rules! isr {
			(@@@ $vec_nr:tt $(, exc $exception:tt)? $(, push $push_err:literal)? $(,)?) => {{
				paste! {
					#[naked]
					#[unsafe(no_mangle)]
					extern "C" fn _oro_isr_handler_ %% $vec_nr %% _zmm () -> ! {
						// SAFETY: inherently unsafe.
						unsafe {
							naked_asm! {
								"cli", "cld",
								$($push_err,)?
								concat!("push ", stringify!($vec_nr)),
								concat!("jmp _oro_isr_common", $($exception,)? "_zmm"),
							}
						}
					}

					#[naked]
					#[unsafe(no_mangle)]
					extern "C" fn _oro_isr_handler_ %% $vec_nr %% _ymm () -> ! {
						// SAFETY: inherently unsafe.
						unsafe {
							naked_asm! {
								"cli", "cld",
								$($push_err,)?
								concat!("push ", stringify!($vec_nr)),
								concat!("jmp _oro_isr_common", $($exception,)? "_ymm"),
							}
						}
					}

					#[naked]
					#[unsafe(no_mangle)]
					extern "C" fn _oro_isr_handler_ %% $vec_nr %% _xmm () -> ! {
						// SAFETY: inherently unsafe.
						unsafe {
							naked_asm! {
								"cli", "cld",
								$($push_err,)?
								concat!("push ", stringify!($vec_nr)),
								concat!("jmp _oro_isr_common", $($exception,)? "_xmm"),
							}
						}
					}

					#[naked]
					#[unsafe(no_mangle)]
					extern "C" fn _oro_isr_handler_ %% $vec_nr %% _novec () -> ! {
						// SAFETY: inherently unsafe.
						unsafe {
							naked_asm! {
								"cli", "cld",
								$($push_err,)?
								concat!("push ", stringify!($vec_nr)),
								concat!("jmp _oro_isr_common", $($exception,)? "_novec"),
							}
						}
					}

					IdtEntry::new().with_kernel_cs().with_attributes(0x8E).with_isr(
						match vector_preservation {
							VectorPreservation::Zmm => _oro_isr_handler_ %% $vec_nr %% _zmm,
							VectorPreservation::Ymm => _oro_isr_handler_ %% $vec_nr %% _ymm,
							VectorPreservation::Xmm => _oro_isr_handler_ %% $vec_nr %% _xmm,
							VectorPreservation::None => _oro_isr_handler_ %% $vec_nr %% _novec,
						}
					)
				}
			}};

			(@exception @error $vec_nr:tt) => {
				isr!(@@@ $vec_nr, exc "_exc")
			};

			(@exception $vec_nr:tt) => {
				isr!(@@@ $vec_nr, exc "_exc", push "push 0")
			};

			($vec_nr:tt) => {
				isr!(@@@ $vec_nr, push "push 0")
			};
		}

		#[rustfmt::skip]
		let new_idt: [IdtEntry; 256] = [
			// Divide by zero
			isr!(@exception 0),
			// Debug
			isr!(@exception 1),
			// NMI
			isr!(@exception 2),
			// Breakpoint
			isr!(@exception 3),
			// Overflow
			isr!(@exception 4),
			// Bound range exceeded
			isr!(@exception 5),
			// Invalid opcode
			isr!(@exception 6),
			// Device not available
			isr!(@exception 7),
			// Double fault
			isr!(@exception @error 8),
			// Coprocessor segment overrun
			isr!(@exception 9),
			// Invalid TSS
			isr!(@exception @error 10),
			// Segment not present
			isr!(@exception @error 11),
			// Stack-segment fault
			isr!(@exception @error 12),
			// General protection fault
			isr!(@exception @error 13),
			// Page fault
			isr!(@exception @error 14),
			// Reserved
			isr!(@exception 15),
			// x87 FPU floating-point error
			isr!(@exception 16),
			// Alignment check
			isr!(@exception @error 17),
			// Machine check
			isr!(@exception 18),
			// SIMD floating-point exception
			isr!(@exception 19),
			// Virtualization exception
			isr!(@exception 20),
			// Control protection exception
			isr!(@exception @error 21),
			// Reserved
			isr!(@exception 22),
			// Reserved
			isr!(@exception 23),
			// Reserved
			isr!(@exception 24),
			// Reserved
			isr!(@exception 25),
			// Reserved
			isr!(@exception 26),
			// Reserved
			isr!(@exception 27),
			// Hypervisor injection exception
			isr!(@exception 28),
			// VMM communication exception
			isr!(@exception @error 29),
			// Security exception
			isr!(@exception @error 30),
			// Reserved
			isr!(@exception 31),

			// NOTE(qix-): I tried. Really, I did. I tried to make a macro that repeated this, but
			// NOTE(qix-): Rust worked against me. Something about hygiene, not that I know anything
			// NOTE(qix-): about hygiene.
			// NOTE(qix-):
			// NOTE(qix-): Time wasted: ~3 hours.
			isr!(32), isr!(33), isr!(34), isr!(35), isr!(36), isr!(37),
			isr!(38), isr!(39), isr!(40), isr!(41), isr!(42), isr!(43),
			isr!(44), isr!(45), isr!(46), isr!(47), isr!(48), isr!(49),
			isr!(50), isr!(51), isr!(52), isr!(53), isr!(54), isr!(55),
			isr!(56), isr!(57), isr!(58), isr!(59), isr!(60), isr!(61),
			isr!(62), isr!(63), isr!(64), isr!(65), isr!(66), isr!(67),
			isr!(68), isr!(69), isr!(70), isr!(71), isr!(72), isr!(73),
			isr!(74), isr!(75), isr!(76), isr!(77), isr!(78), isr!(79),
			isr!(80), isr!(81), isr!(82), isr!(83), isr!(84), isr!(85),
			isr!(86), isr!(87), isr!(88), isr!(89), isr!(90), isr!(91),
			isr!(92), isr!(93), isr!(94), isr!(95), isr!(96), isr!(97),
			isr!(98), isr!(99), isr!(100), isr!(101), isr!(102), isr!(103),
			isr!(104), isr!(105), isr!(106), isr!(107), isr!(108), isr!(109),
			isr!(110), isr!(111), isr!(112), isr!(113), isr!(114), isr!(115),
			isr!(116), isr!(117), isr!(118), isr!(119), isr!(120), isr!(121),
			isr!(122), isr!(123), isr!(124), isr!(125), isr!(126), isr!(127),
			isr!(128), isr!(129), isr!(130), isr!(131), isr!(132), isr!(133),
			isr!(134), isr!(135), isr!(136), isr!(137), isr!(138), isr!(139),
			isr!(140), isr!(141), isr!(142), isr!(143), isr!(144), isr!(145),
			isr!(146), isr!(147), isr!(148), isr!(149), isr!(150), isr!(151),
			isr!(152), isr!(153), isr!(154), isr!(155), isr!(156), isr!(157),
			isr!(158), isr!(159), isr!(160), isr!(161), isr!(162), isr!(163),
			isr!(164), isr!(165), isr!(166), isr!(167), isr!(168), isr!(169),
			isr!(170), isr!(171), isr!(172), isr!(173), isr!(174), isr!(175),
			isr!(176), isr!(177), isr!(178), isr!(179), isr!(180), isr!(181),
			isr!(182), isr!(183), isr!(184), isr!(185), isr!(186), isr!(187),
			isr!(188), isr!(189), isr!(190), isr!(191), isr!(192), isr!(193),
			isr!(194), isr!(195), isr!(196), isr!(197), isr!(198), isr!(199),
			isr!(200), isr!(201), isr!(202), isr!(203), isr!(204), isr!(205),
			isr!(206), isr!(207), isr!(208), isr!(209), isr!(210), isr!(211),
			isr!(212), isr!(213), isr!(214), isr!(215), isr!(216), isr!(217),
			isr!(218), isr!(219), isr!(220), isr!(221), isr!(222), isr!(223),
			isr!(224), isr!(225), isr!(226), isr!(227), isr!(228), isr!(229),
			isr!(230), isr!(231), isr!(232), isr!(233), isr!(234), isr!(235),
			isr!(236), isr!(237), isr!(238), isr!(239), isr!(240), isr!(241),
			isr!(242), isr!(243), isr!(244), isr!(245), isr!(246), isr!(247),
			isr!(248), isr!(249), isr!(250), isr!(251), isr!(252), isr!(253),
			isr!(254), isr!(255),
		];

		core::ptr::write_volatile(&mut *idt, Some(UnsafeCell::new(new_idt)));
	}

	assert!(idt.is_some());

	// SAFETY: We have guaranteed this is valid; we only ever write it once.
	let idt_ref = unsafe { &*idt.as_ref().unwrap().get() };
	drop(idt);

	// SAFETY: Safety considerations offloaded to caller.
	unsafe {
		idt::install_idt(idt_ref);
	}
}

/// Initializes the APIC (Advanced Programmable Interrupt Controller)
/// for interrupt handling.
///
/// # Safety
/// Modifies global state, and must be called only once per core.
///
/// The kernel MUST be fully initialized before calling this function.
pub unsafe fn initialize_lapic_irqs() {
	let lapic = &crate::Kernel::get().handle().lapic;

	lapic.set_spurious_vector(
		ApicSvr::new()
			.with_vector(APIC_SVR_VECTOR)
			.with_software_enable(),
	);

	lapic.set_timer_divider(crate::lapic::ApicTimerDivideBy::Div128);

	lapic.configure_timer(
		ApicTimerConfig::new()
			.with_vector(TIMER_VECTOR)
			.with_mode(ApicTimerMode::OneShot),
	);
}

/// A stack frame for an interrupt handler.
#[expect(missing_docs)]
#[derive(Debug)]
#[repr(C, align(8))]
pub struct StackFrame {
	/// May not be fully initialized; do NOT inspect this data.
	/// It's for the stubs to use, and is a maximum bound for the
	/// size needed with full AVX-512 support.
	pub zmm:    MaybeUninit<[[u64; 8]; 32]>,
	// NOTE(qix-): Following fields MUST total a multiple of 64
	// NOTE(qix-): or else the ZMMn stores will fault.
	pub gsbase: u64,
	pub fsbase: u64,
	pub r15:    u64,
	pub r14:    u64,
	pub r13:    u64,
	pub r12:    u64,
	pub r11:    u64,
	pub r10:    u64,
	pub r9:     u64,
	pub r8:     u64,
	pub rbp:    u64,
	pub rsi:    u64,
	pub rdx:    u64,
	pub rcx:    u64,
	pub rbx:    u64,
	pub rax:    u64,
	pub rdi:    u64,
	pub iv:     u64,
	pub err:    u64,
	pub ip:     u64,
	pub cs:     u64,
	pub flags:  u64,
	pub sp:     u64,
	pub ss:     u64,
}

impl Default for StackFrame {
	#[inline]
	fn default() -> Self {
		// SAFETY: This is safe, as it's all essentially "maybe uninit" anyway.
		// SAFETY: Moreover, all fields are already safely represented by zeros.
		unsafe { core::mem::zeroed() }
	}
}

/// Common entry point for the ISR handlers.
///
/// This function is passed a pointer to the base
/// of the `StackFrame` on the user thread's shadow
/// stack - or, in the event the interrupt came from
/// within the kernel itself, on the kernel's stack.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_handler(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	// SAFETY: This entire thing is inherently unsafe; there's no point
	// SAFETY: in skirting around it, we're glueing low-level CPU code
	// SAFETY: to a higher level Rust kernel.
	unsafe {
		// Make sure the `as` casts won't truncate.
		oro_macro::assert::fits_within::<u64, usize>();

		debug_assert!(stack_ptr.is_aligned());
		let fp = &*stack_ptr;

		debug_assert!(
			((*fp.get()).cs & 3) == 3 || (*fp.get()).iv >= 32,
			"_oro_isr_rust_handler called with kernel exception (core panic handler wasn't called)"
		);

		let kernel = crate::Kernel::get();
		let iv = (*fp.get()).iv;

		if iv >= 32 {
			// Tell the PIC to de-assert.
			kernel.handle().lapic.eoi();
		}

		let preemption_event = match iv {
			// Invalid opcode.
			0x06 => {
				PreemptionEvent::InvalidInstruction(InvalidInstruction {
					ip: (*fp.get()).ip as usize,
				})
			}
			// Page fault.
			0x0E => {
				PreemptionEvent::PageFault(PageFault {
					address: crate::asm::cr2() as usize,
					ip:      Some((*fp.get()).ip as usize),
					access:  {
						let err = (*fp.get()).err;
						if (err & 0b0001_0000) != 0 {
							PageFaultAccess::Execute
						} else if err & 0b0000_0010 != 0 {
							PageFaultAccess::Write
						} else {
							PageFaultAccess::Read
						}
					},
				})
			}
			// Timer
			0x20 => PreemptionEvent::Timer,
			// Unhandled exception?
			// XXX(qix-): This is temporary
			iv if iv < 32 => {
				todo!(
					"unhandled userspace exception: {:#016X?}",
					&*(*stack_ptr).get()
				);
			}
			iv => PreemptionEvent::Interrupt(iv),
		};

		kernel.handle_event(preemption_event);
	}
}

/// Debug assertion function that is called by the
/// interrupt stubs if the stack is unaligned.
#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_dbg_stack_unaligned(
	got: u64,
	alignment: u64,
	stack_ptr: u64,
	expected: u64,
	rip: u64,
) -> ! {
	use oro_debug::dbg_err;

	dbg_err!(
		"ISR STACK MISALIGNED: modulo={got:#016X}, expected={expected:#016X}, \
		 align={alignment:#016X}, rsp={stack_ptr:#016X}, rip={rip:#016X}"
	);

	if (stack_ptr & 7) != 0 {
		dbg_err!("stack pointer is NOT 64-bit aligned; below values will be garbage");
	}

	let end = AddressSpaceLayout::irq_stack_base(PagingLevel::current_from_cpu()) as u64;
	let start = stack_ptr & !7;

	// SAFETY: Doesn't really matter, this is debugging best-effort, as this
	// SAFETY: is a case of a bug in the kernel.
	unsafe {
		let slice = UnsafeCell::new(::core::slice::from_raw_parts(
			start as *const u64,
			((end - start) >> 3) as usize,
		));
		let slice_ref = &*slice.get();

		dbg_err!("    BEGIN STACK");
		for (i, v) in slice_ref.iter().rev().enumerate() {
			dbg_err!("    {:016X}: {v:016X}", i * 8);
		}
		dbg_err!("    END STACK");
	}

	panic!("ISR stack misaligned")
}

/// Core panic.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_core_panic(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	let _ = stack_ptr; // NOTE(qix-): Marked as unused on release modes.

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

		oro_debug::dbg_err!("unhandled exception; core is about to panic");
		// SAFETY: We have to assume it's valid.
		let fr = unsafe { &*(*stack_ptr).get() };

		macro_rules! log_field {
			($label:literal, $f:ident) => {
				let _ = oro_debug::DebugWriter.write_str(concat!("\n", $label, ":\t"));
				log_hex!(fr.$f);
			};
		}

		macro_rules! log_var {
			($label:literal, $v:expr) => {
				let _ = oro_debug::DebugWriter.write_str(concat!("\n", $label, ":\t"));
				log_hex!($v);
			};
		}

		let cr0: u64 = crate::reg::Cr0::load().into();
		let cr2: u64 = crate::asm::cr2();
		let cr3: u64 = crate::asm::cr3();
		let cr4: u64 = crate::reg::Cr4::load().into();
		let lapic_id_u8 = crate::cpuid::CpuidA01C0B::get().map(|v| v.local_apic_id());

		log_field!("IV", iv);
		log_field!("IP", ip);
		log_field!("SP", sp);
		log_field!("CS", cs);
		log_field!("SS", ss);
		log_field!("ERR", err);
		log_field!("FLAGS", flags);
		log_var!("CR0", cr0);
		log_var!("CR2", cr2);
		log_var!("CR3", cr3);
		log_var!("CR4", cr4);
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
		if let Some(lapic_id_u8) = lapic_id_u8 {
			let lapic_id = u64::from(lapic_id_u8);
			log_var!("LAPIC ID (<=255)", lapic_id);
		} else {
			let _ = oro_debug::DebugWriter.write_str("\nLAPIC ID (<=255):\t(unknown)");
		}

		let _ = oro_debug::DebugWriter.write_str("\n\nEND OF CORE DUMP\n");
	}

	// SAFETY: Not much we can do here anyway.
	panic!("core panicked");
}

/// Performs an `iret` into userspace code.
///
/// This function **does** modify the local core's
/// TSS pointers to point to the stack frame base
/// on DPL=3 -> DPL=0 code.
///
/// # Safety
/// The given task context MUST be ready for a context switch,
/// must NOT be run anywhere else, and the CPU must be ready
/// to receive interrupts (kernel initialized, IDT installed, etc).
///
/// This function **may not** be used to switch into kernel (ring 0)
/// code.
///
/// **All locks or other stack-based stateful objects must be destroyed
/// prior to this function being called.** The kernel is entirely
/// destroyed when this function is called.
#[inline]
pub unsafe fn iret_context(cr3: u64) -> ! {
	unsafe extern "C" {
		#[link_name = "_oro_isr_iret_zmm"]
		fn oro_isr_iret_zmm(cr3: u64, irq_frame_base: u64) -> !;
		#[link_name = "_oro_isr_iret_ymm"]
		fn oro_isr_iret_ymm(cr3: u64, irq_frame_base: u64) -> !;
		#[link_name = "_oro_isr_iret_xmm"]
		fn oro_isr_iret_xmm(cr3: u64, irq_frame_base: u64) -> !;
		#[link_name = "_oro_isr_iret_novec"]
		fn oro_isr_iret_novec(cr3: u64, irq_frame_base: u64) -> !;
	}

	let irq_stack_base = AddressSpaceLayout::irq_stack_base(PagingLevel::current_from_cpu()) as u64;

	// SAFETY: We can guarantee that we're the only users of this handle
	// SAFETY: given that `Kernel` handles are core-local.
	unsafe {
		(*crate::Kernel::get().handle().tss.get())
			.rsp0
			.write(irq_stack_base);
	}

	let irq_frame_base = irq_stack_base - core::mem::size_of::<StackFrame>() as u64;

	let vector_preservation = get_vector_preservation();
	match vector_preservation {
		VectorPreservation::Zmm => oro_isr_iret_zmm(cr3, irq_frame_base),
		VectorPreservation::Ymm => oro_isr_iret_ymm(cr3, irq_frame_base),
		VectorPreservation::Xmm => oro_isr_iret_xmm(cr3, irq_frame_base),
		VectorPreservation::None => oro_isr_iret_novec(cr3, irq_frame_base),
	}
}

#[doc(hidden)]
#[cfg(debug_assertions)]
macro_rules! define_all_handlers {
	() => {
		"DEFINE_ALL_HANDLERS CHECK_STACK_ALIGNMENT_DEBUG"
	};
}
#[doc(hidden)]
#[cfg(not(debug_assertions))]
macro_rules! define_all_handlers {
	() => {
		"DEFINE_ALL_HANDLERS CHECK_STACK_ALIGNMENT_NOOP"
	};
}

global_asm! {
	include_str!("../common-pre.S"),
	include_str!("./isr64.S"),
	define_all_handlers!(),
	include_str!("../common-post.S"),
	CS_OFFSET = const core::mem::offset_of!(StackFrame, cs),
	KERNEL_STACK_BASE_L4 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level4),
	KERNEL_STACK_BASE_L5 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level5),
}
