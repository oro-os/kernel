//! Macros (including proc macros) and supporting types for the Oro kernel.
//!
//! This crate also re-exports and provides the supplementary types
//! for the [`oro_macro_proc`] crate.
//!
//! It also houses the tests for the procedural macros.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

#[cfg(test)]
mod tests;

pub mod assert;
pub mod likely;
pub mod unsafe_macros;

pub use oro_macro_proc::*;

/// Converts a `#[naked]`-like assembly block into a byte buffer of assembly
/// instructions.
///
/// This macro uses the same [`core::arch::asm!`] syntax, but instead of embedding
/// the instructions inline into the binary, it generates a constant byte buffer
/// literal with the encoded instructions.
///
/// # Limitations
/// This macro only works with instructions that would otherwise work in a `#[naked]`
/// function. This means that the instructions must not reference any local variables
/// or function arguments.
///
/// The use of the bytes `0xDE`, `0xAD`, `0xBE`, and `0xEF` are allowed (in that order,
/// regardless of endianness) but the sequence cannot be repeated three times in a row,
/// else the macro will produce a short count.
#[macro_export]
macro_rules! asm_buffer {
	($($tt:tt)*) => {
		const {
			#[cfg(not(doc))]
			{
				$crate::asm_buffer_unchecked!($($tt)*)
			}
			#[cfg(doc)]
			{
				[]
			}
		}
	};
}

// We re-export this at the top level so that both
// the derive macro and trait get imported at once.
mod enum_iterator;
pub use enum_iterator::EnumIterator;
