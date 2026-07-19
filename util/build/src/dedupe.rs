//! Implemenhts the dedupe stream for multi-platform builds to not
//! show duplicate diagnostic lines.

use std::{
	collections::HashSet,
	hash::{Hash, Hasher},
};

/// A deduplication stream that filters out duplicate lines.
pub struct DedupeStream<'a, T>
where
	T: std::io::Write,
{
	inner: T,
	seen: &'a mut HashSet<u64>,
	buf: Vec<u8>,
}

impl<'a, T> DedupeStream<'a, T>
where
	T: std::io::Write,
{
	/// Creates a new deduplication stream that wraps the given writer.
	pub fn new(seen: &'a mut HashSet<u64>, inner: T) -> Self {
		Self {
			inner,
			seen,
			buf: Vec::new(),
		}
	}
}

impl<'a, T> std::io::Write for DedupeStream<'a, T>
where
	T: std::io::Write,
{
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		self.buf.extend_from_slice(buf);

		while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
			let line = self.buf.drain(..=pos).collect::<Vec<u8>>();
			let mut hasher = std::collections::hash_map::DefaultHasher::new();
			line.hash(&mut hasher);
			let hash = hasher.finish();

			if self.seen.insert(hash) {
				self.inner.write_all(&line)?;
			}
		}

		Ok(buf.len())
	}

	fn flush(&mut self) -> std::io::Result<()> {
		self.inner.flush()
	}
}
