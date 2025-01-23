//! Known-good method of mapping the kernel module
//! into a supervisor address space.

use oro_debug::dbg;
use oro_elf::{Elf, ElfSegment, ElfSegmentType};
use oro_mem::{
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	phys::{Phys, PhysAddr},
};

use crate::target::{AddressSpace as TargetAddressSpace, ELF_CLASS, ELF_ENDIANNESS, ELF_MACHINE};

/// Maps in the kernel module and returns the entry point
/// and a request scanner for populating the kernel's requests.
///
/// # Panics
/// Panics if the kernel module's `length` field does not fit into a `usize`.
pub fn map_kernel_to_supervisor_space<
	M: Into<oro_boot_protocol::MemoryMapEntry> + Clone,
	I: Iterator<Item = M> + Clone,
>(
	pfa: &crate::pfa::UnsafePrebootPfa<M, I>,
	supervisor_space: &<crate::target::AddressSpace as AddressSpace>::SupervisorHandle,
	kernel: &crate::Kernel,
) -> crate::Result<(usize, oro_boot_protocol::util::RequestScanner)> {
	// Parse the kernel ELF module.
	// SAFETY(qix-): We can assume the kernel module is valid given that it's
	// SAFETY(qix-): been loaded by the bootloader.
	let kernel_elf = unsafe {
		Elf::parse(
			Phys::from_address_unchecked(kernel.base).as_ptr_unchecked(),
			usize::try_from(kernel.length).unwrap(),
			ELF_ENDIANNESS,
			ELF_CLASS,
			ELF_MACHINE,
		)
		.map_err(crate::Error::ElfError)?
	};

	let num_segments = kernel_elf.segments().count();
	if num_segments == 0 {
		return Err(crate::Error::NoKernelSegments);
	}

	let mut kernel_request_scanner = None;

	for segment in kernel_elf.segments() {
		let mapper_segment = match segment.ty() {
			ElfSegmentType::Ignored => continue,
			ElfSegmentType::KernelCode => <TargetAddressSpace as AddressSpace>::kernel_code(),
			ElfSegmentType::KernelData => <TargetAddressSpace as AddressSpace>::kernel_data(),
			ElfSegmentType::KernelRoData | ElfSegmentType::KernelRequests => {
				<TargetAddressSpace as AddressSpace>::kernel_rodata()
			}
			ty => {
				return Err(crate::Error::InvalidSegment(ty));
			}
		};

		// NOTE(qix-): This will almost definitely be improved in the future.
		// NOTE(qix-): At the very least, hugepages will change this.
		// NOTE(qix-): There will probably be some better machinery for
		// NOTE(qix-): mapping ranges of memory in the future.
		for page in 0..(segment.target_size().saturating_add(0xFFF) >> 12) {
			// SAFETY: We're only accessing the inner PFA for a short period.
			let phys_addr = unsafe { pfa.get_mut() }
				.allocate_page()
				.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;

			if page == 0 && segment.ty() == ElfSegmentType::KernelRequests {
				if kernel_request_scanner.is_some() {
					return Err(crate::Error::MultipleKernelRequestSegments);
				}

				// SAFETY(qix-): We can assume the kernel module is valid given that it's
				// SAFETY(qix-): been loaded by the bootloader.
				kernel_request_scanner = Some(unsafe {
					oro_boot_protocol::util::RequestScanner::new(
						Phys::from_address_unchecked(phys_addr).as_mut_ptr_unchecked(),
						segment.target_size(),
					)
				});
			}

			let byte_offset = page << 12;
			// Saturating sub here since the target size might exceed the file size,
			// in which case we have to keep allocating those pages and zeroing them.
			let load_size = segment.load_size().saturating_sub(byte_offset).min(4096);
			let load_virt = segment.load_address() + byte_offset;
			let target_virt = segment.target_address() + byte_offset;

			let local_page_virt =
				unsafe { Phys::from_address_unchecked(phys_addr).as_mut_ptr_unchecked::<u8>() };

			// SAFETY(qix-): We can assume the kernel module is valid given that it's
			// SAFETY(qix-): been loaded by the bootloader.
			let (src, dest) = unsafe {
				(
					core::slice::from_raw_parts(load_virt as *const u8, load_size),
					core::slice::from_raw_parts_mut(local_page_virt, 4096),
				)
			};

			// copy data
			if load_size > 0 {
				dest[..load_size].copy_from_slice(&src[..load_size]);
			}
			// zero remaining
			if load_size < 4096 {
				dest[load_size..].fill(0);
			}

			if let Err(err) =
				mapper_segment.map_nofree_in(supervisor_space, pfa, target_virt, phys_addr)
			{
				panic!(
					"failed to map kernel segment: {err:?}: ls={load_size} p={page} po={page:X?} \
					 lv={load_virt:#016X} tv={target_virt:#016X} s={segment:016X?}"
				);
			}
		}

		dbg!(
			"mapped kernel segment: {:#016X?} <{:X?}> -> {:?} <{:X?}>",
			segment.target_address(),
			segment.target_size(),
			segment.ty(),
			segment.target_size(),
		);
	}

	match kernel_request_scanner {
		Some(scanner) => Ok((kernel_elf.entry_point(), scanner)),
		None => Err(crate::Error::NoKernelRequestSegment),
	}
}

/// Maps the kernel stack into the supervisor space.
///
/// # Panics
/// Panics if the kernel stack segment already contains
/// mappings.
pub fn map_kernel_stack<
	M: Into<oro_boot_protocol::MemoryMapEntry> + Clone,
	I: Iterator<Item = M> + Clone,
>(
	pfa: &crate::pfa::UnsafePrebootPfa<M, I>,
	supervisor_space: &<crate::target::AddressSpace as AddressSpace>::SupervisorHandle,
	stack_pages: usize,
) -> crate::Result<usize> {
	let kernel_stack_segment = <TargetAddressSpace as AddressSpace>::kernel_stack();

	// TODO(qix-): This is nutty. There needs to be a better way to express this.
	let last_stack_page_virt =
		<<TargetAddressSpace as AddressSpace>::SupervisorSegment as AddressSegment<
			<TargetAddressSpace as AddressSpace>::SupervisorHandle,
		>>::range(&kernel_stack_segment)
		.1 & !0xFFF;

	// make sure top guard page is unmapped
	match kernel_stack_segment.unmap_in(supervisor_space, pfa, last_stack_page_virt) {
		// NOTE(qix-): The Ok() case would never hit here since the PFA doesn't support
		// NOTE(qix-): freeing pages.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => panic!("failed to test unmap of top kernel stack guard page: {e:?}"),
	}

	let mut bottom_stack_page_virt = last_stack_page_virt;
	for _ in 0..stack_pages {
		bottom_stack_page_virt -= 4096;

		// SAFETY: We're only accessing the PFA for a short moment.
		let stack_phys = unsafe { pfa.get_mut() }
			.allocate_page()
			.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;

		kernel_stack_segment
			.remap_in(supervisor_space, pfa, bottom_stack_page_virt, stack_phys)
			.map_err(crate::Error::MapError)?;
	}

	// Make sure that the bottom guard page is unmapped
	match kernel_stack_segment.unmap_in(supervisor_space, pfa, bottom_stack_page_virt - 4096) {
		// NOTE(qix-): The Ok() case would never hit here since the PFA doesn't support
		// NOTE(qix-): freeing pages.
		Ok(_) => unreachable!(),
		Err(UnmapError::NotMapped) => {}
		// NOTE(qix-): Should never happen.
		Err(e) => panic!("failed to test unmap of kernel bottom stack guard page: {e:?}"),
	}

	Ok(last_stack_page_virt)
}
