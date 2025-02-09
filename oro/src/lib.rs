//! High-level runtime support for Oro modules.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
// SAFETY(qix-): This is for the `runtime::Key` debug helper, and purely for
// SAFETY(qix-): ergonomics and strict adherence to the intention of the
// SAFETY(qix-): `Key` type. It is not used in any unsafe context.
#![cfg_attr(feature = "nightly", feature(negative_impls))]
// SAFETY(qix-): Seems to be reasonably accepted. It also allows `Key` to
// SAFETY(qix-): infallibly write a format string in a single call.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/110998
#![cfg_attr(feature = "nightly", feature(ascii_char))]

pub(crate) mod arch;
pub(crate) mod buddy_system;
pub(crate) mod lock;

pub mod alloc;
pub mod id;
pub mod macros;
pub mod syscall;
pub mod tls;

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Lazily fetches an interface's ID on first use.
pub struct LazyIfaceId<const TYPE_ID: u64>(AtomicU64);

impl<const TYPE_ID: u64> LazyIfaceId<TYPE_ID> {
	/// Creates a new `LazyIfaceId` instance.
	#[must_use]
	pub const fn new() -> Self {
		Self(AtomicU64::new(
			if (TYPE_ID & id::mask::KERNEL_ID) == 0 {
				// It's a kernel ID, which is always resolved.
				TYPE_ID
			} else {
				0
			},
		))
	}

	/// Returns the interface ID, resolving it if necessary.
	///
	/// Returns `None` if the interface could not be resolved.
	pub fn get(&self) -> Option<u64> {
		let id = self.0.load(Relaxed);
		if id == 0 {
			// SAFETY: Getting the type ID is safe.
			let iface = unsafe {
				crate::syscall_get!(
					crate::id::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
					crate::id::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
					TYPE_ID,
					0
				)
				.ok()?
			};

			if let Err(other_id) = self.0.compare_exchange(0, iface, Relaxed, Relaxed) {
				// Another thread resolved the ID first; use that.
				Some(other_id)
			} else {
				Some(iface)
			}
		} else {
			Some(id)
		}
	}
}

/// Common root ring interfaces.
pub mod root_ring {
	/// Debug output (version 0) interface abstraction.
	pub mod debug_out_v0 {
		use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

		/// The `KERNEL_DEBUG_OUT_V0` interface ID, or `0` if it's not
		/// been resolved.
		static DEBUG_OUT_V0_ID: AtomicU64 = AtomicU64::new(0);

		/// Returns the `KERNEL_DEBUG_OUT_V0` interface ID, resolving
		/// it if necessary.
		///
		/// Returns `None` if the interface could not be resolved.
		pub fn id() -> Option<u64> {
			let id = DEBUG_OUT_V0_ID.load(Relaxed);
			if id == 0 {
				// SAFETY: Getting the interface instance is safe.
				let Ok(iface) = (unsafe {
					crate::syscall_get!(
						crate::id::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
						crate::id::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
						crate::id::iface::ROOT_DEBUG_OUT_V0,
						0
					)
				}) else {
					DEBUG_OUT_V0_ID.store(!0, Relaxed);
					return None;
				};

				DEBUG_OUT_V0_ID.store(iface, Relaxed);
				Some(iface)
			} else if id == !0 {
				// Positive failure cache; if there's no root ring
				// debug interface, there won't be one in the future
				// (at least, we assume).
				None
			} else {
				Some(id)
			}
		}

		/// Writes a byte slice to the debug output.
		///
		/// Note that this function behaves in a somewhat unusual way:
		///
		/// - There is no implicit encoding, though UTF-8 should be used
		///   where possible.
		/// - Newlines (`\n`, `0x0A`) are treated as line breaks, and flush
		///   the buffer.
		/// - The buffer has a per-line minimum and maximum before a line
		///   is force-flushed.
		/// - There is no `flush` command; simply send a newline to flush.
		/// - Data is sent to the buffer in 8-byte increments over
		///   synchronous syscalls. **It is therefore not performant to use
		///   this interface for high volumes of data.**
		/// - This data is not visible to the user in any case, especially
		///   after boot; **do not use this for important messages.**
		pub fn write_bytes(bytes: &[u8]) {
			if bytes.is_empty() {
				return;
			}

			let Some(iface) = id() else {
				return;
			};

			for chunk in bytes.chunks(8) {
				let mut word = 0u64;
				for b in chunk {
					word = (word << 8) | u64::from(*b);
				}

				// SAFETY: This doesen't modify any application state, and is safe.
				unsafe {
					crate::syscall_set!(
						crate::id::iface::ROOT_DEBUG_OUT_V0,
						iface,
						0,
						crate::key!("write"),
						word
					)
					.unwrap();
				}
			}
		}

		/// A [`core::fmt::Write`] implementation for the debug output interface.
		///
		/// This is a unit struct; it requires no initialization.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		///
		/// See [`write_bytes`] for more information.
		pub struct DebugV0Write;

		impl ::core::fmt::Write for DebugV0Write {
			fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
				write_bytes(s.as_bytes());
				Ok(())
			}
		}

		/// `println!()` macro that prints to the root ring debug output (version 0)
		/// interface.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		///
		/// See [`write_bytes`] for more information.
		#[macro_export]
		macro_rules! debug_out_v0_println {
			($($arg:tt)*) => {
				{
					use $crate::root_ring::debug_out_v0::DebugV0Write;
					use ::core::fmt::Write;
					let _ = ::core::writeln!(&mut DebugV0Write, $($arg)*);
				}
			}
		}

		/// `print!()` macro that prints to the root ring debug output (version 0)
		/// interface.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		///
		/// See [`write_bytes`] for more information.
		#[macro_export]
		macro_rules! debug_out_v0_print {
			($($arg:tt)*) => {
				{
					use $crate::root_ring::debug_out_v0::DebugV0Write;
					use ::core::fmt::Write;
					let _ = ::core::write!(&mut DebugV0Write, $($arg)*);
				}
			}
		}
	}
}

#[doc(hidden)]
#[cfg(feature = "nightly")]
mod nightly {
	use core::{ascii::Char, fmt};

	/// Debug output wrapper for keys and errors that use a `key!("...")`-like
	/// encoding.
	///
	/// **Do not use this for anything other than debug logging.** This type is
	/// made intentionally restrictive to prevent misuse.
	///
	/// # Example
	/// ```
	/// use oro::{Key, syscall::key};
	///
	/// let k = key!("hello");
	/// println!("{:?}", Key(&k)); // hello
	/// println!("{:#?}", Key(&k)); // Key("hello\0\0\0\0")
	/// ```
	pub struct Key<'a>(pub &'a u64)
	where
		Self: 'a;

	impl !Send for Key<'_> {}
	impl !Sync for Key<'_> {}

	impl fmt::Debug for Key<'_> {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			#[doc(hidden)]
			macro_rules! c {
				($l:literal) => {
					// SAFETY: We only emit valid ASCII characters in this function.
					unsafe { Char::from_u8_unchecked($l) }
				};
			}

			let mut buf = [c!(b'\0'); (8 * b"\\xBB".len()) + "Key(\"\")".len()];
			let mut i = 0;
			let bytes = self.0.to_be_bytes();

			let limit = if f.alternate() {
				8
			} else {
				bytes.iter().position(|&b| b == 0).unwrap_or(8)
			};

			let quote = f.alternate()
				|| bytes
					.iter()
					.take(limit)
					.any(|&b| b == b' ' || !b.is_ascii_graphic());

			if f.alternate() {
				buf[i] = c!(b'K');
				buf[i + 1] = c!(b'e');
				buf[i + 2] = c!(b'y');
				buf[i + 3] = c!(b'(');
				i += 4;
			}

			if quote {
				buf[i] = c!(b'"');
				i += 1;
			}

			for b in bytes.iter().take(limit) {
				for b in core::ascii::escape_default(*b) {
					// SAFETY: We only emit valid ASCII characters in this function.
					buf[i] = unsafe { Char::from_u8_unchecked(b) };
					i += 1;
				}
			}

			if quote {
				buf[i] = c!(b'"');
				i += 1;
			}

			if f.alternate() {
				buf[i] = c!(b')');
				i += 1;
			}

			let s: &str = (&buf[..i]).as_str();
			f.write_str(s)
		}
	}
}

#[cfg(feature = "nightly")]
pub use self::nightly::*;
