//! Common functionality for the test-ports example.
#![no_std]

pub const TEST_PORTS_IFACE_ID: u64 = 1737937612428;
pub static TEST_PORTS_IFACE: ::oro::LazyIfaceId<TEST_PORTS_IFACE_ID> = ::oro::LazyIfaceId::new();

pub static mut TAG: &'static str = "unknown";

pub const PORT_BASE: usize = 0x20000000000;

pub fn set_consumer() {
	unsafe {
		TAG = "\x1b[95mconsumer";
	}
}

pub fn set_producer() {
	unsafe {
		TAG = "\x1b[96mproducer";
	}
}

#[macro_export]
macro_rules! println {
	($($tt:tt)*) => (
		// SAFETY: This is just a test.
		#[expect(static_mut_refs)]
		unsafe { ::oro::debug_out_v0_println!("test-ports::{}: {}\x1b[m", $crate::TAG, format_args!($($tt)*)); }
	)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
	println!("panic: {info:?}");
	// SAFETY: We're well aware this will terminate the thread or die trying.
	unsafe {
		::oro::terminate();
	}
}

#[macro_export]
macro_rules! test_ports_iface {
	(get $key:literal) => {{
		const KEY: u64 = ::oro::key!($key);
		match ::oro::syscall_get!(
			$crate::TEST_PORTS_IFACE_ID,
			$crate::TEST_PORTS_IFACE
				.get()
				.expect("test-ports iface not initialized"),
			0,
			KEY,
		) {
			Ok(v) => v,
			Err((e, ex)) => {
				panic!(
					"test-port GET failed: {:?} -> {e:?}[{:?}]",
					::oro::Key(&KEY),
					::oro::Key(&ex)
				);
			}
		}
	}};
	(set $key:literal => $val:expr) => {{
		const KEY: u64 = ::oro::key!($key);
		let val: u64 = $val;
		match ::oro::syscall_set!(
			$crate::TEST_PORTS_IFACE_ID,
			$crate::TEST_PORTS_IFACE
				.get()
				.expect("test-ports iface not initialized"),
			0,
			KEY,
			val
		) {
			Ok(v) => v,
			Err((e, ex)) => {
				panic!(
					"test-port SET failed: {:?} = {val:?} ({:?}) -> {e:?}[{:?}]",
					::oro::Key(&KEY),
					::oro::Key(&val),
					::oro::Key(&ex)
				);
			}
		}
	}};
}

#[macro_export]
macro_rules! mapper_iface {
	(get $idx:expr, $key:literal) => {{
		const KEY: u64 = ::oro::key!($key);
		let idx: u64 = $idx;
		match ::oro::syscall_get!(
			::oro::id::iface::KERNEL_MEM_TOKEN_V0,
			::oro::id::iface::KERNEL_MEM_TOKEN_V0,
			idx,
			KEY,
		) {
			Ok(v) => v,
			Err((e, ex)) => {
				panic!(
					"mapper-v0 GET failed: {idx:#016X}.{:?} -> {e:?}[{:?}]",
					::oro::Key(&KEY),
					::oro::Key(&ex)
				);
			}
		}
	}};
	(set $idx:expr, $key:literal => $val:expr) => {{
		const KEY: u64 = ::oro::key!($key);
		let val: u64 = $val;
		let idx: u64 = $idx;
		match ::oro::syscall_set!(
			::oro::id::iface::KERNEL_MEM_TOKEN_V0,
			::oro::id::iface::KERNEL_MEM_TOKEN_V0,
			idx,
			KEY,
			val
		) {
			Ok(v) => v,
			Err((e, ex)) => {
				panic!(
					"mapper-v0 SET failed: {idx:#016X}.{:?} = {val:?} ({:?}) -> {e:?}[{:?}]",
					::oro::Key(&KEY),
					::oro::Key(&val),
					::oro::Key(&ex)
				);
			}
		}
	}};
}
