mod ring;

pub fn init() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);

	unsafe {
		ring::init_root();
		println!("root ring initialized");
	}

	// TODO: Fill in the rest of the owl...
	{
		let root_id = ring::root().id();
		println!("root id = {}", root_id);

		if let Some(ring_0) = ring::get_ring_by_id(0usize) {
			println!("ring_0 id = {}", ring_0.id());
		} else {
			println!("no ring 0 found!");
		}

		if let Some(ring_1) = ring::get_ring_by_id(1usize) {
			println!("ring_1 id = {}", ring_1.id());
		} else {
			println!("no ring 1 found!");
		}
	}

	unsafe {
		ring::drop_root();
		println!("root ring destroyed");
	}
}
