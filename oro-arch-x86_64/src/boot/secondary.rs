//! Boot routines for secondary cores.

use crate::{
	lapic::Lapic,
	mem::{
		address_space::{AddressSpaceHandle, AddressSpaceLayout},
		segment::MapperHandle,
	},
};
use oro_common::mem::{
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::alloc::{PageFrameAllocate, PageFrameFree},
	translate::PhysicalAddressTranslator,
};
use oro_common_proc::asm_buffer;

/// The LA57 bit in the CR4 register.
// TODO(qix-): Pull this out into a register abstraction.
const CR4_LA57: u32 = 1 << 12;

/// The error type for booting a secondary core.
#[allow(dead_code)]
#[derive(Debug)]
pub enum BootError {
	/// The system is out of memory.
	OutOfMemory,
	/// An error occurred while mapping memory.
	MapError(MapError),
	/// An error occurred while unmapping memory (probably the stack guard pages)
	UnmapError(UnmapError),
}

/// Boots a secondary core with the given local APIC and LAPIC ID.
///
/// # Safety
/// Uses the page at physical address 0x8000 as the secondary core's entry point,
/// and the page at physical address 0x9000 as the secondary core's L4 page table.
///
/// Caller must ensure these pages are mapped (via the PAT) and accessible.
#[allow(clippy::missing_docs_in_private_items)]
pub unsafe fn boot_secondary<P: PhysicalAddressTranslator, A: PageFrameAllocate + PageFrameFree>(
	primary_handle: &AddressSpaceHandle,
	pfa: &mut A,
	pat: &P,
	lapic: &Lapic,
	secondary_lapic_id: u8,
	stack_pages: usize,
) -> Result<(), BootError> {
	// Some of these values aren't exact, but we want to align things nicely.
	const ACTUAL_STACK_PTR_SIZE: usize = 8;
	const ACTUAL_CR3_PTR_SIZE: usize = 8;
	const ACTUAL_CR4_VALUE_SIZE: usize = 8;
	const ACTUAL_CR0_VALUE_SIZE: usize = 8;
	const ENTRY_POINT_SIZE: usize = 8;
	const NULLIDT_SIZE: usize = 8; // Really 6
	const CR4BITS_SIZE: usize = 8; // Really 4
	const GDTR_SIZE: usize = 8; // Really 6
	const TOP_RESERVE: usize = ACTUAL_STACK_PTR_SIZE
		+ ACTUAL_CR3_PTR_SIZE
		+ ACTUAL_CR4_VALUE_SIZE
		+ ACTUAL_CR0_VALUE_SIZE
		+ ENTRY_POINT_SIZE
		+ NULLIDT_SIZE
		+ CR4BITS_SIZE
		+ GDTR_SIZE;

	// Make sure the stubs fit in the first half of the page...
	debug_assert!(SECONDARY_BOOT_STUB.len() <= 0x400);
	debug_assert!(SECONDARY_BOOT_LONG_MODE_STUB.len() <= 0x400);

	// ... and that the GDT fits in the second part (minus TOP_RESERVE bytes).
	let gdt_slice = crate::gdt::gdt_bytes();
	debug_assert!(gdt_slice.len() <= (0x800 - TOP_RESERVE));

	// Create a new supervisor address space based on the current address space.
	let mapper = AddressSpaceLayout::duplicate_supervisor_space_shallow(primary_handle, pfa, pat)
		.ok_or(BootError::OutOfMemory)?;

	// Direct-map the code segment into the secondary core's address space.
	// This allows the code to still execute after switching to paging mode.
	AddressSpaceLayout::secondary_boot_stub_code()
		.map(&mapper, pfa, pat, 0x8000, 0x8000)
		.map_err(BootError::MapError)?;

	// Create a stack and map it into the secondary core's address space.
	let kernel_stack_segment = AddressSpaceLayout::kernel_stack();

	let last_stack_page_virt = kernel_stack_segment.range().1 & !0xFFF;

	// "Forget" the entire segment (meaning simply to unmap but not
	// reclaim any mappings). It still reclaims the top level pages
	// but leaves the lower level pages alone.
	kernel_stack_segment.forget_duplicated(&mapper, pfa, pat);

	// make sure top guard page is unmapped
	match kernel_stack_segment.unmap(&mapper, pfa, pat, last_stack_page_virt) {
		// NOTE(qix-): The Ok() case would never hit here since we explicitly unmapped the entire segment.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => return Err(BootError::UnmapError(e)),
	}

	let mut bottom_stack_page_virt = last_stack_page_virt;
	for stack_page_idx in 0..stack_pages {
		bottom_stack_page_virt -= 4096;

		let stack_phys = pfa
			.allocate()
			.ok_or(BootError::MapError(MapError::OutOfMemory))?;

		// We map it into two places; the _real_ location in higher half
		// memory where the stack will ultimately reside, as well as
		// the first page of the stack in the lower half of memory
		// since we don't have an RSP to set up but rather have to
		// rely on ESP (a 32-bit register) until the long mode trampoline
		// is hit.
		kernel_stack_segment
			.remap(&mapper, pfa, pat, bottom_stack_page_virt, stack_phys)
			.map_err(BootError::MapError)?;

		if stack_page_idx == 0 {
			AddressSpaceLayout::secondary_boot_stub_stack()
				.remap(&mapper, pfa, pat, 0x20000, stack_phys)
				.map_err(BootError::MapError)?;
		}
	}

	// Make sure that the bottom guard page is unmapped
	match kernel_stack_segment.unmap(&mapper, pfa, pat, bottom_stack_page_virt - 4096) {
		// NOTE(qix-): The Ok() case would never hit here isnce we explicitly unmapped the entire segment.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => return Err(BootError::UnmapError(e)),
	}

	// The 32-bit stack pointer is at 0x20000 + 4096 = 0x21000.
	// This variable holds the long mode stack pointer that needs
	// to be switched to when the long mode stubs start.
	let stack_ptr = last_stack_page_virt;

	// Copy the mapper into a well-known page (0x9000).
	AddressSpaceLayout::copy_shallow_into(&mapper, 0x9000, pat);

	// Write the stubs into the first half of the page.
	// They live at 0x8000 (CS:IP = 0x0800:0x0000) and
	// 0x8000 + 0x400 (CS:IP = 0x0800:0x0400) for the
	// 16-bit and 64-bit stubs, respectively.
	let stub_virt = pat.to_virtual_addr(0x8000);
	let stub_slice =
		core::slice::from_raw_parts_mut(stub_virt as *mut u8, SECONDARY_BOOT_STUB.len());
	stub_slice.copy_from_slice(SECONDARY_BOOT_STUB);

	let long_mode_stub_virt = pat.to_virtual_addr(0x8000 + 0x400);
	let long_mode_stub_slice = core::slice::from_raw_parts_mut(
		long_mode_stub_virt as *mut u8,
		SECONDARY_BOOT_LONG_MODE_STUB.len(),
	);
	long_mode_stub_slice.copy_from_slice(SECONDARY_BOOT_LONG_MODE_STUB);

	// Write the GDT to the second half of the page.
	// It lives at 0x8000 + 0x800 (CS:IP = 0x0800:0x0800).
	let gdt_virt = pat.to_virtual_addr(0x8000 + 0x800);
	let secondary_gdt = core::slice::from_raw_parts_mut(gdt_virt as *mut u8, gdt_slice.len());
	secondary_gdt.copy_from_slice(gdt_slice);

	let mut meta_ptr = 0x9000 - TOP_RESERVE;

	// Write the absolute entry point address of the Rust stub.
	debug_assert_eq!(meta_ptr, 0x8FC0);
	let entry_point_ptr = oro_kernel_x86_64_rust_secondary_core_entry as *const u8 as u64;
	let entry_point_virt = pat.to_virtual_addr(meta_ptr as u64);
	(entry_point_virt as *mut u64).write(entry_point_ptr);
	meta_ptr += ENTRY_POINT_SIZE;

	// Write the actual CR0 value so that the long mode stub can install it.
	debug_assert_eq!(meta_ptr, 0x8FC8);
	let cr0_ptr_virt = pat.to_virtual_addr(meta_ptr as u64);
	let cr0_value: u64 = crate::reg::Cr0::read().into();
	(cr0_ptr_virt as *mut u64).write(cr0_value);
	meta_ptr += ACTUAL_CR0_VALUE_SIZE;

	// Write the actual CR4 value so that the long mode stub can install it.
	debug_assert_eq!(meta_ptr, 0x8FD0);
	let cr4_ptr_virt = pat.to_virtual_addr(meta_ptr as u64);
	let cr4_value = crate::asm::cr4();
	(cr4_ptr_virt as *mut u64).write(cr4_value);
	meta_ptr += ACTUAL_CR4_VALUE_SIZE;

	// Write the actual CR3 value so that the long mode stub can switch to it.
	debug_assert_eq!(meta_ptr, 0x8FD8);
	let cr3_ptr_virt = pat.to_virtual_addr(meta_ptr as u64);
	let cr3_phys = mapper.base_phys();
	(cr3_ptr_virt as *mut u64).write(cr3_phys);
	meta_ptr += ACTUAL_CR3_PTR_SIZE;

	// Write the real stack pointer so that the long mode stub can switch to it.
	debug_assert_eq!(meta_ptr, 0x8FE0);
	let stack_ptr_virt = pat.to_virtual_addr(meta_ptr as u64);
	(stack_ptr_virt as *mut u64).write(stack_ptr as u64);
	meta_ptr += ACTUAL_STACK_PTR_SIZE;

	// Zero the last 8 bytes of the page for the null IDT.
	debug_assert_eq!(meta_ptr, 0x8FE8);
	let idt_virt = pat.to_virtual_addr(meta_ptr as u64);
	core::ptr::write_bytes(idt_virt as *mut u8, 0, NULLIDT_SIZE);
	meta_ptr += NULLIDT_SIZE;

	// Extract out the interesting bits of CR4 for the secondary core.
	// We only support extracting the LA57 bit for now.
	debug_assert_eq!(meta_ptr, 0x8FF0);
	let cr4_bits_virt = pat.to_virtual_addr(meta_ptr as u64);
	let cr4_bits = (crate::asm::cr4() as u32) & CR4_LA57;
	(cr4_bits_virt as *mut u32).write(cr4_bits);
	meta_ptr += CR4BITS_SIZE;

	// Write the GDT pointer into the last 6 bytes of the page.
	debug_assert_eq!(meta_ptr, 0x8FF8);
	let gdt_ptr_virt = pat.to_virtual_addr(meta_ptr as u64);
	let gdt_ptr: u32 = 0x8000 + 0x800;
	(gdt_ptr_virt as *mut u16).write(
		u16::try_from(gdt_slice.len() - 1).expect("GDT is too large for the GDTR limit value"),
	);
	(gdt_ptr_virt as *mut u16)
		.add(1)
		.cast::<u32>()
		.write_unaligned(gdt_ptr);
	meta_ptr += GDTR_SIZE;

	debug_assert_eq!(meta_ptr, 0x9000);

	// Finally, tell the processor to start executing at page 8 (0x8000).
	// NOTE(qix-): Specifying other pages doesn't seem to work. The documentation
	// NOTE(qix-): surrounding the LAPIC SIPI interrupts are full of holes and
	// NOTE(qix-): 404's from 20+ years ago. If you have any more information on
	// NOTE(qix-): how to make this work a bit cleaner (e.g. not requiring 0x8000
	// NOTE(qix-): hard-coded and instead allowing us to take any page < 256),
	// NOTE(qix-): I'd love to hear all about it.
	lapic.boot_core(secondary_lapic_id, 8);

	Ok(())
}

/// The secondary boot stub machine code.
///
/// This is more or less adapted from the direct-to-long-mode boot stub
/// provided by Brendan from the OSDev Wiki. It's not supposed to work
/// as per the AMD documentation, but it does.
const SECONDARY_BOOT_STUB: &[u8] = &asm_buffer! {
	// 16-bit code starts here
	".code16",

	// Mask off all IRQs with the LAPIC.
	"cli",
	"mov al, 0xFF",
	"out 0xA1, al",
	"out 0x21, al",
	"nop",
	"nop",

	// Load the zero IDT. This makes all NMIs
	// cause a triple fault.
	//
	// The zero IDT has been placed at 0x8FF0
	// by the primary core.
	"lidt [0x8FE8]",

	// Set the PAE and PGE bits in CR4,
	// as well as any others that the primary
	// core has set that we're interested in.
	"mov eax, 10100000b",
	"mov ebx, [0x8FF0]",
	"or eax, ebx",
	"mov cr4, eax",

	// Load the top level page table.
	// The primary core has placed the L4 page table
	// at 0x9000.
	"mov edx, 0x9000",
	"mov cr3, edx",

	// Set the LME and NXE bits in EFER.
	//
	// LME tells the CPU we want Long Mode.
	//
	// NXE tells the CPU to allow us to use the
	// no-execute (NX) bit in the page tables.
	// If we don't enable that, and we use NX,
	// the CPU will fault with a #GP(0xD) with
	// an error code of 0xA (1010b, reserved bit
	// set).
	"xor eax, eax",
	"mov ecx, 0xC0000080",
	"rdmsr",
	"or eax, 0x00000900",
	"wrmsr",

	// Activate long mode by setting
	// both the paging and protected mode
	// bits at the same time in CR0.
	"mov ebx, cr0",
	"or ebx, 0x80000001",
	"mov cr0, ebx",

	// Load the GDT.
	"lgdt [0x8FF8]",

	// Set the stack pointer; the primary core
	// has placed the stack at 0x20000, plus a
	// page, so we can use the stack pointer
	// at 0x21000.
	"mov esp, 0x21000",

	// We can now jump to the long mode stub.
	"ljmp 0x0008, 0x8400",
};

const SECONDARY_BOOT_LONG_MODE_STUB: &[u8] = &asm_buffer! {
	// 64-bit code starts here
	".code64",

	// Set the real stack pointer.
	"mov rax, [0x8FE0]",
	"mov rsp, rax",

	// Set the real CR3 value.
	"mov rax, [0x8FD8]",
	"mov cr3, rax",

	// Install the real CR0 value.
	"mov rax, [0x8FC8]",
	"mov cr0, rax",

	// Install the real CR4 value.
	"mov rax, [0x8FD0]",
	"mov cr4, rax",

	// Jump to the Rust kernel secondary core entry point.
	"push 0",
	"mov rax, [0x8FC0]",
	"jmp rax",
};

#[no_mangle]
unsafe extern "C" fn oro_kernel_x86_64_rust_secondary_core_entry() -> ! {
	crate::gdt::install_gdt();

	oro_debug::dbg!("welcome to the party :D");

	// XXX TODO
	<crate::X86_64 as oro_common::arch::Arch>::halt();
}