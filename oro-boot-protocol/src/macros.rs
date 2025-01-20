//! Macros that help define all of the types
//! of the requests/responses/tags/versions/etc
//! for the Oro kernel boot protocol.

/// Sealed trait for the request tags.
///
/// Prevents implementation of new request types
/// outside of this crate when using the types
/// provided by the `utils` feature.
pub(crate) trait Sealed {}

/// Main Oro boot protocol definition macro.
macro_rules! oro_boot_protocol {
	(
		$(
			$(#[$meta:meta])*
			 $TAG:literal  => $ReqName:ident {
				$(
					$(#[$revision_meta:meta])*
					$revision:literal => {
						$($tt:tt)*
					}
				)*
			}
		)*
	) => {
		::oro_macro::paste! {
			/// A request header. All requests start with this
			/// structure, guaranteed.
			///
			/// All endianness is architecture-endian.
			#[derive(Debug)]
			#[repr(C, align(16))]
			pub struct RequestHeader {
				/// The tag magic value.
				pub magic:    crate::Tag,
				/// The tag revision. Provided by the Kernel.
				pub revision: u64,
				/// Reserved for future use. Must be ignored by the bootloader.
				#[expect(clippy::pub_underscore_fields)]
				pub _reserved: [u8; 16],
			}

			const _: () = {
				::oro_macro::assert::size_of::<RequestHeader, 32>();
				::oro_macro::assert::align_of::<RequestHeader, 16>();
				::oro_macro::assert_offset_of!(RequestHeader, magic, 0);
				::oro_macro::assert_offset_of!(RequestHeader, revision, 8);
			};

			/// Holds the `TAG` constant for each request type.
			#[expect(private_bounds)]
			pub trait RequestTag: crate::macros::Sealed {
				/// The tag for the request.
				const TAG: crate::Tag;
			}

			/// Specifies which Request a given data structure
			/// is for.
			#[expect(private_bounds)]
			pub trait Data: crate::macros::Sealed {
				/// The request this data is for.
				type Request: RequestTag;
			}

			/// Specifies which revision of a Request the data
			/// structure is for.
			#[expect(private_bounds)]
			pub trait DataRevision: Data+ crate::macros::Sealed {
				/// The revision of the request.
				const REVISION: u64;
			}

			$(
				#[doc = concat!("The response data structures for the [`", stringify!($ReqName), "Request`], across all revisions.")]
				pub mod %<snake_case:$ReqName>% {
					#[expect(unused_imports)]
					use super::*;

					$(
						#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
						#[derive(Debug, Clone)]
						#[repr(C, align(16))]
						pub struct $ReqName %% DataV %% $revision {
							$($tt)*
						}

						impl crate::macros::Sealed for $ReqName %% DataV %% $revision {}

						impl super::DataRevision for $ReqName %% DataV %% $revision {
							const REVISION: u64 = $revision;
						}

						impl super::Data for $ReqName %% DataV %% $revision {
							type Request = super::$ReqName %% Request;
						}
					)*

					#[doc = concat!("The response data for the [`super::", stringify!($ReqName), "Request`].")]
					#[repr(C, align(16))]
					pub union $ReqName %% Data {
						$(
							#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
							pub v %% $revision: ::core::mem::ManuallyDrop<$ReqName %% DataV %% $revision>,
						)*
					}

					impl crate::macros::Sealed for $ReqName %% Data {}

					impl super::Data for $ReqName %% Data {
						type Request = super::$ReqName %% Request;
					}

					#[cfg(feature = "utils")]
					#[doc = concat!("A helper enum for the [`super::", stringify!($ReqName), "Request`] response data based on revision number. Holds a mutable reference to the data.")]
					#[non_exhaustive]
					pub enum $ReqName %% KindMut<'a> {
						$(
							#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
							V %% $revision (&'a mut ::core::mem::MaybeUninit<$ReqName %% DataV %% $revision>),
						)*
					}

					#[cfg(feature = "utils")]
					#[doc = concat!("A helper enum for the [`super::", stringify!($ReqName), "Request`] response data based on revision number. Holds an immutable reference to the data.")]
					#[non_exhaustive]
					pub enum $ReqName %% Kind<'a> {
						$(
							#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
							V %% $revision (&'a ::core::mem::MaybeUninit<$ReqName %% DataV %% $revision>),
						)*
					}

					#[cfg(feature = "utils")]
					impl<'a> From<$ReqName %% KindMut<'a>> for $ReqName %% Kind<'a> {
						fn from(kind: $ReqName %% KindMut<'a>) -> Self {
							match kind {
								$(
									$ReqName %% KindMut::V %% $revision(data) => {
										// SAFETY: We can safely cast the mutable reference to an immutable reference.
										$ReqName %% Kind::V %% $revision(data)
									},
								)*
							}
						}
					}
				}

				$(#[$meta])*
				#[repr(C, align(16))]
				pub struct $ReqName %% Request {
					/// The request header.
					pub header: RequestHeader,
					/// Whether or not the response was populated.
					/// 0x00 = not populated, 0xFF = populated.
					///
					/// MUST be set to 0x00 by the kernel, and set to 0xFF
					/// by the bootloader.
					pub populated: u8,
					/// Reserved for future use. Ignored by the kernel.
					#[expect(clippy::pub_underscore_fields)]
					pub _reserved:  [u8; 15],
					/// The response data. Filled in by the bootloader.
					///
					/// The union memory that is populated must match the revision
					/// of the request that was specified by the kernel.
					pub response: %<snake_case:$ReqName>%::$ReqName %% Data,
				}

				const _: () = {
					::oro_macro::assert_offset_of!($ReqName %% Request, header, 0);
					::oro_macro::assert::align_of::<$ReqName %% Request, 16>();
				};

				impl crate::macros::Sealed for $ReqName %% Request {}

				impl RequestTag for $ReqName %% Request {
					#[cfg(not(oro_build_protocol_header))]
					#[doc = concat!("The tag for the [`", stringify!($ReqName), "Request`]: equivalent to `", stringify!($TAG), "`")]
					const TAG: crate::Tag = {
						::oro_macro::assert::size_of1::<_, 8>($TAG);
						// SAFETY: The tag is a valid `u64` value.
						unsafe { ::core::mem::transmute_copy($TAG) }
					};
					#[cfg(oro_build_protocol_header)]
					const TAG: crate::Tag = $TAG;
				}

				#[cfg(feature = "utils")]
				impl crate::util::RequestData for $ReqName %% Request {
					unsafe fn response_data(&mut self) -> *mut u8 {
						::core::ptr::from_mut(&mut self.response).cast()
					}

					fn revision(&self) -> u64 {
						unsafe { core::ptr::read_volatile(&self.header.revision) }
					}

					fn mark_populated(&mut self) {
						unsafe { core::ptr::write_volatile(&mut self.populated, 0xFF); }
					}
				}

				impl $ReqName %% Request {
					/// Creates a new request with the given revision.
					#[must_use]
					pub const fn with_revision(revision: u64) -> Self {
						Self {
							header: RequestHeader {
								magic:    Self::TAG,
								revision,
								_reserved: [0; 16],
							},
							populated: 0,
							_reserved: [0; 15],
							// SAFETY(qix-): All of the members are `MaybeUninit`, so this is safe.
							response: unsafe { ::core::mem::zeroed() },
						}
					}

					/// Returns the response data for the request
					/// or `None` if the response was not populated
					/// or if the revision number is not recognized.
					#[must_use]
					#[cfg(feature = "utils")]
					#[expect(clippy::needless_lifetimes)]
					pub fn response<'a>(&'a self) -> Option<%<snake_case:$ReqName>%::$ReqName %% Kind<'a>> {
						if unsafe { core::ptr::read_volatile(&self.populated) } == 0 {
							return None;
						}

						match unsafe { core::ptr::read_volatile(&self.header.revision) } {
							$(
								$revision => {
									// SAFETY(qix-): We can safely cast a const pointer to a `MaybeUninit` reference.
									unsafe { Some(
										%<snake_case:$ReqName>% :: $ReqName %% Kind::V %% $revision(
											&*(::core::ptr::from_ref(&self.response).cast())
										)
									) }
								},
							)*
							_ => None,
						}
					}

					/// Returns a mutable reference to the response data for the request,
					/// based on the revision. Returns `None` if the revision number is not
					/// recognized. **Does not check if the response was populated.**
					///
					/// # Safety
					/// Accesses to the response data must ensure that the data has been
					/// properly initialized before read, and that the the proper revision
					/// is being accessed.
					#[cfg(feature = "utils")]
					#[must_use]
					#[expect(clippy::needless_lifetimes)]
					pub unsafe fn response_mut_unchecked<'a>(&'a mut self) -> Option<%<snake_case:$ReqName>%::$ReqName %% KindMut<'a>> {
						match unsafe { core::ptr::read_volatile(&self.header.revision) } {
							$(
								$revision => {
									// SAFETY: We can safely create the mutable reference as this is a `&mut self` method.
									unsafe { Some(
										%<snake_case:$ReqName>% :: $ReqName %% KindMut::V %% $revision(
											&mut *(::core::ptr::from_mut(&mut self.response).cast())
										)
									) }
								},
							)*
							_ => None,
						}
					}
				}

			)*

			/// An enum of all possible requests.
			#[cfg(feature = "utils")]
			#[non_exhaustive]
			pub enum Request<'a> {
				$(
					#[doc = concat!("The [`", stringify!($ReqName), "Request`].")]
					$ReqName(%<snake_case:$ReqName>% :: $ReqName %% Kind<'a>),
				)*
			}

			/// Attempts to look up a request by a pointer to a tag value.
			///
			/// Returns `None` if the tag is not recognized.
			///
			/// # Safety
			/// Must only be called to references into the kernel's requests
			/// segment.
			///
			/// The `response` field of the returned request header **must not**
			/// be used. It is only safe to use the `Request` element of the returned
			/// tuple.
			#[cfg(feature = "utils")]
			#[must_use]
			pub unsafe fn request_from_tag(tag: &mut crate::Tag) -> Option<(&mut RequestHeader, Request)> {
				if ::core::ptr::from_mut(tag).align_offset(::core::mem::align_of::<RequestHeader>()) != 0 {
					return None;
				}

				match *tag {
					$(
						$ReqName %% Request::TAG => {
							// SAFETY(qix-): We've already checked that it aligns properly.
							#[expect(clippy::cast_ptr_alignment)]
							let req = unsafe { &mut *::core::ptr::from_mut(tag).cast::<$ReqName %% Request>() };
							match req.header.revision {
								$(
									$revision => Some((
										&mut req.header,
										Request::$ReqName(
											%<snake_case:$ReqName>% :: $ReqName %% Kind::V %% $revision(
												unsafe { &mut *(::core::ptr::from_mut(&mut req.response).cast()) }
											)
										)
									)),
								)*
								_ => None,
							}
						},
					)*
					_ => None,
				}
			}
		}
	};
}

pub(crate) use oro_boot_protocol;
