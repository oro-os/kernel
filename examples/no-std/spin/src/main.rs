#![no_std]
#![no_main]

#[allow(unused_imports)]
use oro;

#[no_mangle]
fn main() {
	loop {
		::core::hint::spin_loop();
	}
}
