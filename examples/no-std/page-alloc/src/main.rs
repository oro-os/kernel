#![no_std]
#![no_main]

use oro::{
	Key, debug_out_v0_println as println,
	id::kernel::iface::{KERNEL_MEM_TOKEN_V0, KERNEL_PAGE_ALLOC_V0},
	syscall,
};

#[no_mangle]
fn main() {
	// Allocate a single 4KiB page.
	let token = match syscall::get!(
		KERNEL_PAGE_ALLOC_V0,
		KERNEL_PAGE_ALLOC_V0,
		syscall::key!("4kib"),
		1
	) {
		Ok(token) => token,
		Err((e, ex)) => {
			println!("error: {e:?}[{ex}]");
			return;
		}
	};

	println!("allocated token: {token:#X}");

	// Confirm its type.
	let ty = match syscall::get!(
		KERNEL_MEM_TOKEN_V0,
		KERNEL_MEM_TOKEN_V0,
		token,
		syscall::key!("type")
	) {
		Ok(ty) => ty,
		Err((e, ex)) => {
			println!("error: {e:?}[{ex}]");
			return;
		}
	};

	println!("token type: {:?}", Key(&ty));

	const TARGET_ADDR: u64 = 0x400_0000_0000;

	// Map it in.
	match syscall::set!(
		KERNEL_MEM_TOKEN_V0,
		KERNEL_MEM_TOKEN_V0,
		token,
		syscall::key!("base"),
		TARGET_ADDR
	) {
		Ok(_) => (),
		Err((e, ex)) => {
			println!("error mapping in token: {e:?}[{ex}]");
			return;
		}
	}

	// Try to read and write from it.
	unsafe {
		*(TARGET_ADDR as *mut u64) = 0x1234_5678_9ABC_DEF0;
		let value = *(TARGET_ADDR as *const u64);
		println!("value: {value:#016X}");
	}
}
