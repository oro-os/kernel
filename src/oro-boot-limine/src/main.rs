#![no_std]
#![no_main]
#![feature(naked_functions, core_intrinsics, more_qualified_paths)]

use core::{arch::asm, ffi::CStr};
use elf::{endian::AnyEndian, ElfBytes, ParseError};
use lazy_static::lazy_static;
#[cfg(debug_assertions)]
use limine_protocol::StackSizeRequest;
use limine_protocol::{
	structures::memory_map_entry::{EntryType, MemoryMapEntry},
	BootTimeRequest, HHDMRequest, MemoryMapRequest, ModuleRequest, Request,
};
use oro_boot::{
	x86_64::{
		l4_to_range_48, BootConfig, MemoryRegion, MemoryRegionKind, KERNEL_STACK_PAGE_TABLE_INDEX,
		ORO_BOOT_PAGE_TABLE_INDEX, RECURSIVE_PAGE_TABLE_INDEX,
	},
	Allocator, Serialize, BOOT_MAGIC,
};
use spin::Mutex;
use uart_16550::SerialPort;
use x86_64::{
	addr::{PhysAddr, VirtAddr},
	structures::paging::{
		frame::PhysFrame,
		mapper::{MapToError, OffsetPageTable},
		page::{Page, PageSize, Size4KiB},
		page_table::{PageTable, PageTableFlags},
		FrameAllocator, Mapper, Translate,
	},
};

lazy_static! {
	#[cfg(target_arch = "x86_64")]
	static ref SERIAL: Mutex<SerialPort> = {
		let mut serial_port = unsafe { SerialPort::new(0x3F8) };
		serial_port.init();
		Mutex::new(serial_port)
	};
}

/// We can put it here since all memory in the lower half are
/// unmapped in the kernel upon boot, and all pages are reclaimed.
const STUBS_ADDR: u64 = 0x0000_6000_0000_0000;

// These are linked via the linker script colocated
// with this crate. The u8 is just a dummy type;
// their _addresses_ are the interesting bits, not
// the contents, which have undefined values and
// accessing them may cause faults.
extern "C" {
	static _ORO_STUBS_START: u8;
	static _ORO_STUBS_END: u8;
}

#[used]
static HHDM_REQUEST: Request<HHDMRequest> = HHDMRequest {
	revision: 0,
	..HHDMRequest::new()
}
.into();

#[used]
static MMAP_REQUEST: Request<MemoryMapRequest> = MemoryMapRequest {
	revision: 0,
	..MemoryMapRequest::new()
}
.into();

#[used]
static MOD_REQUEST: Request<ModuleRequest> = ModuleRequest {
	revision: 0,
	..ModuleRequest::new()
}
.into();

#[used]
static TIME_REQUEST: Request<BootTimeRequest> = BootTimeRequest {
	revision: 0,
	..BootTimeRequest::new()
}
.into();

#[cfg(debug_assertions)]
#[used]
static STKSZ_REQUEST: Request<StackSizeRequest> = StackSizeRequest {
	revision: 0,
	stack_size: 64 * 1024 * 1024,
	..StackSizeRequest::new()
}
.into();

fn map_limine_to_oro_region(kind: &EntryType) -> MemoryRegionKind {
	match kind {
		EntryType::Usable => MemoryRegionKind::Usable,
		EntryType::KernelAndModules => MemoryRegionKind::Modules,
		EntryType::BootloaderReclaimable => MemoryRegionKind::Usable,
		_ => MemoryRegionKind::Reserved,
	}
}

fn is_oro_region_allocatable(kind: &MemoryRegionKind) -> bool {
	kind == &MemoryRegionKind::Usable
}

struct LiminePageFrameAllocator {
	bios_mapping: &'static [&'static MemoryMapEntry],
	bios_mapping_offset: usize,
	byte_offset: u64,
	byte_offset_max: u64,
	total_allocations: u64,
}

impl LiminePageFrameAllocator {
	fn new(bios_mapping: &'static [&'static MemoryMapEntry]) -> Self {
		// get byte offset of first mapping (doesn't need to be usable, just the valid base offset)
		let (byte_offset, byte_offset_max) = if bios_mapping.is_empty() {
			(0, 0)
		} else {
			(
				bios_mapping[0].base,
				bios_mapping[0].base + bios_mapping[0].length,
			)
		};

		Self {
			bios_mapping,
			bios_mapping_offset: 0,
			byte_offset,
			byte_offset_max,
			total_allocations: 0,
		}
	}
}

unsafe impl FrameAllocator<Size4KiB> for LiminePageFrameAllocator {
	fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
		'advance_mapping: {
			if self.byte_offset >= self.byte_offset_max {
				for i in (self.bios_mapping_offset + 1)..self.bios_mapping.len() {
					let mapping = &self.bios_mapping[i];

					// As per Limine spec, base addresses and sizes are
					// guaranteed to be non-overlapping and 4KiB aligned.
					// Thus, we can make a _lot_ of simplifications to
					// the byte math here.
					if is_oro_region_allocatable(&map_limine_to_oro_region(&mapping.kind)) {
						self.bios_mapping_offset = i;
						self.byte_offset = mapping.base;
						self.byte_offset_max = self.byte_offset + mapping.length;
						break 'advance_mapping;
					}
				}

				self.bios_mapping_offset = self.bios_mapping.len();

				return None;
			}
		}

		// It's that simple, because limine guarantees a few things:
		// 1. Bases and lengths of usable memory are always 4KiB aligned
		// 2. There's always at least one page in each memory entry
		//
		// Number 2 is not explicitly listed in the spec so we're
		// making an educated assumption; the authors of Limine
		// has chosen not to specify it but we're going to simplify
		// this by assuming it won't receive a zero-length entry
		// based on Discord conversations.
		let offset = self.byte_offset;
		self.byte_offset += Size4KiB::SIZE;
		self.total_allocations += 1;
		Some(unsafe { PhysFrame::from_start_address_unchecked(PhysAddr::new_unsafe(offset)) })
	}
}

trait DebugPrint {
	fn dbgprint(self);
}

impl DebugPrint for &str {
	fn dbgprint(self) {
		use core::fmt::Write;
		let _ = SERIAL.lock().write_str(self);
	}
}

impl DebugPrint for u64 {
	fn dbgprint(self) {
		for i in 0..16 {
			let shift = 4 * (15 - i);
			let shifted = self >> shift;
			let char_offset = (shifted & 0xF) as usize;
			const CHARS: &str = "0123456789ABCDEF";
			let s: &str = &CHARS[char_offset..char_offset + 1];
			s.dbgprint();
		}
	}
}

impl DebugPrint for &core::ffi::CStr {
	fn dbgprint(self) {
		let mut sp = SERIAL.lock();
		for b in self.to_bytes_with_nul() {
			if *b == 0 {
				break;
			}
			sp.send(*b);
		}
	}
}

macro_rules! dbg {
	($($e:expr),*) => {
		$($e.dbgprint();)*
		"\n".dbgprint();
	}
}

#[cfg(target_arch = "x86_64")]
unsafe fn halt() -> ! {
	asm!("cli");
	loop {
		asm!("hlt");
	}
}

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	dbg!("boot error: kernel panic");
	halt()
}

trait IntoPage {
	fn into_page(self) -> Page;
}

impl IntoPage for Page {
	fn into_page(self) -> Page {
		self
	}
}

impl IntoPage for VirtAddr {
	fn into_page(self) -> Page {
		unsafe { Page::from_start_address_unchecked(self) }
	}
}

impl IntoPage for u64 {
	fn into_page(self) -> Page {
		unsafe { Page::from_start_address_unchecked(VirtAddr::new_unsafe(self)) }
	}
}

trait IntoFrame {
	fn into_frame(self) -> PhysFrame;
}

impl IntoFrame for PhysFrame {
	fn into_frame(self) -> PhysFrame {
		self
	}
}

impl IntoFrame for PhysAddr {
	fn into_frame(self) -> PhysFrame {
		unsafe { PhysFrame::from_start_address_unchecked(self) }
	}
}

impl IntoFrame for u64 {
	fn into_frame(self) -> PhysFrame {
		unsafe { PhysFrame::from_start_address_unchecked(PhysAddr::new_unsafe(self)) }
	}
}

trait MapOrDie {
	unsafe fn map_or_die<P: IntoPage, F: IntoFrame, A: FrameAllocator<Size4KiB>>(
		&mut self,
		page: P,
		frame: F,
		flags: PageTableFlags,
		allocator: &mut A,
	);
}

impl<T> MapOrDie for T
where
	T: Mapper<Size4KiB>,
{
	unsafe fn map_or_die<P: IntoPage, F: IntoFrame, A: FrameAllocator<Size4KiB>>(
		&mut self,
		page: P,
		frame: F,
		flags: PageTableFlags,
		allocator: &mut A,
	) {
		// Note that bit 9 indicates to the kernel that these pages can be re-claimed if need be.
		// We mark any allocated physical frames used for page tables as such.
		match self.map_to_with_table_flags(
			page.into_page(),
			frame.into_frame(),
			flags,
			PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::BIT_9,
			allocator,
		) {
			Ok(flusher) => flusher.flush(),
			Err(err) => {
				dbg!(
					"boot error: failed to map memory: ",
					match err {
						MapToError::FrameAllocationFailed => "frame allocation failed",
						MapToError::ParentEntryHugePage => "parent entry is a huge page",
						MapToError::PageAlreadyMapped(_) => "page already mapped",
					}
				);
				halt();
			}
		}
	}
}

struct OroBootAllocator<'a, 'b, A>
where
	A: FrameAllocator<Size4KiB> + 'static,
{
	current_position: u64,
	last_allocated_page: u64,
	pfa: &'a mut A,
	rw_mapper: &'a mut OffsetPageTable<'b>,
	ro_mapper: &'a mut OffsetPageTable<'b>,
}

impl<'a, 'b, A> OroBootAllocator<'a, 'b, A>
where
	A: FrameAllocator<Size4KiB>,
{
	fn new(
		pfa: &'a mut A,
		rw_mapper: &'a mut OffsetPageTable<'b>,
		ro_mapper: &'a mut OffsetPageTable<'b>,
	) -> Self {
		Self {
			current_position: l4_to_range_48(ORO_BOOT_PAGE_TABLE_INDEX).0,
			last_allocated_page: 0,
			pfa,
			rw_mapper,
			ro_mapper,
		}
	}
}

unsafe impl<'a, 'b, A> Allocator for OroBootAllocator<'a, 'b, A>
where
	A: FrameAllocator<Size4KiB>,
{
	#[inline(always)]
	fn position(&self) -> u64 {
		self.current_position
	}

	// TODO: Woof, this is a a really poorly written implementation.
	// TODO: Might want to re-write this later...
	unsafe fn allocate(&mut self, sz: u64) {
		let current_page = self.current_position >> 12;
		let last_page = (self.current_position + sz) >> 12;

		for page in current_page..=last_page {
			if self.last_allocated_page >= page {
				continue;
			}

			let frame = self.pfa.allocate_frame().unwrap_or_else(|| {
				dbg!("boot error: out of memory when allocating boot protocol structures");
				halt();
			});

			self.rw_mapper.map_or_die(
				page << 12,
				frame,
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
				self.pfa,
			);

			self.ro_mapper
				.map_or_die(page << 12, frame, PageTableFlags::PRESENT, self.pfa);
		}

		self.current_position += sz;
		self.last_allocated_page = last_page;
	}
}

/// A macro simply because we need $src_mapper and $mapper
/// to be the same in the Limine page table case and you can't
/// take a mutable reference AND an immutable reference out at
/// the same time.
macro_rules! map_stubs {
	($src_mapper: expr, $mapper: expr, $pfa: expr) => {
		let start_page = (&_ORO_STUBS_START as *const u8 as u64) >> 12;
		let end_page = (&_ORO_STUBS_END as *const u8 as u64) >> 12;

		if (end_page - start_page) > 512 {
			dbg!("boot error: stubs take up too much memory (more than 512 pages)");
			halt();
		}

		for page in start_page..end_page {
			$mapper.map_or_die(
				STUBS_ADDR + ((page - start_page) << 12),
				match $src_mapper.translate_addr(VirtAddr::new_unsafe(page << 12)) {
					Some(addr) => addr,
					None => {
						dbg!("boot error: stubs were not mapped in correctly");
						halt();
					}
				},
				PageTableFlags::PRESENT | PageTableFlags::NO_CACHE,
				&mut $pfa,
			);
		}
	};
}

/// This is necessary due to the fact that CStr slices
/// don't encode their length, thus iterations will
/// skip right past the null byte and trail off into
/// oblivion.
fn cstr_eq(cstr: &CStr, bytestring: &[u8]) -> bool {
	let cstrb = cstr.to_bytes_with_nul();
	let mut i = 0;
	loop {
		let bsc = bytestring[i];
		if cstrb[i] != bsc {
			return false;
		}
		if bsc == 0 {
			return true;
		}
		i += 1;
	}
}

/// # Safety
/// Do not call directly; use load_elf instead.
unsafe fn load_kernel_elf_err<M: Mapper<Size4KiB>, A: FrameAllocator<Size4KiB>>(
	elf_bytes: &[u8],
	phys_offset: u64,
	mapper: &mut M,
	pfa: &mut A,
) -> Result<u64, ParseError> {
	// Parse just what's necessary
	let elf = ElfBytes::<AnyEndian>::minimal_parse(elf_bytes)?;

	// Base address of module; Limine guarantees that this is
	// 4KiB page aligned.
	let root_addr: u64 = &elf_bytes[0] as *const u8 as u64;

	// Load all segments
	if let Some(segments) = elf.segments() {
		for phdr in segments {
			if phdr.p_type != elf::abi::PT_LOAD {
				continue;
			}

			// TODO make sure all things are page aligned (they're supposed to be
			// TODO in the oro kernel linker script but we should double check);
			// TODO also check the p_align field to make sure it's 0x1000 (also specified)

			let src_virt_addr = root_addr + phdr.p_offset;

			let dest_start_page = phdr.p_vaddr >> 12;
			let dest_end_page = dest_start_page + (phdr.p_memsz >> 12);

			let pflags = {
				let mut pflags = PageTableFlags::PRESENT;

				if (phdr.p_flags & elf::abi::PF_W) != 0 {
					pflags |= PageTableFlags::WRITABLE;
				}
				if (phdr.p_flags & elf::abi::PF_X) == 0 {
					pflags |= PageTableFlags::NO_EXECUTE;
				}

				pflags
			};

			for dest_page in dest_start_page..=dest_end_page {
				let dest_virt_addr = dest_page << 12;
				let dest_phys_frame = pfa.allocate_frame().unwrap_or_else(|| {
					dbg!("boot error: failed to allocate oro kernel segment: out of memory");
					halt();
				});

				mapper.map_or_die(dest_virt_addr, dest_phys_frame, pflags, pfa);

				// The current offset within the segment itself (not the offset in the ELF file)
				let start_segment_offset = (dest_page - dest_start_page) << 12;

				let end_segment_offset = start_segment_offset + 0x1000;
				let end_segment_offset = core::cmp::min(end_segment_offset, phdr.p_filesz);
				let total_copy_bytes = end_segment_offset.saturating_sub(start_segment_offset);
				let total_zero_bytes = 0x1000 - total_copy_bytes;

				let src_write_base = src_virt_addr + start_segment_offset;
				// The "dst" here meaning "in the current memory map" since we're actually
				// setting up the _eventual_ memory map for the Oro kernel which is not yet
				// loaded into CR3. Thus, we use the direct map to write to the physical
				// frame directly.
				let dst_write_base = phys_offset + dest_phys_frame.start_address().as_u64();

				if total_copy_bytes > 0 {
					core::intrinsics::volatile_copy_nonoverlapping_memory(
						dst_write_base as *mut u8,
						src_write_base as *const u8,
						total_copy_bytes as usize,
					);
				}

				if total_zero_bytes > 0 {
					core::intrinsics::volatile_set_memory(
						(dst_write_base + total_copy_bytes) as *mut u8,
						0u8,
						total_zero_bytes as usize,
					);
				}
			}
		}
	} else {
		dbg!("boot error: oro kernel has no loadable segments");
		halt();
	}

	// Return the entry point
	Ok(elf.ehdr.e_entry)
}

/// # Safety
/// Among other things, this is the most naive, trusting implementation
/// of an ELF loader probably to ever exist. It *does not* perform
/// any checks. It gives free reign to the Oro Kernel to set itself up
/// however it wants, including attempting to overwrite other things
/// we've done here (which will ultimately fail with an error, of course).
///
/// Do NOT use this in the actual kernel code. Please use a more fleshed out
/// ELF loader.
unsafe fn load_kernel_elf<M: Mapper<Size4KiB>, A: FrameAllocator<Size4KiB>>(
	elf_bytes: &[u8],
	phys_offset: u64,
	mapper: &mut M,
	pfa: &mut A,
) -> u64 {
	match load_kernel_elf_err(elf_bytes, phys_offset, mapper, pfa) {
		Ok(r) => r,
		Err(_err) => {
			// TODO better error messages
			dbg!("boot error: failed to load oro kernel");
			halt();
		}
	}
}

/// # Safety
/// Do not call directly; only meant to be called by the Limine bootloader!
#[cfg(target_arch = "x86_64")]
#[inline(never)]
#[no_mangle]
pub unsafe fn _start() -> ! {
	x86_64::instructions::interrupts::disable();

	dbg!("starting Oro + limine pre-boot");

	let hhdm = if let Some(res) = HHDM_REQUEST.get_response() {
		res
	} else {
		dbg!("boot error: missing limine hhdm response");
		halt();
	};

	let mmap = if let Some(res) = MMAP_REQUEST.get_response() {
		if let Some(entries) = res.get_memory_map() {
			entries
		} else {
			dbg!("boot error: missing limine mmap slice (response ok)");
			halt();
		}
	} else {
		dbg!("boot error: missing limine mmap response");
		halt();
	};

	let mods = if let Some(res) = MOD_REQUEST.get_response() {
		if res.modules.is_null() {
			dbg!("boot error: no boot modules found (null array in response)");
			halt();
		} else {
			core::slice::from_raw_parts(*(res.modules), res.module_count as usize)
		}
	} else {
		dbg!("boot error: missing limine modules response (or no modules specified)");
		halt();
	};

	let boot_time = if let Some(res) = TIME_REQUEST.get_response() {
		res.boot_time
	} else {
		dbg!("boot error: missing limine boot time response");
		halt();
	};

	#[cfg(debug_assertions)]
	if STKSZ_REQUEST.get_response().is_none() {
		dbg!("!!WARNING!! Oro + limine boot stage built in debug mode, which");
		dbg!("!!WARNING!! means we request a much, much larger stack size to");
		dbg!("!!WARNING!! accommodate Rust's large debug sizes, namely around");
		dbg!("!!WARNING!! parsing the kernel ELF module. However, Limine has");
		dbg!("!!WARNING!! not honored the stack size adjustment request, which");
		dbg!("!!WARNING!! means some crazy stuff is probably about to happen,");
		dbg!("!!WARNING!! the best case being a reboot or stall (triple-fault).");
	}

	// The limine page frame allocator is a simple, temporary page frame allocator
	// that uses the memory map Limine gives us directly, used to allocate the Oro
	// boot protocol structures as well as the physical frames for the boot stubs.
	// The frames that get used here, sans boot stub page table frames, are marked
	// as "Oro boot protocol reclaimable" frames that the OS is free to reclaim if
	// it can.
	let mut pfa = LiminePageFrameAllocator::new(mmap);

	// Make the OS's root page table.
	let (oro_l4_page_table, oro_l4_page_table_phys_addr): (&mut PageTable, u64) = unsafe {
		let phys_addr = match pfa.allocate_frame() {
			Some(frame) => frame.start_address().as_u64(),
			None => {
				dbg!("boot error: cannot allocate Oro L4 page table; out of memory");
				halt();
			}
		};

		let mapped_addr = phys_addr + hhdm.offset as u64;
		let pt = &mut *(mapped_addr as *mut PageTable);
		pt.zero();
		(pt, phys_addr)
	};

	// Make the resulting page table recursive. We can't use a recursive page
	// table _now_ because the page table must be loaded in CR3 in order to
	// use the x86_64 crate's RecursivePageTable structure.
	//
	// Must be done before we pass the mutable reference to the mapper.
	oro_l4_page_table[RECURSIVE_PAGE_TABLE_INDEX as usize].set_addr(
		PhysAddr::new_unsafe(oro_l4_page_table_phys_addr),
		PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
	);

	// Use it to make an offset page table, since our memory
	// is direct-mapped at the moment.
	let mut oro_mapper =
		OffsetPageTable::new(oro_l4_page_table, VirtAddr::new_unsafe(hhdm.offset as u64));

	// Set up the stack space. We'll give it 64KiB to start with (the kernel
	// may choose to grow the stack, as per oro_boot's specification of the
	// kernel stack index value).
	let (_, stack_last_addr) = l4_to_range_48(KERNEL_STACK_PAGE_TABLE_INDEX);
	let stack_init_addr = stack_last_addr & (!0xFFF); // used by the stubs to set the kernel stack
	let stack_page_end = stack_last_addr >> 12;
	let stack_page_start = stack_page_end - 16; // 16 * 4KiB = 64KiB initial kernel stack size

	// Note that the .. is deliberate (over ..=); we do NOT want to use
	// the last stack page, but instead keep it unused as a guard page.
	for stack_page in stack_page_start..stack_page_end {
		// Allocate and map in the stack
		let stack_frame = match pfa.allocate_frame() {
			Some(frame) => frame,
			None => {
				dbg!("boot error: failed to allocate kernel stack; out of memory");
				halt();
			}
		};

		oro_mapper.map_or_die(
			stack_page << 12,
			stack_frame,
			PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
			&mut pfa,
		);
	}

	// Now, attempt to find the kernel "module" and load it.
	let kernel_entry_point: u64;
	'load_kernel: {
		for module in mods {
			if cstr_eq(module.path, b"/oro-kernel\0") {
				kernel_entry_point = load_kernel_elf(
					core::slice::from_raw_parts(module.address, module.size as usize),
					hhdm.offset as u64,
					&mut oro_mapper,
					&mut pfa,
				);
				break 'load_kernel;
			} else {
				dbg!("warning: unused module (unrecognized path): ", module.path);
			}
		}

		dbg!("boot error: /oro-kernel module not found on boot medium");
		halt();
	}

	if kernel_entry_point == 0 {
		// Should never happen but good to check just to
		// safeguard against bugs above.
		dbg!("boot error: oro kernel entry point is null");
		halt();
	}

	// Get the current page tables and create an offset mapper;
	// this is normally not great to do given the Limine page tables
	// but since it's done last, directly before Limine is torn down,
	// we don't care if something becomes mangled in the process.
	// We need to do this to map the execution control switch stubs
	// into a Well Known place.
	let (limine_page_table_frame, _) = x86_64::registers::control::Cr3::read();
	let limine_l4_page_table: &mut PageTable = unsafe {
		&mut *((hhdm.offset as u64 + limine_page_table_frame.start_address().as_u64())
			as *mut PageTable)
	};
	let mut limine_mapper = OffsetPageTable::new(
		limine_l4_page_table,
		VirtAddr::new_unsafe(hhdm.offset as u64),
	);

	// Serialize the boot configuration to memory for the kernel to pick up
	// once we switch to it.
	let boot_config = BootConfig {
		magic: BOOT_MAGIC,
		nonce: boot_time as u64,
		nonce_xor_magic: BOOT_MAGIC ^ (boot_time as u64),
		memory_map: mmap.iter().map(|limine_region| MemoryRegion {
			base: limine_region.base,
			length: limine_region.length,
			kind: map_limine_to_oro_region(&limine_region.kind),
		}),
	};

	// Now we map in the stubs to the same address as where they'll exist
	// after the CR3 switch. We can use the page frame allocator still since
	// these stubs are reclaimable.
	map_stubs!(limine_mapper, oro_mapper, pfa);
	map_stubs!(limine_mapper, limine_mapper, pfa);

	boot_config.serialize(&mut OroBootAllocator::new(
		&mut pfa,
		&mut limine_mapper, // mapped as RW
		&mut oro_mapper,    // mapped as RO
	));

	boot_config.fast_forward_memory_map(
		l4_to_range_48(ORO_BOOT_PAGE_TABLE_INDEX).0,
		pfa.total_allocations,
		is_oro_region_allocatable,
	);

	// TODO Fast-forward the memory map provided to the kernel based on
	// TODO number of pages we've allocated here; for each page used,
	// TODO increase the base address and deduct the length. This still
	// TODO guarantees that physical memory regions are still sorted,
	// TODO and that the kernel can reliably use only unused physical
	// TODO memory, without needing to pass any additional information
	// TODO in the boot protocol structures.

	// Now that it's all mapped, we want to push our important stuff to registers
	// and jump to the stub
	dbg!("pre-boot OK; booting Oro");

	asm!(
		"push {L4_ADDR}",
		"push {STACK_ADDR}",
		"push {KERNEL_ENTRY}",
		"jmp {STUBS_ADDR}",
		L4_ADDR = in(reg) oro_l4_page_table_phys_addr,
		STACK_ADDR = in(reg) stack_init_addr,
		KERNEL_ENTRY = in(reg) kernel_entry_point,
		STUBS_ADDR = in(reg) STUBS_ADDR,
		options(noreturn)
	);
}

/// # Safety
/// DO NOT CALL. This is a trampoline stub that sets up
/// the kernel's execution environment prior to transferring
/// execution to the kernel.
#[cfg(target_arch = "x86_64")]
#[link_section = ".oro_stubs.entry"]
#[no_mangle]
#[naked]
unsafe extern "C" fn _oro_boot_stub() -> ! {
	asm!(
		"pop r10",
		"pop r9",
		"pop r8",
		"mov cr3, r8",
		"mov rsp, r9",
		"push 0", // Push a return value of 0 onto the stack to prevent accidental returns
		"jmp r10",
		options(noreturn)
	);
}
