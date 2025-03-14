//! Implements a simple ring buffer.

/// Simple byte ring buffer.
pub struct RingBuffer<const SZ: usize> {
	/// The ring buffer contents
	bytes:        [u8; SZ],
	/// The current write offset of the ring buffer.
	write_offset: usize,
	/// The current read offset of the ring buffer.
	read_offset:  usize,
}

impl<const SZ: usize> Default for RingBuffer<SZ> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl<const SZ: usize> RingBuffer<SZ> {
	/// Creates a new ring buffer.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			bytes:        [0; SZ],
			write_offset: 0,
			read_offset:  0,
		}
	}

	/// Reads a single `u64` from the ring,
	/// whereby up to 8 bytes of the buffer are
	/// shifted into a final u64.
	///
	/// Short reads have the remaining bytes
	/// shifted in as zeros.
	#[must_use]
	pub fn read(&mut self) -> u64 {
		let read_relative_write_offset = if self.write_offset < self.read_offset {
			self.write_offset + SZ
		} else {
			self.write_offset
		};
		let total_read = (read_relative_write_offset - self.read_offset).min(8);

		if total_read == 0 {
			return 0;
		}

		let mut r = 0;

		for i in 0..total_read {
			let b = self.bytes[(self.read_offset + i) % SZ];
			r = (r << 8) | u64::from(b);
		}

		self.read_offset = (self.read_offset + total_read) % SZ;

		r << (8 * (8 - total_read))
	}

	/// Brings a bag of bytes into the buffer,
	/// boldly bumping the read pointer to beat burgeoning backpressure.
	pub fn write(&mut self, bytes: &[u8]) {
		// TODO(qix-): This is really inefficient; is there a better way of doing this?
		for b in bytes {
			self.bytes[self.write_offset] = *b;
			self.write_offset = (self.write_offset + 1) % SZ;
			if self.write_offset == self.read_offset {
				self.read_offset = (self.read_offset + 1) % SZ;
			}
		}
	}
}

impl<const SZ: usize> core::fmt::Write for RingBuffer<SZ> {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		self.write(s.as_bytes());
		Ok(())
	}
}
