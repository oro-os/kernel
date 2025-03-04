//! Implementation of, and supporting types for, the `asm_buffer!` macro.

use core::cell::LazyCell;

/// A lazy cell that can be used to store a constant byte buffer.
pub struct AsmBuffer(LazyCell<&'static [u8]>);

// SAFETY: This is a safe abstraction.
unsafe impl Sync for AsmBuffer {}

impl AsmBuffer {
	/// Creates a new `AsmBuffer` instance.
	///
	/// # Safety
	/// Do not use this function directly; instead, use the `asm_buffer!` macro.
	pub const unsafe fn from_fn(fun: fn() -> &'static [u8]) -> Self {
		Self(LazyCell::new(fun))
	}
}

impl ::core::ops::Deref for AsmBuffer {
	type Target = [u8];

	#[inline]
	fn deref(&self) -> &Self::Target {
		<LazyCell<&'static [u8]> as ::core::ops::Deref>::deref(&self.0)
	}
}

/// Converts a `#[naked]`-like assembly block into a byte buffer of assembly
/// instructions.
///
/// This macro uses similar syntax to the [`core::arch::asm!`] macro, and instead of embedding
/// the instructions inline into the binary, it generates a constant byte buffer
/// literal with the encoded instructions.
///
/// # Limitations
/// This macro only works with instructions that would otherwise work in a `#[naked]`
/// function. This means that the instructions must not reference any local variables
/// or function arguments.
///
/// # Usage
/// ```no_run
/// asm_buffer! {
///     static MY_ASM_BUFFER: AsmBuffer = {
//          // Code in its own section.
///         { "mov eax, 0x1234", "ret" },
///         // Optional section with inputs and options.
///         { const FOO = ..., options(noreturn), }
///     };
/// }
///
/// some_mut_slice.copy_from_slice(&MY_ASM_BUFFER);
/// assert_eq!(MY_ASM_BUFFER.len(), 42);
/// ```
#[macro_export]
macro_rules! asm_buffer {
	($(#[$meta:meta])* $vis:vis static $name:ident : AsmBuffer = { { $($code:literal),* $(,)? } $(, { $($tt:tt)* })? $(,)? };) => {
		::core::arch::global_asm! {
			".section .rodata",
			concat!(".global __asm_buffer_", line!()),
			concat!(".global __asm_buffer_", line!(), "_end"),
			concat!("__asm_buffer_", line!(), ":"),
			$($code),*,
			concat!("__asm_buffer_", line!(), "_end:"),
			$($($tt)*)?
		}

		$(#[$meta])*
		$vis static $name: $crate::asm_buffer::AsmBuffer = /* SAFETY: inherently unsafe */ unsafe {
			$crate::asm_buffer::AsmBuffer::from_fn(|| {
				// SAFETY: Inherently unsafe.
				unsafe extern "C" {
					#[link_name = concat!("__asm_buffer_", line!())]
					static __asm_buffer: u8;
					#[link_name = concat!("__asm_buffer_", line!(), "_end")]
					static __asm_buffer_end: u8;
				}

				// SAFETY: Inherently unsafe.
				unsafe {
					::core::slice::from_raw_parts(
						&raw const __asm_buffer,
						(&raw const __asm_buffer_end).addr() - (&raw const __asm_buffer).addr(),
					)
				}
			})
		};
	};
}
