//! Architecture selection stub.
//!
//! This crate does nothing more than
//! expose an architecture implementation
//! for the kernel and bootloaders to use,
//! bound tightly to the [`oro_common::arch::Arch`]
//! trait.
//!
//! To use, import like so:
//!
//! ```rust
//! use oro_arch::Arch;
//! use oro_common::arch::Arch as _;
//! ```
//!
//! [`oro_common::arch::Arch`] does not provide any
//! access to the underlying architecture other
//! than what is explicitly specified by the
//! [`oro_common::arch::Arch`] trait.
#![cfg_attr(not(test), no_std)]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![feature(type_alias_impl_trait)]

/// The current architecture implementation.
///
/// See [`oro_common::arch::Arch`] for more information.
pub type Target = impl oro_common::arch::Arch;

// Necessary for TAIT to pick up the trait.
#[allow(dead_code, clippy::missing_docs_in_private_items)]
#[doc(hidden)]
fn _current_arch() -> Target {
	macro_rules! archs {
		($($arch:literal => $imp:path),* $(,)?) => {
			$(
				#[cfg(target_arch = $arch)]
				{
					$imp
				}
			)*

			#[cfg(not(any($(target_arch = $arch),*)))]
			{
				compile_error!("architecture not supported");
			}
		};
	}

	archs! {
		"x86_64" => oro_arch_x86_64::X86_64,
		"aarch64" => oro_arch_aarch64::Aarch64,
	}
}
