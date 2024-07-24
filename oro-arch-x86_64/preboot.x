SECTIONS {
	.text : {
		. = ALIGN(4096);
		_ORO_STUBS_START = .;
		KEEP(*(.oro_xfer_stubs.entry));
		KEEP(*(.oro_xfer_stubs .oro_xfer_stubs.*));
		. = ALIGN(4096);
		_ORO_STUBS_LEN = . - _ORO_STUBS_START;
	} :text
}

INCLUDE "oro-arch-x86_64/arch.x"
