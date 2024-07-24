OUTPUT_FORMAT(elf64-aarch64)
OUTPUT_ARCH(aarch64)

ENTRY(_start)

PHDRS {
	text     PT_LOAD    FLAGS((1 << 0) | (1 << 2)) ; /* rx */
	rodata   PT_LOAD    FLAGS((1 << 2)) ;            /* r */
	data     PT_LOAD    FLAGS((1 << 1) | (1 << 2)) ; /* rw */
	dynamic  PT_DYNAMIC FLAGS((1 << 1) | (1 << 2)) ; /* rw; Dynamic segment needed for PIE */
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
		*(.sdata .sdata.*)
	} :data

	. = ALIGN(4096);

    .dynamic : {
        *(.dynamic)
    } :data :dynamic

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
