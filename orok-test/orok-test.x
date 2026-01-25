SECTIONS {
	.orok_test_strings 0 (INFO) : {
		. = 0;
		_OROK_TEST_STRINGS_BASE = .;
		/* A literal null byte, such that the first offset (0) is always valid and a zero-length string. */
		BYTE(0)
		KEEP(*(.orok_test_strings .orok_test_strings.*))
		BYTE(0)
		_OROK_TEST_STRINGS_LIMIT = .;
	}
}
