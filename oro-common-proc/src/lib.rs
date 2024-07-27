//! Common proc macros used by the `oro-common` crate.
//!
//! > **NOTE:** Do NOT use this crate directly. It is only intended
//! > to be used by the `oro-common` crate; anything meant to be used
//! > by other crates will be re-exported by `oro-common`.
#![deny(missing_docs, clippy::missing_docs_in_private_items)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]
// TODO(qix-): Remove this when <https://github.com/rust-lang/rust-clippy/issues/12425> is fixed
#![allow(clippy::tabs_in_doc_comments)]
#![feature(let_chains, proc_macro_span)]

mod enum_as;
mod enum_iterator;
mod gdb_autoload;
mod paste;

/// Derive macro for the `EnumIterator` trait.
///
/// This macro generates an implementation of the `EnumIterator` trait
/// which allows you to iterate over all unit variants of an enum via the
/// `iter_all()` method.
///
/// All variants in the enum MUST have no fields ("unit variants").
///
/// # Example
///
/// ```rust
/// use oro_common::proc::EnumIterator;
///
/// #[derive(EnumIterator, Debug)]
/// enum MyEnum {
/// 	Variant1,
/// 	Variant2,
/// 	Variant3,
/// }
///
/// for variant in MyEnum::iter_all() {
/// 	println!("{:?}", variant);
/// }
/// ```
#[proc_macro_derive(EnumIterator)]
pub fn derive_enum_iterator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::enum_iterator::derive_enum_iterator(input)
}

/// Proc macro that provides pasting tokens together into identifiers.
///
/// Usage:
/// ```rust
/// paste! {
///    // These both generate a function named `foobar`
///    // (whitespace is ignored).
///    fn foo%%bar() {}
///    fn foo %% bar() {}
/// }
/// ```
///
/// All tokens are concatenated together into a single identifier.
/// Concatenated tokens MUST be identifiers.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn paste(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::paste::paste(input)
}

/// Derive macro that allows unit enums with designators to be safely
/// converted to/from a `u64`.
#[proc_macro_derive(AsU64)]
pub fn enum_as_u64(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::enum_as::enum_as_u64(input)
}

/// Derive macro that allows unit enums with designators to be safely
/// converted to/from a `u32`.
#[proc_macro_derive(AsU32)]
pub fn enum_as_u32(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::enum_as::enum_as_u32(input)
}

/// Loads a python script from a file and embeds it into the binary
/// as an inline GDB autoload script.
#[proc_macro]
pub fn gdb_autoload_inline(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::gdb_autoload::gdb_autoload_inline(input)
}
