#![no_std]
#![no_main]

use oro::debug_out_v0_println as println;

#[cfg(not(target_arch = "x86_64"))]
#[no_mangle]
fn main() {
	println!("This example only works on x86_64");
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
fn main() {
	use core::arch::asm;

	use oro::{id::iface::KERNEL_X86_64_TLS_BASE_V0, key, syscall_get, syscall_set};

	static THE_ANSWER: u64 = 42;

	println!("local answer: {THE_ANSWER}");

	let fsbase_name: u64 = syscall_get!(
		KERNEL_X86_64_TLS_BASE_V0,
		KERNEL_X86_64_TLS_BASE_V0,
		0,
		key!("name"),
	)
	.expect("failed to get FS base name");

	println!("fsbase name: {:?}", oro::Key(&fsbase_name));

	let fsbase = syscall_get!(
		KERNEL_X86_64_TLS_BASE_V0,
		KERNEL_X86_64_TLS_BASE_V0,
		0,
		key!("base"),
	)
	.expect("failed to get initial FS base");

	println!("initial FS base: {fsbase:#016x}");

	syscall_set!(
		KERNEL_X86_64_TLS_BASE_V0,
		KERNEL_X86_64_TLS_BASE_V0,
		0,
		key!("base"),
		&THE_ANSWER as *const _ as u64
	)
	.expect("failed to set FS base");

	println!("set FS base to: {:#016x}", &THE_ANSWER as *const _ as u64);

	let fsbase = syscall_get!(
		KERNEL_X86_64_TLS_BASE_V0,
		KERNEL_X86_64_TLS_BASE_V0,
		0,
		key!("base"),
	)
	.expect("failed to get final FS base");

	println!("final FS base: {fsbase:#016x}");

	let answer: u64;
	unsafe {
		asm!("mov rax, fs:[0]", out("rax") answer);
	}

	println!("read FS base: {answer}");
}
