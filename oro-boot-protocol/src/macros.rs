//! Macros that help define all of the types
//! of the requests/responses/tags/versions/etc
//! for the Oro kernel boot protocol.

/// Main Oro boot protocol definition macro.
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
				pub magic:    u64,
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

				impl $ReqName %% Request {
					#[doc = concat!("The tag for the [`", stringify!($ReqName), "Request`].")]
					pub const TAG: u64 = {
						::oro_common_assertions::size_of1::<_, 8>($TAG);
						unsafe { ::core::mem::transmute_copy($TAG) }
					};

					/// Returns the response data for the request,
					/// or `None` if the response was not populated.
					pub const fn response(&self) -> Option<&%<snake_case:$ReqName>%::$ReqName %% Data> {
						if self.populated == 0xFF {
							unsafe { Some(&self.response.assume_init_ref()) }
						} else {
							None
						}
					}

					/// Creates a new request with the given revision.
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
		}
	};
}

pub(crate) use oro_boot_protocol;
