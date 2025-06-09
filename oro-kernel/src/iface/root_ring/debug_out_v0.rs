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

use core::{cmp::Ordering, marker::PhantomData};

use oro::{key, syscall::Error as SysError};
use oro_kernel_mem::alloc::vec::Vec;

use crate::{
	arch::Arch, interface::Interface, syscall::InterfaceResponse, tab::Tab, thread::Thread,
};

/// The default line buffer size.
const DEFAULT_LINE_BUFFER: usize = 256;
/// The hard-coded maximum line buffer size.
const HARD_MAXIMUM: u64 = 1024;
/// The hard-coded minimum line buffer size.
const HARD_MINIMUM: u64 = 1;

/// Inner state of the debug output stream.
struct BufferState {
	/// The number of bytes to line buffer before
	/// forcing a flush.
	line_buffer: usize,
	/// Buffer for line buffering.
	buffer:      Vec<u8>,
}

impl Default for BufferState {
	fn default() -> Self {
		Self {
			line_buffer: DEFAULT_LINE_BUFFER,
			buffer:      Vec::new(),
		}
	}
}

impl Drop for BufferState {
	fn drop(&mut self) {
		self.flush();
	}
}

impl BufferState {
	/// Writes a single byte to the buffer.
	///
	/// Automatically flushes the buffer if a newline
	/// is encountered or if `line_buffer` bytes are
	/// written.
	fn write(&mut self, b: u8) {
		match b {
			0 => (),
			b'\n' => self.flush(),
			_ => {
				if self.buffer.len() >= self.line_buffer {
					self.flush();
				}
				self.buffer.push(b);
			}
		}
	}

	/// Flushes the buffer immediately.
	fn flush(&mut self) {
		#[cfg(debug_assertions)]
		::oro_debug::log_debug_bytes(&self.buffer);
		self.buffer.clear();
	}

	/// Sets the buffer's max size to `size`.
	///
	/// The buffer is truncated if it is larger than
	/// `size`, and if the populated bytes exceed
	/// `size`, the buffer is flushed.
	fn set_max(&mut self, size: usize) {
		if self.buffer.len() > size {
			self.flush();
		}

		match self.buffer.capacity().cmp(&size) {
			Ordering::Greater => self.buffer.shrink_to(size),
			Ordering::Less => self.buffer.reserve(size - self.buffer.capacity()),
			Ordering::Equal => {}
		}

		self.line_buffer = size;
	}
}

/// See the module level documentation for information about
/// the debug output stream interface.
pub struct DebugOutV0<A: Arch>(PhantomData<A>);

impl<A: Arch> DebugOutV0<A> {
	/// Creates a new `DebugOutV0` instance.
	#[must_use]
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<A: Arch> Interface<A> for DebugOutV0<A> {
	fn type_id(&self) -> u64 {
		oro::id::iface::ROOT_DEBUG_OUT_V0
	}

	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		}

		match key {
			key!("write") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
			key!("line_max") => {
				InterfaceResponse::ok(thread.with(|t| {
					t.data()
						.try_get::<BufferState>()
						.map_or_else(|| DEFAULT_LINE_BUFFER, |b| b.line_buffer) as u64
				}))
			}
			key!("hard_max") => InterfaceResponse::ok(HARD_MAXIMUM),
			key!("hard_min") => InterfaceResponse::ok(HARD_MINIMUM),
			key!("ring_sz") => InterfaceResponse::ok(oro_debug::ring_buffer_len() as u64),
			key!("ring_u64") => InterfaceResponse::ok(oro_debug::ring_buffer_read()),
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}

	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		}

		match key {
			key!("line_max") => {
				let value = value.clamp(HARD_MINIMUM, HARD_MAXIMUM);
				thread.with_mut(|t| t.data_mut().get::<BufferState>().set_max(value as usize));
				InterfaceResponse::ok(0)
			}
			key!("write") => {
				// The value itself holds the bytes; we consume each of the 8 bytes
				// from `value` until we encounter a `0` byte, after which we ignore
				// the rest.
				let bytes = value.to_be_bytes();
				thread.with_mut(|t| {
					let inner = t.data_mut().get::<BufferState>();

					for b in bytes {
						inner.write(b);
					}
				});

				InterfaceResponse::ok(0)
			}
			key!("hard_max") | key!("hard_min") | key!("ring_sz") | key!("ring_u64") => {
				InterfaceResponse::immediate(SysError::ReadOnly, 0)
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
