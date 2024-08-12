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
//!
//! # Configuration
//! This crate also exports the architecture-specific
//! configuration type as `Config`. It is required
//! To pass this type to the kernel initialization
//! functions with relevant information pertaining
//! to e.g. ACPI, device tree information, etc.,
//! depending on the architecture.
#![cfg_attr(not(test), no_std)]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![feature(type_alias_impl_trait)]

// A little anecdote about the `Config` type being exported here.
// It's not really what I wanted - I would have preferred it being
// exposed via the `Arch` trait. I spent a good 24 hours trying to
// make it work - going so far as to allow the preboot environment
// to pass in the concrete type, serializing it with ser2mem,
// and then having modified ser2mem to allow a `#[ser2mem(ptr)]`
// attribute on struct fields, pass the pointer from the arch
// config serialization to it, and write it as a static ref as-is.
// However the lack of a proper concrete type coming from the TAIT
// architecture trait boiled all the way up to the preboot crate,
// no matter how much I wrestled with it.
//
// So, here's where it landed. It's not ideal, but it works. I'm not
// sure there's any other feasible way to do it. It's at least
// type-checked by the boot sequence, but still not specified
// nicely in the `Arch` trait like I would have liked, which is
// fine. It's not a big deal.
//
// Qix-

/// The current architecture implementation.
///
/// See [`oro_common::arch::Arch`] for more information.
pub type Target = impl oro_common::arch::Arch;

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! archs {
	($($arch:literal => $pkg:ident :: $archname:ident),* $(,)?) => {
		$(
			// Necessary for TAIT to pick up the trait.
			#[cfg(target_arch = $arch)]
			#[allow(dead_code, clippy::missing_docs_in_private_items)]
			#[doc(hidden)]
			fn _current_arch() -> Target {
				$pkg::$archname
			}

			/// The architecture-specific configuration used by the
			/// current architecture implementation.
			#[cfg(target_arch = $arch)]
			pub type Config = $pkg::Config;
		)*

		#[cfg(not(any($(target_arch = $arch),*)))]
		const _: () = {
			compile_error!("architecture not supported");
		};
	};
}

archs! {
	"x86_64" => oro_arch_x86_64::X86_64,
	"aarch64" => oro_arch_aarch64::Aarch64,
}
