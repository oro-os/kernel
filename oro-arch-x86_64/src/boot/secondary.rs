//! Boot routines for secondary cores.

use core::{
	cell::UnsafeCell,
	mem::{ManuallyDrop, offset_of},
	sync::atomic::{
		AtomicBool, AtomicU64,
		Ordering::{Acquire, Relaxed, Release},
	},
};

use oro_acpi::{Madt, Rsdp};
use oro_boot_protocol::acpi::AcpiKind;
use oro_debug::{dbg, dbg_err};
use oro_macro::{asm_buffer, assert};
use oro_mem::{
	alloc::sync::Arc,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use crate::{
	lapic::Lapic,
	mem::{
		address_space::{AddressSpaceHandle, AddressSpaceLayout},
		segment::MapperHandle,
	},
	time::GetInstant,
};

/// Indicates that the primary has finished initializing the core state
/// and that the secondaries are now free to boot.
///
/// We have to do this since the control of the LAPIC is singular (i.e.
/// non-`Clone`-able), the core-local kernel needs an owning reference to it,
/// but the secondary boot sequence needs to use it to send SIPIs to the secondaries.
///
/// However, if the secondaries boot before the primary has finished initializing
/// and they try to reference the global kernel state, it'll invoke UB. So we have
/// to barrier them until all of the cores are booted, the primary has initialized
/// the global kernel state, and _then_ they can start.
pub(super) static SECONDARIES_MAY_BOOT: AtomicBool = AtomicBool::new(false);

/// The error type for booting a secondary core.
#[derive(Debug)]
pub enum BootError {
	/// The system is out of memory.
	OutOfMemory,
	/// An error occurred while mapping memory.
	MapError(MapError),
	/// An error occurred while unmapping memory (probably the stack guard pages)
	UnmapError(UnmapError),
	/// The secondary errored out with the given value.
	SecondaryError(u64),
	/// Timed out waiting for the secondary to boot.
	SecondaryTimeout,
}

/// Secondary boot metadata structure.
#[repr(C, align(4096))]
struct BootMeta {
	/// The boot stubs.
	stubs: [u8; 0x800],
	/// The GDTR.
	gdtr: [u8; 6],
	/// The LAPIC ID.
	lapic_id: u8,
	/// The primary flag.
	primary_flag: AtomicU64,
	/// The secondary flag.
	secondary_flag: AtomicU64,
	/// The actual stack pointer.
	///
	/// This is the stack pointer that the long mode stub will switch to,
	/// and that serves as the final kernel stack pointer.
	actual_stack_ptr: u64,
	/// The actual CR3 value.
	actual_cr3_ptr: u64,
	/// The actual CR4 value.
	actual_cr4_value: u64,
	/// The actual CR0 value.
	actual_cr0_value: u64,
	/// The entry point.
	entry_point: u64,
	/// The null IDT.
	null_idt: [u8; 6],
	/// The CR4 bits.
	cr4_bits: u32,
	/// The timekeeper to use.
	timekeeper: ManuallyDrop<Arc<dyn GetInstant>>,
	/// The GDT bytes.
	gdt: [u8; 0x100],
}

const _: () = {
	// Ensure the boot meta structure is the correct size.
	assert::fits::<BootMeta, 4096>();
};

/// Boots a secondary core with the given local APIC and LAPIC ID.
///
/// # Panics
/// Panics if the boot stubs are too large to fit in the first half of the page.
/// This should never happen outside of development of the stubs themselves (and
/// moreso, would be relatively rare anyway).
///
/// # Safety
/// Uses the page at physical address 0x8000 as the secondary core's entry point,
/// and the page at physical address 0x9000 as the secondary core's L4 page table.
///
/// Caller must ensure these pages are mapped and accessible.
pub unsafe fn boot(
	primary_handle: &AddressSpaceHandle,
	lapic: &Lapic,
	secondary_lapic_id: u8,
	stack_pages: usize,
	timekeeper: Arc<dyn GetInstant>,
) -> Result<(), BootError> {
	let stubs = Phys::from_address_unchecked(0x8000)
		.as_ref::<UnsafeCell<BootMeta>>()
		.unwrap();

	// Make sure the stubs fit in the first half of the page...
	assert!(SECONDARY_BOOT_STUB.len() <= 0x400);
	assert!(SECONDARY_BOOT_LONG_MODE_STUB.len() <= 0x400);

	// Create a new supervisor address space based on the current address space.
	let mapper = AddressSpaceLayout::duplicate_supervisor_space_shallow(primary_handle)
		.ok_or(BootError::OutOfMemory)?;

	// Direct-map the code segment into the secondary core's address space.
	// This allows the code to still execute after switching to paging mode.
	AddressSpaceLayout::secondary_boot_stub_code()
		.map(&mapper, 0x8000, 0x8000)
		.map_err(BootError::MapError)?;

	// Create a stack and map it into the secondary core's address space.
	let kernel_stack_segment = AddressSpaceLayout::kernel_stack();
	let last_stack_page_virt = kernel_stack_segment.range().1 & !0xFFF;

	// "Forget" the entire segment (meaning simply to unmap but not
	// reclaim any mappings). We don't reclaim anything since the only
	// allocation that `duplicate_supervisor_space_shallow` makes
	// is for the very, very top level page table. All of the L4/5 entries
	// still point to the primary core's page tables, so resetting them
	// before we remap them is sufficient enough.
	kernel_stack_segment.unmap_all_without_reclaim(&mapper);

	// make sure top guard page is unmapped
	match kernel_stack_segment.unmap(&mapper, last_stack_page_virt) {
		// NOTE(qix-): The Ok() case would never hit here since we explicitly unmapped the entire segment.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => return Err(BootError::UnmapError(e)),
	}

	let mut bottom_stack_page_virt = last_stack_page_virt;
	for _ in 0..stack_pages {
		bottom_stack_page_virt -= 4096;

		let stack_phys = GlobalPfa
			.allocate()
			.ok_or(BootError::MapError(MapError::OutOfMemory))?;

		// We map it into two places; the _real_ location in higher half
		// memory where the stack will ultimately reside, as well as
		// the first page of the stack in the lower half of memory
		// since we don't have an RSP to set up but rather have to
		// rely on ESP (a 32-bit register) until the long mode trampoline
		// is hit.
		kernel_stack_segment
			.map(&mapper, bottom_stack_page_virt, stack_phys)
			.map_err(BootError::MapError)?;
	}

	// Make sure that the bottom guard page is unmapped
	match kernel_stack_segment.unmap(&mapper, bottom_stack_page_virt - 4096) {
		// NOTE(qix-): The Ok() case would never hit here isnce we explicitly unmapped the entire segment.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => return Err(BootError::UnmapError(e)),
	}

	// Now copy the entire top-level page table into 0x9000.
	let l4_table = mapper.base_phys().as_ptr_unchecked::<u8>();
	let secondary_l4_table = Phys::from_address_unchecked(0x9000).as_mut_ptr_unchecked::<u8>();
	secondary_l4_table.copy_from(l4_table, 4096);

	// Write the stubs into the first half of the page.
	// They live at 0x8000 (CS:IP = 0x0800:0x0000) and
	// 0x8000 + 0x400 (CS:IP = 0x0800:0x0400) for the
	// 16-bit and 64-bit stubs, respectively.
	let stub_slice = &mut (*stubs.get()).stubs[0..SECONDARY_BOOT_STUB.len()];
	stub_slice.copy_from_slice(&SECONDARY_BOOT_STUB);

	let long_mode_stub_slice =
		&mut (*stubs.get()).stubs[0x400..0x400 + SECONDARY_BOOT_LONG_MODE_STUB.len()];
	long_mode_stub_slice.copy_from_slice(&SECONDARY_BOOT_LONG_MODE_STUB);

	// The GDT is copied as-is, just placed in a well-known location.
	let gdt_slice = crate::gdt::GDT.as_bytes();
	// TODO(qix-): Somehow make this a compile-time assertion.
	debug_assert!(
		gdt_slice.len() <= (*stubs.get()).gdt.len(),
		"GDT is too large for the secondary boot stub page; increase the size of the stub page"
	);
	(*stubs.get()).gdt[..gdt_slice.len()].copy_from_slice(gdt_slice);

	// Write the LAPIC ID.
	(*stubs.get()).lapic_id = secondary_lapic_id;

	// Zero the primary flag.
	(*stubs.get()).primary_flag = AtomicU64::new(0);

	// Zero the secondary flag.
	(*stubs.get()).secondary_flag = AtomicU64::new(0);

	// Write the absolute entry point address of the Rust stub.
	let entry_point_ptr = oro_kernel_x86_64_rust_secondary_core_entry as *const u8 as u64;
	(*stubs.get()).entry_point = entry_point_ptr;

	// Write the actual CR0 value so that the long mode stub can install it.
	let cr0_value: u64 = crate::reg::Cr0::load().into();
	(*stubs.get()).actual_cr0_value = cr0_value;

	// Write the actual CR4 value so that the long mode stub can install it.
	let cr4_value: u64 = crate::reg::Cr4::load().into();
	(*stubs.get()).actual_cr4_value = cr4_value;

	// Write the actual CR3 value so that the long mode stub can switch to it.
	let cr3_phys = mapper.base_phys();
	(*stubs.get()).actual_cr3_ptr = cr3_phys.address_u64();

	// Write the real stack pointer so that the long mode stub can switch to it.
	(*stubs.get()).actual_stack_ptr = last_stack_page_virt as u64;

	// Zero out the null IDT.
	(*stubs.get()).null_idt = [0; 6];

	// Extract out the interesting bits of CR4 for the secondary core,
	// without enabling anything that might screw up 16-bit initialization.
	// We treat CR4 here as a 32-bit since this field is being accessed
	// by the 32-bit stub.
	let cr4_bits: u64 = crate::reg::Cr4::load().with_pge(false).into();
	(*stubs.get()).cr4_bits = cr4_bits as u32;

	// Write the GDT pointer into the last 6 bytes of the page.
	let gdt_base: u32 = 0x8000 + offset_of!(BootMeta, gdt) as u32;
	let gdtr_descriptor_slice = &mut (*stubs.get()).gdtr;
	gdtr_descriptor_slice[0..2].copy_from_slice(&(gdt_slice.len() as u16 - 1).to_le_bytes());
	gdtr_descriptor_slice[2..6].copy_from_slice(&gdt_base.to_le_bytes());

	// Write the timekeeper implementation so the secondary cores can use it.
	(*stubs.get()).timekeeper = ManuallyDrop::new(timekeeper);

	// Make sure all cores see the change.
	crate::asm::strong_memory_fence();

	// Finally, tell the processor to start executing at page 8 (0x8000).
	// NOTE(qix-): Specifying other pages doesn't seem to work. The documentation
	// NOTE(qix-): surrounding the LAPIC SIPI interrupts are full of holes and
	// NOTE(qix-): 404's from 20+ years ago. If you have any more information on
	// NOTE(qix-): how to make this work a bit cleaner (e.g. not requiring 0x8000
	// NOTE(qix-): hard-coded and instead allowing us to take any page < 256),
	// NOTE(qix-): I'd love to hear all about it.
	lapic.boot_core(secondary_lapic_id, 8);

	// Tell the secondary core we're ready to go.
	(*stubs.get()).primary_flag.store(1, Release);

	// Wait for the secondary core to signal it's ready.
	let mut ok = false;
	for _ in 0..10_000_000 {
		match (*stubs.get()).secondary_flag.load(Acquire) {
			1 => {
				ok = true;
				break;
			}
			0 => ::core::hint::spin_loop(),
			err => {
				// Tell the secondary we no longer want it to boot.
				// Just as a precaution since it's already in an error state.
				(*stubs.get())
					.primary_flag
					.store(0xFFFF_FFFF_FFFF_FFFE, Release);
				return Err(BootError::SecondaryError(err));
			}
		}
	}

	if !ok {
		// Tell the secondary we no longer want it to boot.
		(*stubs.get())
			.primary_flag
			.store(0xFFFF_FFFF_FFFF_FFFE, Release);
		return Err(BootError::SecondaryTimeout);
	}

	Ok(())
}

asm_buffer! {
	/// The secondary boot stub machine code.
	///
	/// This is more or less adapted from the direct-to-long-mode boot stub
	/// provided by Brendan from the `OSDev` Wiki. It's not supposed to work
	/// as per the AMD documentation, but it does.
	static SECONDARY_BOOT_STUB: AsmBuffer = {
		{
			// 16-bit code starts here
			".code16",

			// Disable interrupts and clear direction flag.
			"cli",
			"cld",

			// Mask off all IRQs with the LAPIC.
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
			"lidt [{LIDT_ADDR}]",

			// Set the PAE and PGE bits in CR4,
			// as well as any others that the primary
			// core has set that we're interested in.
			"mov eax, 0b10100000",
			"mov ebx, [{CR4_BITS}]",
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
			"lgdt [{GDTR}]",

			// We can now jump to the long mode stub.
			"ljmp 0x0008, 0x8400",
		},
		{
			LIDT_ADDR = const 0x8000 + offset_of!(BootMeta, null_idt),
			CR4_BITS = const 0x8000 + offset_of!(BootMeta, cr4_bits),
			GDTR = const 0x8000 + offset_of!(BootMeta, gdtr),
		},
	};
}

asm_buffer! {
	/// The secondary boot stub machine code for long mode.
	///
	/// This is jumped to by the 16-bit stub after setting up the
	/// interim long mode environment, necessary to switch the
	/// code segment selector via an `ljmp` instruction.
	static SECONDARY_BOOT_LONG_MODE_STUB: AsmBuffer = {
		{
			// 64-bit code starts here
			".code64",

			// Set the real stack pointer.
			"mov rax, [{STACK_PTR}]",
			"mov rsp, rax",

			// Set the real CR3 value.
			"mov rax, [{CR3}]",
			"mov cr3, rax",

			// Install the real CR0 value.
			"mov rax, [{CR0}]",
			"mov cr0, rax",

			// Install the real CR4 value.
			"mov rax, [{CR4}]",
			"mov cr4, rax",

			// Jump to the Rust kernel secondary core entry point.
			"push 0",
			"mov rax, [{ENTRY_POINT}]",
			"jmp rax",
		},
		{
			STACK_PTR = const 0x8000 + offset_of!(BootMeta, actual_stack_ptr),
			CR3 = const 0x8000 + offset_of!(BootMeta, actual_cr3_ptr),
			CR0 = const 0x8000 + offset_of!(BootMeta, actual_cr0_value),
			CR4 = const 0x8000 + offset_of!(BootMeta, actual_cr4_value),
			ENTRY_POINT = const 0x8000 + offset_of!(BootMeta, entry_point),
		},
	};
}

/// The Rust entry point for secondary cores. This is jumped to
/// by the long mode stub after setting up most of the rest of
/// the *actual* long mode environment.
#[unsafe(no_mangle)]
unsafe extern "C" fn oro_kernel_x86_64_rust_secondary_core_entry() -> ! {
	crate::asm::flush_tlb();
	crate::gdt::GDT.install();
	crate::interrupt::install_default();

	let stubs = Phys::from_address_unchecked(0x8000)
		.as_ref::<UnsafeCell<BootMeta>>()
		.unwrap();

	// Take the timekeeper, not dropping it later on.
	let timekeeper = ManuallyDrop::take(&mut (*stubs.get()).timekeeper);

	// Pull the RSDP from the boot protocol
	// SAFETY(qix-): We can just unwrap these values as they're guaranteed to be OK
	// SAFETY(qix-): since the primary core has already validated them to even boot
	// SAFETY(qix-): the secondaries.
	let AcpiKind::V0(acpi) = super::protocol::ACPI_REQUEST.response().unwrap() else {
		unreachable!();
	};
	let Some(sdt) = Rsdp::get(core::ptr::read_volatile(&acpi.assume_init_ref().rsdp))
		.as_ref()
		.and_then(Rsdp::sdt)
	else {
		// Tell the primary we failed.
		dbg_err!("failed to get RSDT from ACPI tables");
		(*stubs.get())
			.secondary_flag
			.store(0xFFFF_FFFF_FFFF_FFFE, Release);
		crate::asm::hang();
	};

	let Some(lapic) = sdt
		.find::<Madt>()
		.as_ref()
		.map(Madt::lapic_phys)
		.map(|phys| Phys::from_address_unchecked(phys).as_mut_ptr_unchecked::<u8>())
		.map(|lapic_virt| Lapic::new(lapic_virt))
	else {
		// Tell the primary we failed.
		dbg_err!("failed to get LAPIC from ACPI tables");
		(*stubs.get())
			.secondary_flag
			.store(0xFFFF_FFFF_FFFF_FFFE, Release);
		crate::asm::hang();
	};

	dbg!("local APIC version: {:?}", lapic.version());

	// Make sure the LAPIC IDs match.
	let lapic_id = lapic.id();
	let given_lapic_id = (*stubs.get()).lapic_id;

	if lapic_id != given_lapic_id {
		// Tell the primary we failed.
		dbg_err!("LAPIC ID mismatch: expected {given_lapic_id}, got {lapic_id}");
		(*stubs.get())
			.secondary_flag
			.store(0xFFFF_FFFF_FFFF_FFFE, Release);
		crate::asm::hang();
	}

	dbg!("secondary core booted with LAPIC ID {}", lapic_id);

	// Wait for the primary to tell us to continue.
	let mut ok = false;
	for _ in 0..100_000 {
		match (*stubs.get()).primary_flag.load(Acquire) {
			1 => {
				// Tell the primary we're ready to go.
				// SAFETY(qix-): Once we've written this value, the boot stub pages are no longer
				// SAFETY(qix-): safe to write to.
				(*stubs.get()).secondary_flag.store(1, Release);
				ok = true;
				break;
			}
			0 => ::core::hint::spin_loop(),
			_ => {
				break;
			}
		}
	}

	if !ok {
		dbg_err!(
			"secondary core timed out waiting for primary to signal to continue, or primary told \
			 us to stop"
		);

		(*stubs.get())
			.secondary_flag
			.store(0xFFFF_FFFF_FFFF_FFFE, Release);

		crate::asm::hang();
	}

	// We've been given the green light.
	// Unmap the secondary boot stub code and stack.
	// We don't reclaim the pages since they're "static" and will
	// be shared again for any other secondary cores. We simply 'forget'
	// them.
	let mapper = AddressSpaceLayout::current_supervisor_space();
	// SAFETY(qix-): We're sure that unmapping without reclaiming won't lead to a memory leak.
	unsafe {
		AddressSpaceLayout::secondary_boot_stub_code().unmap_all_without_reclaim(&mapper);
		AddressSpaceLayout::secondary_boot_stub_stack().unmap_all_without_reclaim(&mapper);
	}

	// Wait for the primary to tell us to continue.
	for _ in 0..10_000_000_000_usize {
		if SECONDARIES_MAY_BOOT.load(Relaxed) {
			break;
		}
		::core::hint::spin_loop();
	}

	assert!(
		SECONDARIES_MAY_BOOT.load(Relaxed),
		"secondary core booted but primary didn't signal to continue in a timely fashion"
	);

	super::initialize_core_local(lapic, timekeeper);
	super::finalize_boot_and_run();
}
