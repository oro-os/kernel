//! All registry tables supported in the Oro kernel.

pub mod thread_control_v0;

/// Creates a `u64` table ID from an 8 byte string literal.
macro_rules! table_id {
	($id:literal) => {{
		assert!($id.len() == 8, "table IDs must be 8 bytes long");

		let b = $id.as_bytes();
		(((b[0] as u64) << (8 * 7))
			| ((b[1] as u64) << (8 * 6))
			| ((b[2] as u64) << (8 * 5))
			| ((b[3] as u64) << (8 * 4))
			| ((b[4] as u64) << (8 * 3))
			| ((b[5] as u64) << (8 * 2))
			| ((b[6] as u64) << (8 * 1))
			| ((b[7] as u64) << (8 * 0)))
	}};
}

pub(crate) use table_id;
