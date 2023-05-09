use oro_boot::x86_64 as boot;

pub fn init() {
	let boot_config = unsafe {
		&*(boot::l4_to_range_48(boot::ORO_BOOT_PAGE_TABLE_INDEX).0 as *const boot::BootConfig)
	};

	// Validate the magic number
	if boot_config.magic != oro_boot::BOOT_MAGIC {
		panic!("boot error (kernel): boot config magic number mismatch");
	}
	if boot_config.nonce_xor_magic != (oro_boot::BOOT_MAGIC ^ boot_config.nonce) {
		panic!("boot error (kernel): boot config magic^nonce mismatch");
	}
}
