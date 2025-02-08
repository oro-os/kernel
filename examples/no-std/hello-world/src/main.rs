#![no_std]
#![no_main]

use oro::{
	id::iface::{KERNEL_IFACE_QUERY_BY_TYPE_V0, ROOT_DEBUG_OUT_V0},
	key, syscall_get, syscall_set,
};

fn write_bytes(bytes: &[u8]) {
	if bytes.len() == 0 {
		return;
	}

	// Get the iface ID for the root ring debug output interface.
	let Ok(iface) = syscall_get!(
		KERNEL_IFACE_QUERY_BY_TYPE_V0,
		KERNEL_IFACE_QUERY_BY_TYPE_V0,
		ROOT_DEBUG_OUT_V0,
		0
	) else {
		// No root debug output interface.
		return;
	};

	// Write to the line-buffered output interface if we found it.
	for chunk in bytes.chunks(8) {
		let mut word = 0u64;
		for b in chunk {
			word = (word << 8) | *b as u64;
		}

		syscall_set!(ROOT_DEBUG_OUT_V0, iface, 0, key!("write"), word).unwrap();
	}
}

fn write_str(s: &str) {
	write_bytes(s.as_bytes());
}

#[no_mangle]
fn main() {
	write_str("Hello, Oro!\n");
}
