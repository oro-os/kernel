#![no_std]
#![no_main]

use oro::{id::kernel::iface::ROOT_DEBUG_OUT_V0, syscall};

fn write_bytes(bytes: &[u8]) {
	if bytes.len() == 0 {
		return;
	}

	for chunk in bytes.chunks(8) {
		let mut word = 0u64;
		for b in chunk {
			word = (word << 8) | *b as u64;
		}

		// XXX(qix-): Hard coding the ID for a moment, bear with.
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
