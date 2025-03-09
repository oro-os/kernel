//! Interrupt handling for x86_64 architecture.
#![expect(rustdoc::private_intra_doc_links)]

use core::{
	arch::{global_asm, naked_asm},
	cell::UnsafeCell,
	mem::MaybeUninit,
};

use idt::IdtEntry;
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
#[expect(clippy::missing_docs_in_private_items)]
#[derive(Debug)]
#[repr(C, align(8))]
struct StackFrame {
	/// May not be fully initialized; do NOT inspect this data.
	/// It's for the stubs to use, and is a maximum bound for the
	/// size needed with full AVX-512 support.
	zmm:    MaybeUninit<[[u64; 8]; 32]>,
	// NOTE(qix-): Following fields MUST total a multiple of 64
	// NOTE(qix-): or else the ZMMn stores will fault.
	gsbase: u64,
	fsbase: u64,
	r15:    u64,
	r14:    u64,
	r13:    u64,
	r12:    u64,
	r11:    u64,
	r10:    u64,
	r9:     u64,
	r8:     u64,
	rbp:    u64,
	rsi:    u64,
	rdx:    u64,
	rcx:    u64,
	rbx:    u64,
	rax:    u64,
	rdi:    u64,
	iv:     u64,
	err:    u64,
	ip:     u64,
	cs:     u64,
	flags:  u64,
	sp:     u64,
	ss:     u64,
}

/// Common entry point for the ISR handlers.
///
/// This function is passed a pointer to the base
/// of the `StackFrame` on the user thread's shadow
/// stack - or, in the event the interrupt came from
/// within the kernel itself, on the kernel's stack.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_handler(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	todo!("oro_isr_rust_handler: {:#016X?}", unsafe {
		&*(*stack_ptr).get()
	});
}

/// Debug assertion function that is called by the
/// interrupt stubs if the stack is unaligned.
#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_dbg_stack_unaligned(got: u64, expected: u64, stack_ptr: u64) -> ! {
	panic!(
		"CORE PANIC - ISR STACK MISALIGNED: modulo={got:#016X}, expected={expected:#016X}, \
		 rsp={stack_ptr:#016X}"
	);
}

/// Core panic.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_core_panic(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	#[cfg(feature = "simple_core_dump")]
	{
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
					let _ = oro_debug::DebugWriter
						.write_str(unsafe { core::str::from_utf8_unchecked(&b) });
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

	#[cfg(not(feature = "simple_core_dump"))]
	{
		// SAFETY: Not much we can do here anyway.
		panic!("core panicked: {:#?}", unsafe { &*(*stack_ptr).get() });
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
	include_str!("./isr64.S"),
	define_all_handlers!(),
	CS_OFFSET = const core::mem::offset_of!(StackFrame, cs),
	KERNEL_STACK_BASE_L4 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level4),
	KERNEL_STACK_BASE_L5 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level5),
}
