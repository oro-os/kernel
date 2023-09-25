use crate::log;
use alloc::alloc::Layout;
use buddy_system_allocator::{Heap, LockedHeapWithRescue};
use core::mem::MaybeUninit;
use lazy_static::lazy_static;
use oro_boot::{
	x86_64::{
		l4_mkvirtaddr, l4_to_range_48, l4_to_recursive_table, MemoryRegionKind,
		KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES, ORO_BOOT_PAGE_TABLE_INDEX,
		RECURSIVE_PAGE_TABLE_INDEX,
	},
	Fake, Proxy, BOOT_MAGIC,
};
use spin::mutex::{spin::SpinMutex, TicketMutex};
use uart_16550::SerialPort;
use volatile::Volatile;
use x86_64::{
	structures::{
		gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
		idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
		paging::{
			page_table::PageTableEntry, FrameAllocator, FrameDeallocator, Mapper, Page, PageTable,
			PageTableFlags, PhysFrame, RecursivePageTable, Size4KiB,
		},
		tss::TaskStateSegment,
	},
	PhysAddr, VirtAddr,
};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
	static ref SERIAL: SpinMutex<SerialPort> = {
		let mut serial_port = unsafe { SerialPort::new(0x3F8) };
		serial_port.init();
		SpinMutex::new(serial_port)
	};
	static ref IDT: InterruptDescriptorTable = {
		let mut idt = InterruptDescriptorTable::new();
		idt.page_fault.set_handler_fn(irq_page_fault);
		idt.breakpoint.set_handler_fn(irq_breakpoint);
		idt
	};
	static ref TSS: TaskStateSegment = {
		let mut tss = TaskStateSegment::new();
		tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
			const STACK_SIZE: usize = 4096 * 5;
			static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

			let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
			stack_start + STACK_SIZE
		};
		tss
	};
	static ref GDT: (GlobalDescriptorTable, Selectors) = {
		let mut gdt = GlobalDescriptorTable::new();
		let cs = gdt.add_entry(Descriptor::kernel_code_segment());
		let ds = gdt.add_entry(Descriptor::kernel_data_segment());
		let tss = gdt.add_entry(Descriptor::tss_segment(&TSS));
		(gdt, Selectors { cs, ds, tss })
	};
	/// NOTE: DO NOT call `.clean_up()`, and DO NOT pass a range to `.clean_up_addr_range()`
	/// NOTE: that includes the PFA swap page!!!! This WILL cause the system to crash
	/// NOTE: and the kernel to page fault!!!!
	static ref KERNEL_MAPPER: SpinMutex<RecursivePageTable<'static>> = {
		let base_addr = l4_to_recursive_table(RECURSIVE_PAGE_TABLE_INDEX);
		let page_table = unsafe { &mut *(base_addr as *mut PageTable) };
		SpinMutex::new(RecursivePageTable::new(page_table).unwrap())
	};
}

static mut PFA: MaybeUninit<TicketMutex<PageFrameAllocator>> = MaybeUninit::uninit();

struct Selectors {
	cs: SegmentSelector,
	ds: SegmentSelector,
	tss: SegmentSelector,
}

type BootConfig = Proxy![oro_boot::x86_64::BootConfig<Fake<oro_boot::x86_64::MemoryRegion>>];
type MemoryRegion = Proxy![oro_boot::x86_64::MemoryRegion];

struct MemoryRegionIter {
	mmap: &'static [MemoryRegion],
	region_idx: u64,
	region_offset: u64,
}

impl MemoryRegionIter {
	fn new(mmap: &'static [MemoryRegion]) -> Self {
		Self {
			mmap,
			region_idx: 0,
			region_offset: 0,
		}
	}
}

/// Expects everything is aligned and sorted; this should be checked
/// by the kernel prior to initializing the PFA or other memory subsystems.
impl Iterator for MemoryRegionIter {
	type Item = u64;

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			if self.region_idx >= self.mmap.len() as u64 {
				return None;
			}

			if !is_region_allocatable(&self.mmap[self.region_idx as usize])
				|| self.region_offset >= self.mmap[self.region_idx as usize].length
			{
				self.region_idx += 1;
				self.region_offset = 0;
			} else {
				break;
			}
		}

		let base = self.mmap[self.region_idx as usize].base + self.region_offset;
		self.region_offset += 4096;
		Some(base)
	}
}

struct PageFrameAllocator {
	mmap: MemoryRegionIter,
	ptentry: &'static mut PageTableEntry,

	/// The physical address of the tip of the freed frame stack,
	/// or 0 if there exist no frames on the stack.
	free_tip: u64,
	/// The mapped swap page virtual address.
	/// IMPLEMENTATION **MUST NOT** USE THIS ADDRESS IN ANY WAY OTHER THAN
	/// FOR TLB INVALIDATION. To read and write the page, volatile operations
	/// should be used.
	mapped_addr: u64,
	/// A volatile reference to the first u64 of the mapped addr.
	/// Note that reads and writes to this address when a page is not
	/// mapped invokes UB in Rust. We're, almost literally, dancing with
	/// fire here. Be very careful.
	mapped_tip: Volatile<&'static mut u64>,
}

impl PageFrameAllocator {
	fn new(mmap: MemoryRegionIter, ptentry: &'static mut PageTableEntry, mapped_addr: u64) -> Self {
		Self {
			mmap,
			ptentry,
			free_tip: 0,
			mapped_addr,
			mapped_tip: Volatile::new(unsafe { &mut *(mapped_addr as *mut u64) }),
		}
	}
}

unsafe impl FrameAllocator<Size4KiB> for PageFrameAllocator {
	fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
		if self.free_tip == 0 {
			// Take a page off the memory map, or report that there is no more memory to use!
			self.mmap.next().map(|addr| unsafe {
				PhysFrame::from_start_address_unchecked(PhysAddr::new_unsafe(addr))
			})
		} else {
			// Take a frame off the top of the stack and allocate it.
			Some(unsafe {
				let phys_addr = PhysAddr::new_unsafe(self.free_tip);

				// load the frame into our swap location and flush the TLB
				self.ptentry.set_addr(
					phys_addr,
					PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
				);
				x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(self.mapped_addr));

				// read the next stack entry address
				self.free_tip = self.mapped_tip.read();

				// make sure we loaded something sensible; since we manage all of these addresses
				// ourselves (internally) we can reasonably expect this is *always* the case.
				debug_assert_eq!(self.free_tip % 4096, 0);

				// don't expose internal addresses, etc. to anything, let alone a userspace process
				self.mapped_tip.write(0u64);

				// unmap it (for security purposes, we don't want to accidentally keep that page around)
				self.ptentry.set_unused();
				x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(self.mapped_addr));

				PhysFrame::from_start_address_unchecked(phys_addr)
			})
		}
	}
}

impl FrameDeallocator<Size4KiB> for PageFrameAllocator {
	unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
		let frame_addr = frame.start_address().as_u64();

		// load the frame into our swap location and flush the TLB
		self.ptentry
			.set_frame(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
		x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(self.mapped_addr));

		// write the next tip address to the frame
		self.mapped_tip.write(self.free_tip);
		self.free_tip = frame_addr;

		// unmap it (for security purposes, we don't want to accidentally keep that page around)
		self.ptentry.set_unused();
		x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(self.mapped_addr));
	}
}

#[inline]
fn is_region_allocatable(region: &MemoryRegion) -> bool {
	region.kind == MemoryRegionKind::Usable
}

pub unsafe fn halt() -> ! {
	use core::arch::asm;
	asm!("cli");
	loop {
		asm!("hlt");
	}
}

pub fn print_args(args: core::fmt::Arguments) {
	use core::fmt::Write;
	SERIAL.lock().write_fmt(args).unwrap();
}

static mut CURRENT_HEAP_BASE: u64 = 0;
const KERNEL_SECRET_HEAP_LAST_VALID_ADDRESS: u64 =
	l4_to_range_48(KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES.1).1;

/// The number of bytes to allocate for new buddy arenas when the
/// global allocator panics due to being out of space.
const HEAP_CHUNK_SIZE: u64 = 512 * 4096;

#[global_allocator]
static KERNEL_SECRET_HEAP_ALLOCATOR: LockedHeapWithRescue<32> =
	LockedHeapWithRescue::new(|heap: &mut Heap<32>, layout: &Layout| {
		::x86_64::instructions::interrupts::without_interrupts(|| unsafe {
			if layout.size() as u64 > HEAP_CHUNK_SIZE {
				// Should never happen; if this ever _does_ happen, then the heap chunk size needs to
				// either be increased, or we need to modify how we perform panic recovery allocation
				// calculations.
				panic!("allocation of type larger than a single heap chunk size occurred");
			}

			// Should be properly initialized by the init routine.
			debug_assert_ne!(CURRENT_HEAP_BASE, 0);

			let heap_start = VirtAddr::new(CURRENT_HEAP_BASE);
			let heap_end = heap_start + HEAP_CHUNK_SIZE - 1u64;

			// >, not >= here since KERNEL_SECRET_HEAP_LAST_VALID_ADDRESS
			// is upper-bound inclusive.
			if heap_end.as_u64() > KERNEL_SECRET_HEAP_LAST_VALID_ADDRESS {
				panic!("kernel ran out of secret heap address space");
			}

			let page_range = {
				let heap_start_page = Page::containing_address(heap_start);
				let heap_end_page = Page::containing_address(heap_end);
				Page::range_inclusive(heap_start_page, heap_end_page)
			};

			let mut frame_allocator = PFA.assume_init_ref().lock();
			let mut mapper = KERNEL_MAPPER.lock();

			for page in page_range {
				if let Some(frame) = frame_allocator.allocate_frame() {
					match mapper.map_to_with_table_flags(
						page,
						frame,
						PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
						PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
						&mut *frame_allocator,
					) {
						Ok(_tlb_entry) => {
							// We don't do anything with it here; we instead
							// flush everything all at once after this.
						}
						Err(error) => {
							log::warning!(
								"failed to allocate more heap storage (kernel will panic): {error:?}"
							);
							return; // Will inevitably cause the heap allocator to fail and the kernel to stall.
						}
					}
				} else {
					return;
				}
			}

			::x86_64::instructions::tlb::flush_all();

			heap.add_to_heap(
				heap_start.as_u64() as usize,
				(heap_end.as_u64() + 1u64) as usize,
			);

			CURRENT_HEAP_BASE += HEAP_CHUNK_SIZE;
		});
	});

extern "x86-interrupt" fn irq_page_fault(frm: InterruptStackFrame, err_code: PageFaultErrorCode) {
	unsafe {
		SERIAL.force_unlock();
	}
	log::debug!("PAGE FAULT frm={frm:#?} err_code={err_code:#?}");
	unsafe {
		halt();
	}
}

extern "x86-interrupt" fn irq_breakpoint(_frm: InterruptStackFrame) {
	//use core::fmt::Write;
	//unsafe {
	//	SERIAL.force_unlock();
	//}
	//SERIAL.lock().write_str("BREAKPOINT").unwrap();
	//unsafe {
	//	halt();
	//}
}

pub fn init() {
	use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
	use x86_64::instructions::tables::load_tss;

	GDT.0.load();
	unsafe {
		CS::set_reg(GDT.1.cs);
		DS::set_reg(GDT.1.ds);
		SS::set_reg(GDT.1.ds);
		GS::set_reg(GDT.1.ds);
		FS::set_reg(GDT.1.ds);
		ES::set_reg(GDT.1.ds);
		load_tss(GDT.1.tss);
	}
	IDT.load();

	let boot_config =
		unsafe { &*(l4_to_range_48(ORO_BOOT_PAGE_TABLE_INDEX).0 as *const BootConfig) };

	// Validate the magic number
	if boot_config.magic != BOOT_MAGIC {
		panic!("boot config magic number mismatch");
	}
	if boot_config.nonce_xor_magic != (BOOT_MAGIC ^ boot_config.nonce) {
		panic!("boot config magic^nonce mismatch");
	}

	log::ok!("boot::oro x86_64");

	// Validate the memory map.
	//
	// Note that boot stages are free to advance the base/reduce the length (both)
	// in order to cull off initial frames they might have used during initialization.
	// Both the validator and the PFA will happily handle cases where entire regions are
	// empty, as long as the sort order remains intact.
	//
	// Note that we only check usable memory regions.
	{
		let mut last_end = 0;
		for (idx, region) in boot_config.memory_map.iter().enumerate() {
			if !is_region_allocatable(region) {
				continue;
			}

			if (region.base % 4096) != 0 {
				panic!(
					"boot config memory region index {idx} has unaligned base address: {}",
					region.base
				);
			}
			if (region.length % 4096) != 0 {
				panic!(
					"boot config memory region index {idx} has unaligned length: {}",
					region.length
				);
			}
			if region.base < last_end {
				panic!(
					"boot config memory region index {idx} has unsorted base: {} < {last_end}",
					region.base
				);
			}
			last_end = region.base + region.length;
		}
	}

	log::ok!("boot::boot stage memory map validation");

	// Set up the memory allocation subsystem
	{
		// ... set up PFA iterator
		let mut mmap_iter = MemoryRegionIter::new(boot_config.memory_map);

		// ... use iterator to allocate frames for the PFA swap space
		let secret_heap_l3_addr = mmap_iter
			.next()
			.expect("cannot allocate PFA swap L3: out of memory");
		let secret_heap_l2_addr = mmap_iter
			.next()
			.expect("cannot allocate PFA swap L2: out of memory");
		let secret_heap_l1_addr = mmap_iter
			.next()
			.expect("cannot allocate PFA swap L1: out of memory");

		// ... set up the PFA / mapper swap page page table entries ("page page" intentional)
		//     and calculate both the page entry address as well as the swap page base address
		//     for the PFA to later use
		let (pfa_page_table_entry, pfa_mapped_page_addr) = unsafe {
			// sanity check; this is documented in the file where it's defined
			// but it's always good to double check where it counts.
			#[allow(clippy::assertions_on_constants)]
			{
				debug_assert!(KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES.0 >= 256);
			}
			const HEAP_IDX: u16 = KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES.0;

			let mut mapper = KERNEL_MAPPER.lock();
			debug_assert!(
				!mapper.level_4_table()[HEAP_IDX as usize]
					.flags()
					.contains(PageTableFlags::PRESENT)
			);
			mapper.level_4_table()[HEAP_IDX as usize].set_addr(
				PhysAddr::new_unsafe(secret_heap_l3_addr),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
			let l3_vaddr = l4_mkvirtaddr(
				RECURSIVE_PAGE_TABLE_INDEX,
				RECURSIVE_PAGE_TABLE_INDEX,
				RECURSIVE_PAGE_TABLE_INDEX,
				HEAP_IDX,
			);
			x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(l3_vaddr));
			let l3_page_table = &mut *(l3_vaddr as *mut PageTable);
			l3_page_table.zero();
			l3_page_table[0].set_addr(
				PhysAddr::new_unsafe(secret_heap_l2_addr),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
			let l2_vaddr = l4_mkvirtaddr(
				RECURSIVE_PAGE_TABLE_INDEX,
				RECURSIVE_PAGE_TABLE_INDEX,
				HEAP_IDX,
				0,
			);
			x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(l2_vaddr));
			let l2_page_table = &mut *(l2_vaddr as *mut PageTable);
			l2_page_table.zero();
			l2_page_table[0].set_addr(
				PhysAddr::new_unsafe(secret_heap_l1_addr),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
			let l1_vaddr = l4_mkvirtaddr(RECURSIVE_PAGE_TABLE_INDEX, HEAP_IDX, 0, 0);
			x86_64::instructions::tlb::flush(VirtAddr::new_unsafe(l1_vaddr));
			let l1_page_table = &mut *(l1_vaddr as *mut PageTable);
			l1_page_table.zero();
			(&mut l1_page_table[0], l4_to_range_48(HEAP_IDX).0)
		};

		log::ok!("boot::recursive memory mapper");

		// ... set up PFA and pass PFA iterator + other memory items
		unsafe {
			PFA.write(TicketMutex::new(PageFrameAllocator::new(
				mmap_iter,
				pfa_page_table_entry,
				pfa_mapped_page_addr,
			)));
		}

		log::ok!("boot::page frame allocator");

		// ... unmap (and reclaim physical memory for) anything in lower half
		{
			// sanity check; this is documented in the file where it's defined
			// but it's always good to double check where it counts.
			#[allow(clippy::assertions_on_constants)]
			{
				debug_assert!(RECURSIVE_PAGE_TABLE_INDEX >= 256);
			}

			const REC: u16 = RECURSIVE_PAGE_TABLE_INDEX; // just as a convenience

			let mut mapper = KERNEL_MAPPER.lock();
			let mut pfa = unsafe { PFA.assume_init_ref() }.lock();
			let l4 = mapper.level_4_table();
			for l4_idx in 0..256u16 {
				let entry = &mut l4[l4_idx as usize];
				if entry.is_unused() {
					continue;
				}

				let l3 = unsafe { &mut *(l4_mkvirtaddr(REC, REC, REC, l4_idx) as *mut PageTable) };

				for l3_idx in 0..512u16 {
					let entry = &mut l3[l3_idx as usize];
					if entry.is_unused() {
						continue;
					}

					let l2 = unsafe {
						&mut *(l4_mkvirtaddr(REC, REC, l4_idx, l3_idx) as *mut PageTable)
					};

					for l2_idx in 0..512u16 {
						let entry = &mut l2[l2_idx as usize];
						if entry.is_unused() {
							continue;
						}
						let l1 = unsafe {
							&mut *(l4_mkvirtaddr(REC, l4_idx, l3_idx, l2_idx) as *mut PageTable)
						};

						for l1_idx in 0..512u16 {
							let entry = &mut l1[l1_idx as usize];
							if entry.is_unused() {
								continue;
							}

							if entry.flags().contains(PageTableFlags::BIT_9) {
								unsafe {
									pfa.deallocate_frame(entry.frame().unwrap());
								}
							}
							entry.set_unused();
						}

						if entry.flags().contains(PageTableFlags::BIT_9) {
							unsafe {
								pfa.deallocate_frame(entry.frame().unwrap());
							}
						}
						entry.set_unused();
					}

					if entry.flags().contains(PageTableFlags::BIT_9) {
						unsafe {
							pfa.deallocate_frame(entry.frame().unwrap());
						}
					}
					entry.set_unused();
				}

				if entry.flags().contains(PageTableFlags::BIT_9) {
					unsafe {
						pfa.deallocate_frame(entry.frame().unwrap());
					}
				}
				entry.set_unused();
			}

			// invalidate TLB
			::x86_64::instructions::tlb::flush_all();
		}

		log::ok!("boot::unmap+reclaim lower half");

		// ... set up global (kernel) buddy allocator (placing it _above_ anything
		//     we put in secret heap above, i.e. the PFA swap page)
		unsafe {
			CURRENT_HEAP_BASE = l4_mkvirtaddr(KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES.0, 1, 0, 0);
		}

		log::ok!("boot::kernel heap");
	}

	log::ok!("boot::memory subsystem");
}
