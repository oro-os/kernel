OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_start)

PHDRS {
	oro_boot PT_LOAD    FLAGS((1 << 2)            | (1 << 20) | (1 << 21)); /* r  + oro-kernel + oro-boot */
	text     PT_LOAD    FLAGS((1 << 0) | (1 << 2) | (1 << 20)            ); /* rx + oro-kernel */
	rodata   PT_LOAD    FLAGS((1 << 2)            | (1 << 20)            ); /* r  + oro-kernel */
	data     PT_LOAD    FLAGS((1 << 1) | (1 << 2) | (1 << 20)            ); /* rw + oro-kernel */
}

SECTIONS {
	. = 0xFFFFFFFF80000000;

	/* Put these as early as possible. */
	.oro_boot : {
		KEEP(*(.oro_boot .oro_boot.*))
	} :oro_boot

	. = ALIGN(4096);

}

INCLUDE "oro-arch-x86_64/arch.x"

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
	} :data

	. = ALIGN(4096);

	.debug_gdb_scripts : {
		KEEP(*(.debug_gdb_scripts .debug_gdb_scripts.*))
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
