#![no_std]
#![no_main]

use oro::{id::kernel::iface::ROOT_DEBUG_OUT_V0, syscall};

fn write_bytes(bytes: &[u8]) {
	if bytes.len() == 0 {
		return;
	}

	let mut word = bytes[0] as u64;

	for i in 1..bytes.len() {
		if i % 8 == 0 {
			// XXX(qix-): Hard coding the ID for a moment, bear with.
			syscall::set!(
				ROOT_DEBUG_OUT_V0,
				4294967296,
				0,
				syscall::key!("write"),
				word
			)
			.unwrap();
			word = 0;
		}

		word = (word << 8) | bytes[i] as u64;
	}

	if bytes.len() % 8 != 0 {
		// Shift it the remaining bits.
		word <<= 8 * (8 - (bytes.len() % 8));
		syscall::set!(
			ROOT_DEBUG_OUT_V0,
			4294967296,
			0,
			syscall::key!("write"),
			word
		)
		.unwrap();
	}
}

fn write_str(s: &str) {
	write_bytes(s.as_bytes());
}

#[no_mangle]
fn main() {
	write_str("Hello, Oro!\n");
}
