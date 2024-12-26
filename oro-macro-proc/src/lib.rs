//! Common proc macros used by the Oro kernel.
//!
//! > **NOTE:** Don't use this crate directly; instead, use the `oro-macro` crate.
#![deny(missing_docs, clippy::missing_docs_in_private_items)]
// TODO(qix-): Remove this when <https://github.com/rust-lang/rust-clippy/issues/12425> is fixed
#![expect(clippy::tabs_in_doc_comments)]
#![feature(let_chains, proc_macro_span)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

mod asm_buffer;
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
/// use oro_proc::EnumIterator;
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
///
/// # Manipulation
/// Identifiers (or identifier-like) tokens can be manipulated
/// to generate new identifiers that wouldn't otherwise be possible
/// with regular Rust syntax.
///
/// The syntax is `#operation:token`, where `operation` is one of
/// the manipulation operations, and `token` is the identifier to
/// manipulate.
///
/// Note that nothing can immediately precede the `#` symbol.
///
/// ## `title_case`
/// Converts the token to TitleCase.
///
/// Numbers and symbols are treated as word boundaries.
///
/// ```no_run
/// paste! {
///    const #title_case:some_value = 42;
///    assert_eq!(SomeValue, 42);
/// }
/// ```
///
/// ## `snake_case`
/// Converts the token to snake_case.
///
/// Numbers and symbols are treated as word boundaries.
///
/// ```no_run
/// paste! {
///    const #snake_case:SomeValue = 42;
///    assert_eq!(some_value, 42);
/// }
/// ```
///
/// ## `camel_case`
/// Converts the token to camelCase.
///
/// Numbers and symbols are treated as word boundaries.
///
/// ```no_run
/// paste! {
///    const #camel_case:SomeValue = 42;
///    assert_eq!(someValue, 42);
/// }
/// ```
///
/// ## `const_case`
/// Converts the token to CONST_CASE.
///
/// Numbers and symbols are treated as word boundaries.
///
/// ```no_run
/// paste! {
///    const #const_case:SomeValue = 42;
///    assert_eq!(SOME_VALUE, 42);
/// }
/// ```
#[proc_macro]
pub fn paste(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match self::paste::paste(input) {
		Ok(output) => output,
		Err(err) => err.to_compile_error().into(),
	}
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

/// Unchecked version of the `asm_buffer!` macro from the `oro-macro` crate.
///
/// You should use that one instead. Do not use this macro directly, as it
/// fails when generating documentation and provides no additional control or
/// benefit over `asm_buffer!`.
#[proc_macro]
pub fn asm_buffer_unchecked(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	self::asm_buffer::asm_buffer(input)
}
