OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_start)

PHDRS {
	text     PT_LOAD    FLAGS((1 << 0) | (1 << 2)) ; /* rx */
	rodata   PT_LOAD    FLAGS((1 << 2)) ;            /* r */
	data     PT_LOAD    FLAGS((1 << 1) | (1 << 2)) ; /* rw */
}

SECTIONS {
	. = 0xFFFFFFFF80000000;

	.text : {
		*(.text .text.*)
	} :text

	. = ALIGN(4096);

	.rodata : {
		*(.rodata .rodata.*)
	} :rodata

	. = ALIGN(4096);

	.data : {
		*(.data .data.*)
	} :data

	. = ALIGN(4096);

	.bss : {
		*(COMMON)
		*(.bss .bss.*) /* MUST be last allocated to :data */
	} :data

	/DISCARD/ : {
		*(.eh_frame)
		*(.note .note.*)
	}
}
