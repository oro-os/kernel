//! Constructor for the default IDT.
//!
//! Somewhat lengthy so it's put into its own module.

use core::arch::naked_asm;

use oro_arch_x86_64::idt::IdtEntry;
use oro_kernel_macro::paste;

/// Creates a new default IDT.
pub fn new_default() -> [IdtEntry; 256] {
	macro_rules! isr {
		(@@@ $vec_nr:tt $(, exc $exception:tt)? $(, push $push_err:literal)? $(,)?) => {{
			paste! {
				#[naked]
				#[unsafe(no_mangle)]
				extern "C" fn _oro_isr_handler_ %% $vec_nr () -> ! {
					// SAFETY: inherently unsafe.
					unsafe {
						naked_asm! {
							"cli", "cld",
							$($push_err,)?
							concat!("push ", stringify!($vec_nr)),
							concat!("jmp _oro_isr_common", $($exception,)?),
						}
					}
				}


				IdtEntry::new().with_kernel_cs().with_attributes(0x8E).with_isr(
					_oro_isr_handler_ %% $vec_nr
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

	new_idt
}
