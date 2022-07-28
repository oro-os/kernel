mod irq;

pub fn init() {
	println!("cpu is x86_64");
	irq::init();
	println!("... irq OK");
}

pub fn halt() {
	::x86_64::instructions::hlt();
}
