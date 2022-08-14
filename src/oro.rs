//! Entry point for the Oro kernel.
//!
//! All functionality herein, including all child modules,
//! are architecture-agnostic.

mod ring;

/// Main architecture-agnostic kernel entry point
///
/// Must only be called after the processor and all
/// loggers (framebuffer logger, serial logger, etc.)
/// are initialized.
///
/// Assumes a global allocator exists and is usable.
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
