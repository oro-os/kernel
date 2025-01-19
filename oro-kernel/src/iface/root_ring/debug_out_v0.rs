//! Low level debug output stream root ring interface.
//!
//! This interface allows **very** primitive debug output
//! to be sent to the same debug output stream that the
//! kernel itself uses.
//!
//! The debug stream is forced as line buffered.
//!
//! Lines still follow the same rules as the kernel's
//! debug logging, except that the filename is replaced
//! with the string `<module>:0`.
//!
//! **Streams are not interpreted as UTF-8.** They are
//! outputted to the debug stream as-is.

use core::marker::PhantomData;

use oro_mem::alloc::vec::Vec;
use oro_sync::{Lock, Mutex};
use oro_sysabi::{key, syscall::Error as SysError};

use crate::{
	arch::Arch,
	interface::{Interface, InterfaceResponse, SystemCallResponse},
	tab::Tab,
	thread::Thread,
};

/// The hard-coded maximum line buffer size.
const HARD_MAXIMUM: u64 = 1024;
/// The hard-coded minimum line buffer size.
const HARD_MINIMUM: u64 = 1;

/// Inner state of the debug output stream.
struct Inner {
	/// The number of bytes to line buffer before
	/// forcing a flush.
	line_buffer: usize,
	/// Buffer for line buffering.
	buffer:      Vec<u8>,
}

impl Default for Inner {
	fn default() -> Self {
		Self {
			line_buffer: 256,
			buffer:      Vec::new(),
		}
	}
}

/// See the module level documentation for information about
/// the debug output stream interface.
pub struct DebugOutV0<A: Arch>(Mutex<Inner>, PhantomData<A>);

impl<A: Arch> DebugOutV0<A> {
	/// Creates a new `DebugOutV0` instance.
	#[must_use]
	pub fn new() -> Self {
		Self(Mutex::new(Inner::default()), PhantomData)
	}
}

impl<A: Arch> Interface<A> for DebugOutV0<A> {
	fn type_id(&self) -> u64 {
		oro_sysabi::id::iface::ROOT_DEBUG_OUT_V0
	}

	fn get(&self, _thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadIndex,
				ret:   0,
			});
		}

		match key {
			key!("write") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::WriteOnly,
					ret:   0,
				})
			}
			key!("line_max") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   self.0.lock().line_buffer as u64,
				})
			}
			key!("hard_max") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   HARD_MAXIMUM,
				})
			}
			key!("hard_min") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   HARD_MINIMUM,
				})
			}
			_ => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::BadKey,
					ret:   0,
				})
			}
		}
	}

	fn set(&self, _thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadIndex,
				ret:   0,
			});
		}

		match key {
			key!("line_max") => {
				let value = value.clamp(HARD_MINIMUM, HARD_MAXIMUM);

				self.0.lock().line_buffer = value as usize;

				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   0,
				})
			}
			key!("write") => {
				// The value itself holds the bytes; we consume each of the 8 bytes
				// from `value` until we encounter a `0` byte, after which we ignore
				// the rest.
				let bytes = value.to_be_bytes();
				let mut inner = self.0.lock();

				for b in bytes {
					let flush = match b {
						0 => continue,
						b'\n' => true,
						_ => {
							inner.buffer.push(b);
							inner.buffer.len() >= inner.line_buffer
						}
					};

					if flush {
						::oro_debug::log_debug_bytes(&inner.buffer);
						inner.buffer.clear();
					}
				}

				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   0,
				})
			}
			key!("hard_max") | key!("hard_min") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::ReadOnly,
					ret:   0,
				})
			}
			_ => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::BadKey,
					ret:   0,
				})
			}
		}
	}
}
