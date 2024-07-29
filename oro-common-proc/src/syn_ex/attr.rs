//! Utilities for working with attributes on certain [`syn`] types.

/// Allows high-level querying and interacting with attributes on
/// certain [`syn`] types.
pub trait Attributes {
	/// Returns an attribute given its name. The name can be
	/// a path, such as `foo::bar`.
	///
	/// Returns `None` if the attribute is not present.
	fn get_attribute(&self, name: &str) -> Option<&syn::Attribute>;
}

impl Attributes for syn::DeriveInput {
	fn get_attribute(&self, name: &str) -> Option<&syn::Attribute> {
		let name_segments = name.split("::").collect::<Vec<_>>();
		self.attrs.iter().find(|attr| {
			let mut name_segments = name_segments.iter();
			let mut attr_segments = attr.path().segments.iter();

			loop {
				match (name_segments.next(), attr_segments.next()) {
					(Some(name), Some(attr)) => {
						if attr.ident != name {
							return false;
						}
					}
					(None, Some(_)) | (Some(_), None) => return false,
					(None, None) => return true,
				}
			}
		})
	}
}
