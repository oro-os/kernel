//! Provides the `bitstruct!{}` proc macro.
#![allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]

use std::{array, collections::HashSet, hash::Hash};

use proc_macro::Span;
use quote::quote;
use syn::{
	Attribute, Expr, ExprLit, Fields, Ident, Lit, LitInt, Result, Token, Type, Variant, Visibility,
	braced, bracketed, parenthesized,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	spanned::Spanned,
	token::{Brace, Bracket, Paren},
};

mod kw {
	syn::custom_keyword!(From);
}

#[derive(Debug)]
struct Bitstruct {
	attributes: Vec<Attribute>,
	vis: Visibility,
	_struct: Token![struct],
	name: Ident,
	_parens: Paren,
	primitive_type: Type,
	_braces: Brace,
	body: Vec<BitstructDef>,
}

impl Parse for Bitstruct {
	fn parse(input: ParseStream) -> Result<Self> {
		let mut content;
		Ok(Self {
			attributes: input.call(Attribute::parse_outer)?,
			vis: input.parse()?,
			_struct: input.parse()?,
			name: input.parse()?,
			_parens: parenthesized!(content in input),
			primitive_type: content.parse()?,
			_braces: braced!(content in input),
			body: content.call(parse_optionally_separated::<_, Token![,]>)?,
		})
	}
}

fn parse_optionally_separated<P: Parse, S: Parse>(input: ParseStream) -> Result<Vec<P>> {
	let mut items = Vec::new();
	while !input.is_empty() {
		while input.parse::<S>().is_ok() {}
		if input.is_empty() {
			break;
		}
		items.push(input.parse()?);
	}
	Ok(items)
}

#[derive(Debug)]
struct BitstructDef {
	attributes:   Vec<Attribute>,
	vis:          Visibility,
	field_name:   FieldName,
	_brackets:    Bracket,
	bit_range:    BitRange,
	_thick_arrow: Token![=>],
	field_body:   FieldBody,
	_semi:        Option<Token![;]>,
}

impl Parse for BitstructDef {
	fn parse(input: ParseStream) -> Result<Self> {
		let bit_range;
		Ok(Self {
			attributes:   input.call(Attribute::parse_outer)?,
			vis:          input.parse()?,
			field_name:   input.parse()?,
			_brackets:    bracketed!(bit_range in input),
			bit_range:    bit_range.parse()?,
			_thick_arrow: input.parse()?,
			field_body:   input.parse()?,
			_semi:        input.parse()?,
		})
	}
}

#[derive(Debug)]
enum FieldBody {
	Const(LitInt),
	ExtType(ExtType),
	Enum(EnumField),
}

impl Parse for FieldBody {
	fn parse(input: ParseStream) -> Result<Self> {
		let lookahead = input.lookahead1();
		if lookahead.peek(LitInt) {
			Ok(Self::Const(input.parse()?))
		} else if lookahead.peek(Token![enum]) {
			Ok(Self::Enum(input.parse()?))
		} else if lookahead.peek(Token![as])
			|| lookahead.peek(kw::From)
			|| lookahead.peek(Token![const])
		{
			Ok(Self::ExtType(input.parse()?))
		} else {
			Err(lookahead.error())
		}
	}
}

#[derive(Debug)]
enum ExtType {
	As(AsConversion),
	From(FromConversion),
}

impl Parse for ExtType {
	fn parse(input: ParseStream) -> Result<Self> {
		let lookahead = input.lookahead1();
		if lookahead.peek(kw::From) || lookahead.peek(Token![const]) {
			Ok(Self::From(input.parse()?))
		} else if lookahead.peek(Token![as]) {
			Ok(Self::As(input.parse()?))
		} else {
			Err(lookahead.error())
		}
	}
}

#[derive(Debug)]
struct AsConversion {
	_as:     Token![as],
	_unsafe: Option<Token![unsafe]>,
	ty:      Type,
}

impl Parse for AsConversion {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Self {
			_as:     input.parse()?,
			_unsafe: input.parse()?,
			ty:      input.parse()?,
		})
	}
}

#[derive(Debug)]
struct FromConversion {
	_const:  Option<Token![const]>,
	_from:   kw::From,
	_langle: Token![<],
	ty:      Type,
	_rangle: Token![>],
}

impl Parse for FromConversion {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Self {
			_const:  input.parse()?,
			_from:   input.parse()?,
			_langle: input.parse()?,
			ty:      input.parse()?,
			_rangle: input.parse()?,
		})
	}
}

#[derive(Debug)]
struct EnumField {
	_enum:     Token![enum],
	name:      Ident,
	_parens:   Paren,
	repr_type: Type,
	_braces:   Brace,
	variants:  Punctuated<Variant, Token![,]>,
}

impl Parse for EnumField {
	fn parse(input: ParseStream) -> Result<Self> {
		let type_content;
		let variant_content;
		Ok(Self {
			_enum:     input.parse()?,
			name:      input.parse()?,
			_parens:   parenthesized!(type_content in input),
			repr_type: type_content.parse()?,
			_braces:   braced!(variant_content in input),
			variants:  variant_content.parse_terminated(Variant::parse, Token![,])?,
		})
	}
}

#[derive(Debug)]
enum FieldName {
	/// `_`
	Ignored(Span),
	/// A custom field name.
	Ident(Ident),
}

impl FieldName {
	fn span(&self) -> Span {
		match self {
			Self::Ignored(span) => *span,
			Self::Ident(ident) => ident.span().unwrap(),
		}
	}

	fn is_ignored(&self) -> bool {
		match self {
			Self::Ignored(_) => true,
			Self::Ident(i) => i.to_string().starts_with('_'),
		}
	}
}

impl std::fmt::Display for FieldName {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Self::Ignored(_) => "_".fmt(f),
			Self::Ident(ident) => ident.fmt(f),
		}
	}
}

impl Parse for FieldName {
	fn parse(input: ParseStream) -> Result<Self> {
		input
			.parse::<Token![_]>()
			.map(|t| Self::Ignored(t.span().unwrap()))
			.or_else(|_| input.parse().map(Self::Ident))
	}
}

#[derive(Debug)]
struct BitRange {
	high:   u8,
	_colon: Option<Token![:]>,
	low:    Option<u8>,
}

impl BitRange {
	fn mask(&self) -> u128 {
		let high = self.high;
		let low = self.low.unwrap_or(high);
		((1_u128 << (high + 1)) - 1) & !((1_u128 << low) - 1)
	}

	fn span(&self) -> Span {
		if let Some(low) = self.low {
			self.high.span().unwrap().join(low.span().unwrap()).unwrap()
		} else {
			self.high.span().unwrap()
		}
	}

	fn high(&self) -> u8 {
		self.high
	}

	fn low(&self) -> u8 {
		self.low.unwrap_or(self.high)
	}

	fn count(&self) -> u32 {
		u32::from(self.high() - self.low()) + 1
	}
}

impl Parse for BitRange {
	fn parse(input: ParseStream) -> Result<Self> {
		let high = input.parse::<LitInt>()?.base10_parse::<u8>()?;
		let colon = input.parse::<Option<Token![:]>>()?;
		let low = if colon.is_some() {
			Some(input.parse::<LitInt>()?.base10_parse::<u8>()?)
		} else {
			None
		};
		Ok(Self {
			high,
			_colon: colon,
			low,
		})
	}
}

trait TypeEx {
	fn get_unsigned_primitive(&self) -> Option<String>;
	fn is_unsigned_primitive(&self) -> bool {
		self.get_unsigned_primitive().is_some()
	}

	fn get_unsigned_bit_width(&self) -> Option<u8> {
		match self.get_unsigned_primitive()?.as_str() {
			"u8" => Some(8),
			"u16" => Some(16),
			"u32" => Some(32),
			"u64" => Some(64),
			"u128" => Some(128),
			_ => unreachable!(),
		}
	}

	fn is_bool(&self) -> bool;
}

impl TypeEx for Type {
	fn get_unsigned_primitive(&self) -> Option<String> {
		if let Type::Path(p) = self {
			if let Some(segment) = p.path.segments.last() {
				if segment.ident == "u8"
					|| segment.ident == "u16"
					|| segment.ident == "u32"
					|| segment.ident == "u64"
					|| segment.ident == "u128"
				{
					return Some(segment.ident.to_string());
				}
			}
		}

		None
	}

	fn is_bool(&self) -> bool {
		if let Type::Path(p) = self {
			if let Some(segment) = p.path.segments.last() {
				return segment.ident == "bool";
			}
		}

		false
	}
}

/// Defines a register (or register-like) wrapper type around a primitive integer type,
///
/// See [`crate::register`] for more information.
pub fn bitstruct(input: proc_macro::TokenStream) -> Result<impl quote::ToTokens> {
	let Bitstruct {
		attributes,
		vis,
		name,
		primitive_type,
		body,
		..
	} = syn::parse(input)?;

	if !primitive_type.is_unsigned_primitive() {
		return Err(syn::Error::new_spanned(
			&primitive_type,
			"bistruct repr type must be an unsigned integer",
		));
	}

	let mut hit_bits: u128 = 0;
	let mut bit_fields: [Option<(String, Span)>; 128] = array::from_fn(|_| None);

	let mut const_bits_mask: u128 = 0;
	let mut const_bits: u128 = 0;

	let mut try_from_checks = vec![];
	let mut members = vec![];

	let mut enum_defs = vec![];

	for def in body {
		let vis = &def.vis;

		if def.bit_range.high() < def.bit_range.low() {
			def.bit_range
				.span()
				.error(format!(
					"field '{}' high bit cannot be lower than low bit",
					def.field_name
				))
				.emit();
			continue;
		}

		if def.bit_range.high() >= primitive_type.get_unsigned_bit_width().unwrap() {
			def.bit_range
				.span()
				.error(format!(
					"field '{}' high bit cannot be greater than or equal to the repr type bit \
					 width ({})",
					def.field_name,
					primitive_type.get_unsigned_bit_width().unwrap()
				))
				.emit();
			continue;
		}

		let field_mask = def.bit_range.mask();
		let conflicting_bits = hit_bits & field_mask;

		let mut mask_bits = field_mask;
		let mut conflicts = HashSet::new();

		while mask_bits != 0 {
			let bit = mask_bits.trailing_zeros();
			mask_bits &= !(1 << bit);

			if (conflicting_bits & (1 << bit)) != 0 {
				let (name, span) = bit_fields[bit as usize].as_ref().unwrap();
				let line_span = LineCmpSpan(*span);
				conflicts.insert((name.clone(), line_span));
			}

			bit_fields[bit as usize] = Some((def.field_name.to_string(), def.bit_range.span()));
		}

		if !conflicts.is_empty() {
			let mut diag = def.bit_range.high.span().unwrap().warning(format!(
				"bit field '{}' overlaps with existing fields",
				def.field_name
			));
			for (name, line_span) in conflicts {
				diag = diag.span_note(
					line_span.0,
					format!("overlaps with field `{name}` defined here"),
				);
			}
			diag.emit();
		}

		hit_bits |= field_mask;

		let attrs = &def.attributes;

		let get_name = Ident::new(&format!("{}", def.field_name), def.field_name.span().into());
		let set_name = Ident::new(
			&format!("set_{}", def.field_name),
			def.field_name.span().into(),
		);
		let with_name = Ident::new(
			&format!("with_{}", def.field_name),
			def.field_name.span().into(),
		);

		let low = def.bit_range.low();
		let low_mask = (1u128 << def.bit_range.count()) - 1;

		match &def.field_body {
			FieldBody::Const(lit) => {
				let Ok(lit_val) = lit.base10_parse::<u128>() else {
					lit.span()
						.unwrap()
						.error(
							"bit field constant must be an integer that fits within 128 bits in \
							 order to parse",
						)
						.emit();
					continue;
				};

				let max_bits = def.bit_range.count();
				let bit_count = 128 - lit_val.leading_zeros();

				if bit_count > max_bits {
					lit.span()
						.unwrap()
						.error(format!(
							"bit field '{}' constant value {:b} is too large for {} bits",
							def.field_name, lit_val, max_bits
						))
						.help(format!(
							"bit ranges are inclusive; high bit {} - low bit {} + 1 = {max_bits} \
							 bits; given constant is {} bits",
							def.bit_range.high(),
							def.bit_range.low(),
							bit_count
						))
						.emit();

					continue;
				}

				if !def.field_name.is_ignored() {
					name.span()
						.unwrap()
						.warning(format!(
							"bit field '{name}' is given a name, but is a constant value; the \
							 name is not used",
						))
						.help("prefix the field with a '_'")
						.emit();
				}

				if !def.attributes.is_empty() {
					let mut diag = def
						.field_name
						.span()
						.warning("bit field has unused attributes");

					let mut has_doc = false;
					let mut unused_count = 0;

					for attr in def.attributes {
						let is_doc = attr.path().is_ident("doc");

						if is_doc {
							has_doc = true;
						} else {
							diag = diag.span_note(attr.span().unwrap(), "unused attribute");
							unused_count += 1;
						}
					}

					if has_doc {
						diag = diag.note(
							"#[doc] attributes are not considered unused and did not trigger this \
							 warning",
						);
					}

					if unused_count > 0 {
						diag.help(
							"constant bit fields generate no accessor methods whereby the \
							 attributes could be attached",
						)
						.emit();
					}
				}

				const_bits |= lit_val << def.bit_range.low();
				const_bits_mask |= field_mask;
			}
			FieldBody::ExtType(ext_ty) => {
				if def.field_name.is_ignored() {
					def.field_name
						.span()
						.warning("bit field is ignored; type is not used")
						.help("remove the '_' prefix, or remove the field entirely")
						.emit();
				}

				let see_message =
					format!("See [`Self::{get_name}()`] for more information about this field.",);

				match &ext_ty {
					ExtType::As(as_conv) if as_conv.ty.is_bool() => {
						if def.bit_range.count() != 1 {
							as_conv
								.ty
								.span()
								.unwrap()
								.error(format!(
									"boolean bit field must be exactly 1 bit wide (field is {} \
									 bits wide)",
									def.bit_range.count()
								))
								.emit();
							continue;
						}

						members.push(quote! {
							#(#attrs)*
							#vis const fn #get_name(self) -> bool {
								(self.0 & (1 << #low)) != 0
							}

							#[doc = "Sets the bit field, returning the new value as a copy."]
							#[doc = #see_message]
							#vis const fn #with_name(self, val: bool) -> Self {
								if val {
									Self(self.0 | (1 << #low))
								} else {
									Self(self.0 & !(1 << #low))
								}
							}

							#[doc = "Sets the bit field in place. Returns `self`."]
							#[doc = #see_message]
							#vis fn #set_name(&mut self, val: bool) -> &mut Self {
								if val {
									self.0 |= 1 << #low;
								} else {
									self.0 &= !(1 << #low);
								}
								self
							}
						});
					}
					ExtType::As(as_conv) if as_conv._unsafe.is_some() => {
						let ty = &as_conv.ty;

						members.push(quote! {
							#(#attrs)*
							#vis const fn #get_name(self) -> #ty {
								// SAFETY: Consumer of this proc-macro has opted into the unsafe conversion via
								// SAFETY: the `as unsafe` syntax.
								unsafe { ::core::mem::transmute(((self.0 >> #low) & (#low_mask as #primitive_type)) as #primitive_type) }
							}

							#[doc = "Sets the bit field, returning the new value as a copy."]
							#[doc = #see_message]
							#vis const fn #with_name(self, val: #ty) -> Self {
								Self((self.0 & !((#low_mask as #primitive_type) << #low)) | (((val as #primitive_type) & (#low_mask as #primitive_type)) << #low))
							}

							#[doc = "Sets the bit field in place. Returns `self`."]
							#[doc = #see_message]
							#vis fn #set_name(&mut self, val: #ty) -> &mut Self {
								self.0 = (self.0 & !((#low_mask as #primitive_type) << #low)) | (((val as #primitive_type) & (#low_mask as #primitive_type)) << #low);
								self
							}
						});
					}
					ExtType::As(as_conv) => {
						let ty = &as_conv.ty;

						members.push(quote! {
							#(#attrs)*
							#vis const fn #get_name(self) -> #ty {
								((self.0 >> #low) & (#low_mask as #primitive_type)) as #primitive_type as #ty
							}

							#[doc = "Sets the bit field, returning the new value as a copy."]
							#[doc = #see_message]
							#vis const fn #with_name(self, val: #ty) -> Self {
								Self((self.0 & !((#low_mask as #primitive_type) << #low)) | (((val as #primitive_type) & (#low_mask as #primitive_type)) << #low))
							}

							#[doc = "Sets the bit field in place. Returns `self`."]
							#[doc = #see_message]
							#vis fn #set_name(&mut self, val: #ty) -> &mut Self {
								self.0 = (self.0 & !((#low_mask as #primitive_type) << #low)) | (((val as #primitive_type) & (#low_mask as #primitive_type)) << #low);
								self
							}
						});
					}
					ExtType::From(from_conv) if from_conv._const.is_some() => {
						let ty = &from_conv.ty;

						let as_conv_name = Ident::new(
							&format!("as_{}", primitive_type.get_unsigned_primitive().unwrap()),
							ty.span(),
						);

						members.push(quote! {
							#(#attrs)*
							#vis const fn #get_name(self) -> #ty {
								#ty::from(((self.0 >> #low) & (#low_mask as #primitive_type)) as #primitive_type)
							}

							#[doc = "Sets the bit field, returning the new value as a copy."]
							#[doc = #see_message]
							#vis const fn #with_name(self, val: #ty) -> Self {
								let val: #primitive_type = val.#as_conv_name();
								Self((self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low))
							}

							#[doc = "Sets the bit field in place. Returns `self`."]
							#[doc = #see_message]
							#vis fn #set_name(&mut self, val: #ty) -> &mut Self {
								let val: #primitive_type = val.#as_conv_name();
								self.0 = (self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low);
								self
							}
						});
					}
					ExtType::From(from_conv) => {
						let ty = &from_conv.ty;
						members.push(quote! {
							#(#attrs)*
							#vis fn #get_name(self) -> #ty {
								<#ty as ::core::convert::From<#primitive_type>>::from(((self.0 >> #low) & (#low_mask as #primitive_type)) as #primitive_type)
							}

							#[doc = "Sets the bit field, returning the new value as a copy."]
							#[doc = #see_message]
							#vis fn #with_name(self, val: #ty) -> Self {
								let val: #primitive_type = <#primitive_type as ::core::convert::From<#ty>>::from(val);
								Self((self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low))
							}

							#[doc = "Sets the bit field in place. Returns `self`."]
							#[doc = #see_message]
							#vis fn #set_name(&mut self, val: #ty) -> &mut Self {
								let val: #primitive_type = <#primitive_type as ::core::convert::From<#ty>>::from(val);
								self.0 = (self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low);
								self
							}
						});
					}
				}
			}
			FieldBody::Enum(enum_field) => {
				match enum_field.repr_type.get_unsigned_bit_width() {
					None => {
						enum_field
							.repr_type
							.span()
							.unwrap()
							.error("enum repr type must be an unsigned integer")
							.emit();
						continue;
					}
					Some(width) => {
						if u32::from(width) < def.bit_range.count() {
							enum_field
								.repr_type
								.span()
								.unwrap()
								.error(format!(
									"enum repr type is too small for bit field '{}'",
									def.field_name
								))
								.emit();
							continue;
						}
					}
				}

				#[expect(clippy::needless_continue)]
				for variant in &enum_field.variants {
					let Some(ref discrim_val) = &variant.discriminant else {
						variant
							.ident
							.span()
							.unwrap()
							.error("bitstruct enum variant must have a discriminant")
							.emit();
						continue;
					};

					let Expr::Lit(ExprLit {
						lit: Lit::Int(ref discrim_val),
						..
					}) = &discrim_val.1
					else {
						variant
							.ident
							.span()
							.unwrap()
							.error("bitstruct enum variant discriminant must be a literal integer")
							.emit();
						continue;
					};

					let Ok(discrim_val) = discrim_val.base10_parse::<u128>() else {
						variant
							.ident
							.span()
							.unwrap()
							.error("bitstruct enum variant discriminant doesn't fit into 128 bits")
							.emit();
						continue;
					};

					if discrim_val >= (1 << def.bit_range.count()) {
						variant
							.ident
							.span()
							.unwrap()
							.error(format!(
								"bitstruct enum variant discriminant is too large for bit field \
								 '{}'",
								def.field_name
							))
							.note(format!(
								"bit field is {} bits wide; given discriminant is {} bits wide",
								def.bit_range.count(),
								128 - discrim_val.leading_zeros()
							))
							.emit();
						continue;
					}

					if variant.fields != Fields::Unit {
						variant
							.ident
							.span()
							.unwrap()
							.error("bitstruct enum variant must be a unit variant")
							.emit();

						continue;
					}
				}

				let enum_name = &enum_field.name;
				let variants = &enum_field.variants;
				let repr_type = &enum_field.repr_type;

				let get_message = format!("Returns this field's [`{enum_name}`] value.");
				let set_message = format!("Sets this field's [`{enum_name}`] value.");
				let with_message =
					format!("Returns a copy of this field with the [`{enum_name}`] value set.");

				// We can assume this to be a strong enough check because the compiler won't allow
				// duplicate discriminants in an enum.
				let is_exhaustive = enum_field.variants.len() == (1 << def.bit_range.count());

				let non_exhaustive_attr = if is_exhaustive {
					None
				} else {
					Some(quote!(#[non_exhaustive]))
				};

				enum_defs.push(quote! {
					#(#attrs)*
					#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
					#[repr(#repr_type)]
					#non_exhaustive_attr
					#vis enum #enum_name {
						#variants
					}
				});

				members.push(quote! {
					#[doc = #get_message]
					#vis const fn #get_name(self) -> #enum_name {
						// SAFETY: The discriminant is guaranteed to be within the enum's range (we generated it).
						unsafe { ::core::mem::transmute(((self.0 >> #low) & (#low_mask as #primitive_type)) as #repr_type) }
					}

					#[doc = #set_message]
					#vis fn #set_name(&mut self, val: #enum_name) -> &mut Self {
						let val = val as #primitive_type;
						self.0 = (self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low);
						self
					}

					#[doc = #with_message]
					#vis const fn #with_name(self, val: #enum_name) -> Self {
						let val = val as #primitive_type;
						Self((self.0 & !((#low_mask as #primitive_type) << #low)) | ((val & (#low_mask as #primitive_type)) << #low))
					}
				});
			}
		}
	}

	if const_bits_mask != 0 {
		try_from_checks.insert(
			0,
			quote! {
				if (value & (#const_bits_mask as #primitive_type)) != (#const_bits as #primitive_type) {
					return Err(value);
				}
			},
		);
	}

	Ok(quote! {
		#(#attributes)*
		#[repr(transparent)]
		#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
		#vis struct #name(#primitive_type);

		#[automatically_derived]
		impl #name {
			/// Returns the default value.
			#vis const fn new() -> Self {
				Self(#const_bits as #primitive_type)
			}

			#(#members)*
		}

		#[automatically_derived]
		impl ::core::default::Default for #name {
			fn default() -> Self {
				Self::new()
			}
		}

		#[automatically_derived]
		impl ::core::convert::TryFrom<#primitive_type> for #name {
			type Error = #primitive_type;

			fn try_from(value: #primitive_type) -> Result<Self, Self::Error> {
				#(#try_from_checks)*
				Ok(Self(value))
			}
		}

		#(#enum_defs)*
	})
}

#[repr(transparent)]
struct LineCmpSpan(Span);

impl PartialEq for LineCmpSpan {
	fn eq(&self, other: &Self) -> bool {
		self.0.start().line() == other.0.start().line()
	}
}

impl Eq for LineCmpSpan {}

impl Hash for LineCmpSpan {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.start().line().hash(state);
	}
}
