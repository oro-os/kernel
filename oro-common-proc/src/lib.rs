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
mod vla;

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
#[allow(clippy::missing_panics_doc)]
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

/// Marks a structure as a VLA (Variable Length Array).
///
/// This structure expects the last structure field to be marked
/// with a `#[vla]` attribute in order to prevent accidental
/// usages.
///
/// The last attribute must have a "type" that is an array
/// of type `T` with a length of 0. It must be annotated with
/// `#[vla(count_field)]`, where `count_field` is the name of
/// a field in the structure that is a count of the array.
///
/// The struct field can be any unsigned numeric type as long
/// as it can be converted to a `usize` via `usize::from`.
///
/// An additional `impl` block is emitted with `vla_field()`
/// and `vla_field_mut()` methods (where the `vla_field`
/// is the name of the VLA field) that return a slice of
/// `count_field` elements of `T`.
///
/// The length field must be a **count** of the array,
/// **not the byte size**. It does not have to be `pub`.
///
/// The VLA field will be set to private, and the two
/// methods will be set to the original visibility of the
/// `#[vla(...)]` field.
///
/// # Example
/// ```no_run
/// use oro_common::proc::vla;
///
/// #[vla]
/// struct MyVla {
/// 	some_other_field: &'static str,
/// 	vla_count:        u32,
/// 	#[vla(vla_count)]
/// 	pub vla_items:    [SomeEntry; 0], // `pub` optional
/// }
/// ```
///
/// `#[vla]` will then emit the following structure and `impl`:
///
/// ```no_run
/// struct MyVla {
/// 	some_other_field: &'static str,
/// 	vla_count:        u32,
/// 	vla_items:        [SomeEntry; 0],
/// }
///
/// #[automatically_derived]
/// impl MyVla {
/// 	// `pub` because the original field was `pub`.
/// 	pub unsafe fn vla_items(&self) -> &[SomeEntry] {
/// 		// ...
/// 	}
///
/// 	pub unsafe fn vla_items_mut(&mut self) -> &mut [SomeEntry] {
/// 		// ...
/// 	}
/// }
/// ```
///
/// # Usage in Macros
/// Macro rules macros cannot use proc-macros in `$(#[$attr:meta])*` expansions.
/// Macros that expect a VLA structure can use `#[vla(allow_missing)]` on the struct
/// to allow the macro to compile without the VLA field.
#[proc_macro_attribute]
pub fn vla(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	match self::vla::vla(attr, item) {
		Ok(output) => output,
		Err(err) => err.to_compile_error().into(),
	}
}
