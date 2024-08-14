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
#[allow(clippy::too_many_lines)]
macro_rules! oro_boot_protocol {
	(
		$(
			$(#[$meta:meta])*
			$ReqName:ident [ $TAG:literal ] {
				$(
					$revision:tt => {
						$($tt:tt)*
					}
				)*
			}
		)*
	) => {
		::oro_common_proc::paste! {
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
				pub reserved: [u8; 16],
			}

			const _: () = {
				::oro_common_assertions::size_of::<RequestHeader, 32>();
				::oro_common_assertions::align_of::<RequestHeader, 16>();
				::oro_common_assertions::offset_of!(RequestHeader, magic, 0);
				::oro_common_assertions::offset_of!(RequestHeader, revision, 8);
			};

			/// Holds the `TAG` constant for each request type.
			#[allow(private_bounds)]
			pub trait RequestTag: crate::macros::Sealed {
				/// The tag for the request.
				const TAG: crate::Tag;
			}

			$(
				#[doc = concat!("The response data structures for the [`", stringify!($ReqName), "Request`], across all revisions.")]
				pub mod %<snake_case:$ReqName>% {
					$(
						#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
						#[derive(Debug, Clone, Copy)]
						#[repr(C, align(16))]
						pub struct $ReqName %% DataV %% $revision {
							$($tt)*
						}
					)*

					#[doc = concat!("The response data for the [`super::", stringify!($ReqName), "Request`].")]
					#[repr(C, align(16))]
					pub union $ReqName %% Data {
						$(
							#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
							pub v %% $revision: $ReqName %% DataV %% $revision,
						)*
					}

					#[cfg(feature = "utils")]
					#[doc = concat!("A helper enum for the [`super::", stringify!($ReqName), "Request`] response data based on revision number.")]
					pub enum $ReqName %% Kind<'a> {
						$(
							#[doc = concat!("The response data for version ", stringify!($revision), " of the [`super::", stringify!($ReqName), "Request`].")]
							V %% $revision (&'a mut ::core::mem::MaybeUninit<$ReqName %% DataV %% $revision>),
						)*
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
					pub reserved:  [u8; 15],
					/// The response data. Filled in by the bootloader.
					///
					/// The union memory that is populated must match the revision
					/// of the request that was specified by the kernel.
					pub response: ::core::mem::MaybeUninit<%<snake_case:$ReqName>%::$ReqName %% Data>,
				}

				const _: () = {
					::oro_common_assertions::offset_of!($ReqName %% Request, header, 0);
					::oro_common_assertions::align_of::<$ReqName %% Request, 16>();
				};

				impl crate::macros::Sealed for $ReqName %% Request {}

				impl RequestTag for $ReqName %% Request {
					#[doc = concat!("The tag for the [`", stringify!($ReqName), "Request`].")]
					const TAG: crate::Tag = {
						::oro_common_assertions::size_of1::<_, 8>($TAG);
						unsafe { ::core::mem::transmute_copy($TAG) }
					};
				}

				impl $ReqName %% Request {
					/// Returns the response data for the request,
					/// or `None` if the response was not populated.
					#[must_use]
					pub const fn response(&self) -> Option<&%<snake_case:$ReqName>%::$ReqName %% Data> {
						if self.populated == 0xFF {
							unsafe { Some(&self.response.assume_init_ref()) }
						} else {
							None
						}
					}

					/// Creates a new request with the given revision.
					#[must_use]
					pub const fn with_revision(revision: u64) -> Self {
						Self {
							header: RequestHeader {
								magic:    Self::TAG,
								revision,
								reserved: [0; 16],
							},
							populated: 0,
							reserved:  [0; 15],
							response:  ::core::mem::MaybeUninit::uninit(),
						}
					}
				}

			)*

			/// An enum of all possible requests.
			#[cfg(feature = "utils")]
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
							#[allow(clippy::cast_ptr_alignment)]
							let req = unsafe { &mut *::core::ptr::from_mut(tag).cast::<$ReqName %% Request>() };
							match req.header.revision {
								$(
									$revision => Some((
										&mut req.header,
										Request::$ReqName(
											%<snake_case:$ReqName>% :: $ReqName %% Kind::V %% $revision(
												unsafe { &mut *(req.response).as_mut_ptr().cast() }
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
