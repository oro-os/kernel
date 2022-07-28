pub fn init() {
	println!("cpu is x86_64");
	panic!("test message");
}

pub fn halt() {
	::x86_64::instructions::hlt();
}
