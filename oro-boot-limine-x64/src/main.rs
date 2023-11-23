#![no_std]
#![no_main]
#![feature(
	naked_functions,
	core_intrinsics,
	more_qualified_paths,
	never_type,
	asm_const
)]

use core::{arch::asm, ffi::CStr};
use elf::{endian::AnyEndian, ElfBytes, ParseError};
use lazy_static::lazy_static;
#[cfg(debug_assertions)]
use limine::StackSizeRequest;
use limine::{
	BootTimeRequest, HhdmRequest, MemmapEntry, MemmapRequest, MemoryMapEntryType, ModuleRequest,
	NonNullPtr, Ptr,
};
#[cfg(oro_test)]
use oro_arch_x64::KERNEL_TEST_SHM_PAGE_TABLE_INDEX;
use oro_arch_x64::{
	l4_to_range_48, Allocator, BootConfig, MemoryRegion, MemoryRegionKind, Serialize, BOOT_MAGIC,
	KERNEL_STACK_PAGE_TABLE_INDEX, ORO_BOOT_PAGE_TABLE_INDEX, RECURSIVE_PAGE_TABLE_INDEX,
};
use spin::Mutex;
use uart_16550::SerialPort;
#[cfg(oro_test)]
use x86_64::structures::paging::mapper::CleanUp;
use x86_64::{
	addr::{PhysAddr, VirtAddr},
	structures::paging::{
		frame::PhysFrame,
		mapper::{MapToError, OffsetPageTable},
		page::{Page, PageSize, Size4KiB},
		page_table::{PageTable, PageTableFlags},
		FrameAllocator, FrameDeallocator, Mapper, Translate,
	},
};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("oro-limine-boot-x64 can only be built for x86_64 targets");

lazy_static! {
	static ref SERIAL: Mutex<SerialPort> = {
		let mut serial_port = unsafe { SerialPort::new(0x3F8) };
		serial_port.init();
		Mutex::new(serial_port)
	};
}

/// We can put it here since all memory in the lower half are
/// unmapped in the kernel upon boot, and all pages are reclaimed.
///
/// During tests, this becomes a higher-half address, such that the
/// kernel and bootloader have a means of passing execution back and forth.
#[cfg(not(oro_test))]
const STUBS_ADDR: u64 = 0x0000_6000_0000_0000;
#[cfg(oro_test)]
const STUBS_ADDR: u64 = l4_to_range_48(KERNEL_TEST_SHM_PAGE_TABLE_INDEX).0;

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
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new(0);
#[used]
static MMAP_REQUEST: MemmapRequest = MemmapRequest::new(0);
#[used]
static MOD_REQUEST: ModuleRequest = ModuleRequest::new(0);
#[used]
static TIME_REQUEST: BootTimeRequest = BootTimeRequest::new(0);

#[cfg(debug_assertions)]
#[used]
static STKSZ_REQUEST: StackSizeRequest = StackSizeRequest::new(0).stack_size(64 * 1024);

fn map_limine_to_oro_region(kind: &MemoryMapEntryType) -> MemoryRegionKind {
	match kind {
		MemoryMapEntryType::Usable => MemoryRegionKind::Usable,
		MemoryMapEntryType::KernelAndModules => MemoryRegionKind::Modules,
		// We don't tell the kernel it can reclaim bootloader frames
		// if we're running the test suite because we need to be able to loop
		// back to the bootloader to re-initialize the kernel and run the next
		// test.
		#[cfg(not(oro_test))]
		MemoryMapEntryType::BootloaderReclaimable => MemoryRegionKind::Usable,
		_ => MemoryRegionKind::Reserved,
	}
}

#[inline]
fn is_oro_region_allocatable(kind: &MemoryRegionKind) -> bool {
	kind == &MemoryRegionKind::Usable
}

struct LiminePageFrameAllocator {
	bios_mapping: &'static [NonNullPtr<MemmapEntry>],
	bios_mapping_offset: usize,
	byte_offset: u64,
	byte_offset_max: u64,
	total_allocations: u64,
	#[cfg(oro_test)]
	hhdm_offset: u64,
}

impl LiminePageFrameAllocator {
	fn new(bios_mapping: &'static [NonNullPtr<MemmapEntry>]) -> Self {
		// get byte offset of first mapping (doesn't need to be usable, just the valid base offset)
		let (byte_offset, byte_offset_max) = if bios_mapping.is_empty() {
			(0, 0)
		} else {
			(
				bios_mapping[0].base,
				bios_mapping[0].base + bios_mapping[0].len,
			)
		};

		Self {
			bios_mapping,
			bios_mapping_offset: 0,
			byte_offset,
			byte_offset_max,
			total_allocations: 0,
			#[cfg(oro_test)]
			hhdm_offset: 0,
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
					if is_oro_region_allocatable(&map_limine_to_oro_region(&mapping.typ)) {
						self.bios_mapping_offset = i;
						self.byte_offset = mapping.base;
						self.byte_offset_max = self.byte_offset + mapping.len;
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

		#[cfg(oro_test)]
		{
			// If we're in a test mode, we zero the page before returning it.
			// This is to make sure that no test leaks memory.
			assert!(self.hhdm_offset != 0);
			let virt_addr = offset + self.hhdm_offset;
			unsafe {
				::core::intrinsics::volatile_set_memory(
					virt_addr as *mut u8,
					0u8,
					Size4KiB::SIZE as usize,
				)
			};
		}

		Some(unsafe { PhysFrame::from_start_address_unchecked(PhysAddr::new_unsafe(offset)) })
	}
}

trait DebugPrint {
	fn dbgprint(&self);
}

impl DebugPrint for &str {
	fn dbgprint(&self) {
		use core::fmt::Write;
		let _ = SERIAL.lock().write_str(self);
	}
}

impl DebugPrint for u64 {
	fn dbgprint(&self) {
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
	fn dbgprint(&self) {
		let mut sp = SERIAL.lock();
		for b in self.to_bytes_with_nul() {
			if *b == 0 {
				break;
			}
			sp.send(*b);
		}
	}
}

impl DebugPrint for Ptr<i8> {
	fn dbgprint(&self) {
		self.to_str().unwrap().dbgprint();
	}
}

macro_rules! dbg {
	($($e:expr),*) => {
		$($e.dbgprint();)*
		"\n".dbgprint();
	}
}

unsafe fn halt() -> ! {
	asm!("cli");
	loop {
		asm!("hlt");
	}
}

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	dbg!("panic::preboot::boot error");
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
		let page = page.into_page();

		// Note that bit 9 indicates to the kernel that these pages can be re-claimed if need be.
		// We mark any allocated physical frames used for page tables as such.
		match self.map_to_with_table_flags(
			page,
			frame.into_frame(),
			flags,
			PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::BIT_9,
			allocator,
		) {
			Ok(flusher) => flusher.flush(),
			Err(err) => {
				dbg!(
					"error::preboot::failed to map memory: ",
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
				dbg!("error::preboot::out of memory when allocating boot protocol structures");
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
			dbg!("error::preboot::stubs take up too much memory (more than 512 pages)");
			halt();
		}

		for page in start_page..end_page {
			$mapper.map_or_die(
				STUBS_ADDR + ((page - start_page) << 12),
				match $src_mapper.translate_addr(VirtAddr::new_unsafe(page << 12)) {
					Some(addr) => addr,
					None => {
						dbg!("error::preboot::stubs were not mapped in correctly");
						halt();
					}
				},
				PageTableFlags::PRESENT,
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
					dbg!("error::preboot::failed to allocate oro kernel segment: out of memory");
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
		dbg!("error::preboot::oro kernel has no loadable segments");
		halt();
	}

	// Return the entry point
	Ok(elf.ehdr.e_entry)
}

struct NoopDeallocator;

impl FrameDeallocator<Size4KiB> for NoopDeallocator {
	unsafe fn deallocate_frame(&mut self, _frame: PhysFrame<Size4KiB>) {}
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
			dbg!("error::preboot::failed to load oro kernel");
			halt();
		}
	}
}

/// # Safety
/// Do not call directly; only meant to be called by the Limine bootloader!
#[inline(never)]
#[no_mangle]
pub unsafe fn _start() -> ! {
	x86_64::instructions::interrupts::disable();

	dbg!("ok::preboot::booting oro + limine");

	let hhdm = if let Some(res) = HHDM_REQUEST.get_response().get() {
		res
	} else {
		dbg!("error::preboot::missing limine hhdm response");
		halt();
	};

	let mmap = if let Some(res) = MMAP_REQUEST.get_response().get() {
		res.memmap()
	} else {
		dbg!("error::preboot::missing limine mmap response");
		halt();
	};

	let mods = if let Some(res) = MOD_REQUEST.get_response().get() {
		res.modules()
	} else {
		dbg!("error::preboot::missing limine modules response (or no modules specified)");
		halt();
	};

	let boot_time = if let Some(res) = TIME_REQUEST.get_response().get() {
		res.boot_time
	} else {
		dbg!("error::preboot::missing limine boot time response");
		halt();
	};

	#[cfg(debug_assertions)]
	if STKSZ_REQUEST.get_response().get().is_none() {
		dbg!("warn::preboot::Oro + limine boot stage built in debug mode, which");
		dbg!("warn::preboot::means we request a much, much larger stack size to");
		dbg!("warn::preboot::accommodate Rust's large debug sizes, namely around");
		dbg!("warn::preboot::parsing the kernel ELF module. However, Limine has");
		dbg!("warn::preboot::not honored the stack size adjustment request, which");
		dbg!("warn::preboot::means some crazy stuff is probably about to happen,");
		dbg!("warn::preboot::the best case being a reboot or stall (triple-fault).");
	}

	#[cfg_attr(not(oro_test), allow(clippy::never_loop))]
	loop {
		// The limine page frame allocator is a simple, temporary page frame allocator
		// that uses the memory map Limine gives us directly, used to allocate the Oro
		// boot protocol structures as well as the physical frames for the boot stubs.
		// The frames that get used here, sans boot stub page table frames, are marked
		// as "Oro boot protocol reclaimable" frames that the OS is free to reclaim if
		// it can.
		let mut pfa = LiminePageFrameAllocator::new(mmap);

		// If we're in a test mode, we write the offset address to the PFA so that it can
		// zero each allocated page. We do this to make sure that no test leaks memory.
		#[cfg(oro_test)]
		{
			pfa.hhdm_offset = hhdm.offset;
		}

		// Make the OS's root page table.
		let (oro_l4_page_table, oro_l4_page_table_phys_addr): (&mut PageTable, u64) = unsafe {
			let phys_addr = match pfa.allocate_frame() {
				Some(frame) => frame.start_address().as_u64(),
				None => {
					dbg!("error::preboot::cannot allocate Oro L4 page table; out of memory");
					halt();
				}
			};

			let mapped_addr = phys_addr + hhdm.offset;
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
			OffsetPageTable::new(oro_l4_page_table, VirtAddr::new_unsafe(hhdm.offset));

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
					dbg!("error::preboot::failed to allocate kernel stack; out of memory");
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
				if cstr_eq(module.path.to_str().unwrap(), b"/oro-kernel\0") {
					kernel_entry_point = load_kernel_elf(
						core::slice::from_raw_parts(
							module.base.as_ptr().unwrap(),
							module.length as usize,
						),
						hhdm.offset,
						&mut oro_mapper,
						&mut pfa,
					);
					break 'load_kernel;
				} else {
					dbg!(
						"warn::preboot::unused module (unrecognized path): ",
						module.path
					);
				}
			}

			dbg!("error::preboot::'/oro-kernel' module not found on boot medium");
			halt();
		}

		if kernel_entry_point == 0 {
			// Should never happen but good to check just to
			// safeguard against bugs above.
			dbg!("error::preboot::oro kernel entry point is null");
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
			&mut *((hhdm.offset + limine_page_table_frame.start_address().as_u64())
				as *mut PageTable)
		};
		let mut limine_mapper =
			OffsetPageTable::new(limine_l4_page_table, VirtAddr::new_unsafe(hhdm.offset));

		// Serialize the boot configuration to memory for the kernel to pick up
		// once we switch to it.
		let boot_config = BootConfig {
			magic: BOOT_MAGIC,
			nonce: boot_time as u64,
			nonce_xor_magic: BOOT_MAGIC ^ (boot_time as u64),
			memory_map: mmap.iter().map(|limine_region| MemoryRegion {
				base: limine_region.base,
				length: limine_region.len,
				kind: map_limine_to_oro_region(&limine_region.typ),
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

		dbg!("ok::preboot");

		// Now that it's all mapped, we want to push our important stuff to registers
		// and jump to the stub
		#[cfg(not(oro_test))]
		{
			asm!(
				"jmp r11",
				in("r8") oro_l4_page_table_phys_addr,
				in("r9") stack_init_addr,
				in("r10") kernel_entry_point,
				in("r11") STUBS_ADDR,
				options(noreturn)
			);
		}

		#[cfg(oro_test)]
		{
			x86_64::instructions::tlb::flush_all();

			asm!(
				"and rsp, -16",
				"call r11",
				in("r8") oro_l4_page_table_phys_addr,
				in("r9") stack_init_addr,
				in("r10") kernel_entry_point,
				in("r11") STUBS_ADDR,
			);

			asm!(
				"nop",
				out("r8") _,
				out("r9") _,
				out("r10") _,
				out("r11") _,
				out("r12") _,
				out("rax") _,
			);

			// Unmap the stubs from the limine_mapper
			let start_page = (&_ORO_STUBS_START as *const u8 as u64) >> 12;
			let end_page = (&_ORO_STUBS_END as *const u8 as u64) >> 12;
			for page in start_page..end_page {
				if limine_mapper
					.unmap(Page::<Size4KiB>::from_start_address_unchecked(
						VirtAddr::new_unsafe(STUBS_ADDR + ((page - start_page) << 12)),
					))
					.is_err()
				{
					dbg!("error::preboot::failed to unmap stubs from boot stage address space");
					halt();
				};
			}

			// Unmap the boot protocol structures from the limine_mapper by starting from
			// the first page, checking to see if each successive page is mapped in, and unmapping it,
			// looping until we hit the first page that's not mapped in.
			let (boot_config_page, _) = l4_to_range_48(ORO_BOOT_PAGE_TABLE_INDEX);
			let mut page = boot_config_page >> 12;
			loop {
				let page_addr = VirtAddr::new_unsafe(page << 12);
				let page_addr = Page::<Size4KiB>::from_start_address_unchecked(page_addr);
				if limine_mapper.translate_page(page_addr).is_ok() {
					if limine_mapper.unmap(page_addr).is_err() {
						dbg!(
							"error::preboot::failed to unmap boot protocol structures from boot stage address space"
						);
						halt();
					}
					page += 1;
				} else {
					break;
				}
			}

			limine_mapper.clean_up(&mut NoopDeallocator);

			x86_64::instructions::tlb::flush_all();

			dbg!("debug::preboot::kernel returned; restarting");
		}
	}
}

/// # Safety
/// DO NOT CALL. This is a trampoline stub that sets up
/// the kernel's execution environment prior to transferring
/// execution to the kernel.
#[cfg(not(oro_test))]
#[link_section = ".oro_stubs.entry"]
#[no_mangle]
#[naked]
unsafe extern "C" fn _oro_boot_stub() -> ! {
	asm!(
		"mov cr3, r8",
		"mov rsp, r9",
		"push 0", // Push a return value of 0 onto the stack to prevent accidental returns
		"jmp r10",
		options(noreturn)
	);
}

/// A version of the boot stub used during tests.
/// It's guaranteed NOT to be unmapped by the kernel,
/// since in test modes we map this into a higher-half
/// section guaranteed not to be touched by the kernel.
///
/// # Safety
/// DO NOT CALL. This is a trampoline stub that sets up
/// the kernel's execution environment prior to transferring
/// execution to the kernel.
#[cfg(oro_test)]
#[link_section = ".oro_stubs.entry"]
#[no_mangle]
#[naked]
unsafe extern "C" fn _oro_boot_stub_call() {
	asm!(
		"pushf",
		"push rax",
		"push rbx",
		"push rcx",
		"push rdx",
		"push rsi",
		"push rdi",
		"push rbp",
		"push r13",
		"push r14",
		"push r15",
		"xor rax, rax",
		"str ax",
		"push rax",
		"sub rsp, 20",
		"sgdt [rsp]",
		"sidt [rsp+10]",
		"mov r11, cr3",
		"mov cr3, r8",
		"mov r12, rsp",
		"mov rsp, r9",
		"push r11",
		"push r12",
		"call r10",
		"cli",
		"pop r12",
		"pop r11",
		"mov rsp, r12",
		"mov cr3, r11",
		"lgdt [rsp]",
		"lidt [rsp+10]",
		"add rsp, 20",
		"pop rax",
		"ltr ax",
		"pop r15",
		"pop r14",
		"pop r13",
		"pop rbp",
		"pop rdi",
		"pop rsi",
		"pop rdx",
		"pop rcx",
		"pop rbx",
		"pop rax",
		"popf",
		"ret",
		options(noreturn)
	);
}
