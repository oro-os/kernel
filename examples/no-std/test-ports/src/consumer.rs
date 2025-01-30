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

	loop {
		let offset = counter & OFFSET_MASK;
		let entry_base = unsafe { base.add(FIELD_COUNT * offset) };

		while unsafe { entry_base.read_volatile() } == 0 {
			::core::hint::spin_loop();
		}

		unsafe {
			let tag = entry_base.read_volatile();
			::core::hint::black_box(tag);
			for i in 1..FIELD_COUNT {
				let value = entry_base.add(i).read_volatile();
				::core::hint::black_box(value);
			}

			entry_base.write_volatile(0);
		}

		counter += 1;

		if counter % 10_000_000 == 0 {
			println!("acknowledged {counter} entries");
		}
	}
}
