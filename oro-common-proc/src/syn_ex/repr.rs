//! Implements high-level operations on [`syn`] types for the `#[repr(...)]` attribute.

use super::Attributes;

/// Provides high-level operations on [`syn`] types for the `#[repr(...)]` attribute.
pub trait ReprAttribute {
	/// Returns the [`ReprArgs`] of the type, if present.
	///
	/// Returns an error if the attribute is not present, or cannot be found.
	fn repr_args(&self) -> syn::Result<ReprArgs>;
}

/// Represents the arguments of a `#[repr(...)]` attribute.
pub struct ReprArgs {
	/// The span of the entire attribute.
	pub span:   proc_macro2::Span,
	/// The ABI, if specified.
	///
	/// ```
	/// #[repr(u8)]
	///        ^^
	/// ```
	pub abi:    Option<syn::Ident>,
	/// Whether the type is packed.
	///
	/// ```
	/// #[repr(packed)]
	///        ^^^^^^
	/// ```
	pub packed: bool,
}

impl syn::parse::Parse for ReprArgs {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let span = input.span();
		let mut abi = None;
		let mut packed = false;

		let mut first = true;

		while !input.is_empty() {
			if first {
				first = false;
			} else {
				input.parse::<syn::Token![,]>()?;
			}

			let ident: syn::Ident = input.parse()?;

			if ident == "packed" {
				if packed {
					return Err(syn::Error::new_spanned(ident, "duplicate packed specifier"));
				}

				packed = true;
			} else {
				if abi.is_some() {
					return Err(syn::Error::new_spanned(ident, "duplicate ABI specifier"));
				}

				abi = Some(ident);
			}
		}

		Ok(Self { span, abi, packed })
	}
}

impl<T> ReprAttribute for T
where
	T: Attributes + syn::spanned::Spanned,
{
	fn repr_args(&self) -> syn::Result<ReprArgs> {
		self.get_attribute("repr")
			.ok_or_else(|| syn::Error::new(self.span(), "no #[repr(...)] attribute found"))?
			.parse_args()
	}
}
