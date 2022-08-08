use alloc::boxed::Box;

const SIZE: usize = 4096 * 2;

fn allocate_boxed() -> Box<[u8; SIZE]> {
	let mut outer_boxed = unsafe {
		use alloc::alloc::{alloc, Layout};
		let layout = Layout::new::<[u8; SIZE]>();
		let ptr = alloc(layout) as *mut [u8; SIZE];
		if ptr.is_null() {
			crate::alloc_error_handler(layout);
		}
		println!("got allocation: {:#16x}", ptr as usize);
		(*ptr)[0] = 42;
		println!("wrote: {}", (*ptr)[0]);
		(*ptr)[0] = 69;
		println!("wrote: {}", (*ptr)[0]);
		let boxed = Box::from_raw(ptr);
		println!("boxed: {}", boxed[0]);
		boxed
	};

	println!("outer_boxed: {}", outer_boxed[0]);
	outer_boxed[0] = 101;
	println!("outer_boxed wrote: {}", outer_boxed[0]);
	outer_boxed
}

pub fn init() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);

	println!("FIRST:");
	let _first = allocate_boxed();
	println!("SECOND:");
	let _second = allocate_boxed();
}
