/*
	If you use `oro-boot` for booting the kernel (which is optional),
	it relies on the following segments being present.

	You should `INCLUDE` these in your linker script between the program
	headers and the sections. It expects there to be a `text` program
	header, and will emit a `.text` section if there isn't one.
*/

SECTIONS {
	.text : {
		. = ALIGN(4096);
		_ORO_STUBS_START = .;
		KEEP(*(.oro_xfer_stubs.entry));
		KEEP(*(.oro_xfer_stubs .oro_xfer_stubs.*));
		. = ALIGN(4096);
		_ORO_STUBS_LEN = . - _ORO_STUBS_START;

		KEEP(*(.force_keep .force_keep.*));
	} :text
}
