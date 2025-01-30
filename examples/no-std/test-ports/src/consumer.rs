#![no_std]
#![no_main]

use example_test_ports::*;

#[no_mangle]
fn main() {
	set_consumer();

	let test = test_ports_iface!(get "health");
	println!("tested iface... {test:?} (should be 1337)");

	let token = test_ports_iface!(get "cnsmtkn");
	println!("got token... {token:#016X}");

	let ty = mapper_iface!(get token, "type");
	println!("token type is: {:?}", Key(&ty));

	let subty = mapper_iface!(get token, "subtype");
	println!("token subtype is: {:?}", Key(&subty));

	mapper_iface!(set token, "base" => PORT_BASE as u64);
	println!("mapped token to base: {PORT_BASE:#016X}");

	let base = PORT_BASE as *mut u64;
	let mut counter = 0;

	// NOTE(qix-): We configured the port to have 2 fields. Therefore there are
	// NOTE(qix-): 256 entries of 2 u64's. In the future we'll be able to
	// NOTE(qix-): confirm this, as well as enforce compatibility at the kernel level.

	loop {
		let offset = counter & 0xFF;

		if unsafe { base.add(2 * offset).read_volatile() } == 0 {
			println!("drained; waiting");
		}

		while unsafe { base.add(2 * offset).read_volatile() } == 0 {
			::core::hint::spin_loop();
		}

		unsafe {
			let tag = base.add(2 * offset).read_volatile();
			let value = base.add(2 * offset + 1).read_volatile();
			println!("read entry {counter} with tag {tag:#016X} and value {value}");

			for _ in 0..10000 {
				::core::hint::spin_loop();
			}

			base.add(2 * offset).write_volatile(0);
			println!("acknowledged entry {counter}");
		}

		counter += 1;

		for _ in 0..350_000 {
			::core::hint::spin_loop();
		}
	}
}
