pub fn init_oro() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);

	println!("bringing cpu online");
	crate::arch::init();
}
