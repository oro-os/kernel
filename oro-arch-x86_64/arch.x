SECTIONS {
	.text : {
		KEEP(*(.text.force_keep .text.force_keep.*));
	} :text
}
