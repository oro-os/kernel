pub fn init() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);
}
