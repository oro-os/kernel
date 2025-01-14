#![no_main]

#[no_mangle]
fn main() {
	loop {
		std::thread::yield_now();
	}
}
