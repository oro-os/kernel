#![no_std]
#![no_main]

use example_test_ports::*;

#[no_mangle]
fn main() {
	set_producer();

	let test = test_ports_iface!(get "health");
	println!("tested iface... {test:?} (should be 1337)");

	let token = test_ports_iface!(get "prodtkn");
	println!("got token... {token:#016X}");

	let ty = mapper_iface!(get token, "type");
	println!("token type is: {:?}", Key(&ty));

	let subty = mapper_iface!(get token, "subtype");
	println!("token subtype is: {:?}", Key(&subty));

	mapper_iface!(set token, "base" => PORT_BASE as u64);
	println!("mapped token to base: {PORT_BASE:#016X}");
}
