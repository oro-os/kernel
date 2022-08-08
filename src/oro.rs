mod module;
mod ring;

use ring::Ring;

static mut ROOT_RING: Option<Ring> = None;

pub fn init() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);

	unsafe {
		ROOT_RING = Some(Ring::root());
		debug_assert!(ROOT_RING.as_ref().unwrap().id() == 0);
	}
	println!("root ring initialized");

	// TODO: Fill in the rest of the owl...

	unsafe {
		ROOT_RING = None;
	}
	println!("root ring destroyed");
}
