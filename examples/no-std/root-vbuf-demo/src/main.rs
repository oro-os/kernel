#![no_std]
#![no_main]

use oro::{
	id::iface::{ROOT_BOOT_VBUF_V0, ROOT_DEBUG_OUT_V0},
	key, syscall_get, syscall_set,
};

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
		syscall_set!(ROOT_DEBUG_OUT_V0, 4294967296, 0, key!("write"), word).unwrap();
	}
}

fn write_str(s: &str) {
	write_bytes(s.as_bytes());
}

struct Vbuf {
	width:          u64,
	height:         u64,
	stride:         u64,
	bits_per_pixel: u64,
	data:           *mut u8,
}

fn find_video_buffer(idx: u64) -> Result<Vbuf, (oro::syscall::Error, u64)> {
	macro_rules! get_vbuf_field {
		($field:literal) => {{
			syscall_get!(
				ROOT_BOOT_VBUF_V0,
				// XXX(qix-): Hardcoding the ID for now, bear with.
				4294967297,
				idx,
				key!($field),
			)?
		}};
	}

	let vbuf_addr: u64 = 0x60000000000 + (idx as u64) * 0x100000000;

	Ok(Vbuf {
		width:          get_vbuf_field!("width"),
		height:         get_vbuf_field!("height"),
		stride:         get_vbuf_field!("pitch"),
		bits_per_pixel: get_vbuf_field!("bit_pp"),
		data:           {
			syscall_set!(
				ROOT_BOOT_VBUF_V0,
				// XXX(qix-): Hardcoding the ID for now, bear with.
				4294967297,
				idx,
				key!("!vmbase!"),
				vbuf_addr
			)?;

			vbuf_addr as *mut u8
		},
	})
}

#[no_mangle]
fn main() {
	write_str("looking for vbuf 0...\n");
	if let Ok(vbuf) = find_video_buffer(0) {
		write_str("found vbuf 0\n");
		let bytes_per_pixel = vbuf.bits_per_pixel / 8;
		for y in 0..vbuf.height {
			for x in 0..(vbuf.width * bytes_per_pixel) {
				unsafe {
					*vbuf.data.offset(((y * vbuf.stride) + x) as isize) =
						if (((x + y) % 5) as u8) == 0 {
							0xFF
						} else {
							0x00
						};
				}
			}
		}
	} else {
		write_str("failed to find vbuf 0\n");
	}
}
