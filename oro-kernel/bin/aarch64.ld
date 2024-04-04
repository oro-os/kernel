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
	kernel_start = .;

	.text : {
		*(.text .text.*)
	} :text

	. = ALIGN(4096);

	.oro_stubs : {
		_ORO_STUBS_START = ALIGN(4096);
		KEEP(*(.oro_stubs.entry));
		KEEP(*(.oro_stubs .oro_stubs.*));
		_ORO_STUBS_END = ALIGN(4096);
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

    .dynamic : {
        *(.dynamic)
    } :data :dynamic

	.bss : {
		*(COMMON)
		*(.bss .bss.*) /* MUST be last allocated to :data */
	} :data

	/DISCARD/ : {
		*(.eh_frame)
		*(.note .note.*)
	}
}
