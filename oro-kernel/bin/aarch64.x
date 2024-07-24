OUTPUT_FORMAT(elf64-aarch64)
OUTPUT_ARCH(aarch64)

ENTRY(_start)

PHDRS {
	text     PT_LOAD    FLAGS((1 << 0) | (1 << 2) | (1 << 20)); /* rx + oro-kernel */
	rodata   PT_LOAD    FLAGS((1 << 2)            | (1 << 20)); /* r  + oro-kernel */
	data     PT_LOAD    FLAGS((1 << 1) | (1 << 2) | (1 << 20)); /* rw + oro-kernel */
	dynamic  PT_DYNAMIC FLAGS((1 << 1) | (1 << 2) | (1 << 20)); /* rw + oro-kernel; Dynamic segment needed for PIE */
}

SECTIONS {
	. = 0xFFFFFFFF80000000;
}

INCLUDE "oro-arch-aarch64/arch.x"

SECTIONS {
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

	. = ALIGN(4096);

	.debug_gdb_scripts : {
		KEEP(*(.debug_gdb_scripts .debug_gdb_scripts.*))
	} :text

	/DISCARD/ : {
		*(.eh_frame)
		*(.note .note.*)
	}
}
