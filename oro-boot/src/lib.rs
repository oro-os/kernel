//! Boot routine for the Oro kernel.
//!
//! This crate is intended to be used by bootloaders
//! to boot into the Oro kernel via a standardized
//! and safe interface.
//!
//! The role of a bootloader implementation in Oro is to ultimately
//! call this function with a proper configuration, which
//! provides a clean and standardized way of initializing and booting
//! into the Oro kernel without needing to know the specifics of the
//! kernel's initialization process.
//!
//! This crate is not strictly necessary to boot the Oro kernel, but
//! provides a known-good way to do so.
#![cfg_attr(not(test), no_std)]
// SAFETY(qix-): Needed for the Target abstractions. This is accepted,
// SAFETY(qix-): just moving slowly.
// SAFETY(qix-): https://github.com/rust-lang/rust/pull/120700
#![feature(type_alias_impl_trait)]
// SAFETY(qix-): This isn't _super_ critical but it helps specify
// SAFETY(qix-): the `boot_to_kernel()` method. It's accepted, but
// SAFETY(qix-): has some dependencies that need to be stabilized.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/35121
#![feature(never_type)]
// SAFETY(qix-): Required for the transfer stubs.
// SAFETY(qix-): Already accepted, just moving slowly.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/90957
#![feature(naked_functions)]

mod map;
mod pfa;
mod target;

use oro_boot_protocol::{
	util::{RequestData, RequestScanner, TrySendError},
	DataRevision, RequestTag,
};
use oro_debug::{dbg, dbg_warn};
pub use oro_mem::mapper::MapError;
use oro_mem::{mapper::AddressSpace, translate::OffsetTranslator};

/// The bootstrapper error type.
#[derive(Debug, Clone, Copy)]
pub enum Error {
	/// An error occurred attempting to map memory into
	/// the new supervisor space.
	MapError(MapError),
	/// An error occurred when parsing the kernel ELF file.
	ElfError(oro_elf::ElfError),
	/// The provided kernel ELF file has no kernel segments.
	NoKernelSegments,
	/// The provided kernel ELF file has an invalid segment.
	InvalidSegment {
		/// The ELF flags of the unrecognized / invalid segment.
		flags: u32,
		/// The ELF type of the unrecognized / invalid segment.
		ptype: u32,
	},
	/// The kernel module has multiple kernel request segments.
	MultipleKernelRequestSegments,
	/// The kernel module has no kernel request segment.
	NoKernelRequestSegment,
}

/// The bootstrapper result type.
pub type Result<T> = core::result::Result<T, Error>;

/// Bootstrapper utility for mapping the kernel, populating requests,
/// and booting into the Oro kernel.
///
/// The use of this structure isn't strictly necessary, but provides a
/// known-good way to boot into the Oro kernel.
pub struct OroBootstrapper<
	M: Into<oro_boot_protocol::MemoryMapEntry> + Clone,
	I: Iterator<Item = M> + Clone,
> {
	/// The physical address translator that is used by this bootstrapper.
	pat: OffsetTranslator,
	/// The PFA used to write variable length bootloader protocol structures to memory.
	pfa: pfa::PrebootPfa<M, I>,
	/// The supervisor space
	supervisor_space: self::target::SupervisorHandle,
	/// The mapped kernel's request section scanner.
	scanner: RequestScanner,
	/// The entry point of the kernel (in the target address space).
	kernel_entry: usize,
	/// The target virtual address of the stack head.
	stack_addr: usize,
}

impl<M: Into<oro_boot_protocol::MemoryMapEntry> + Clone, I: Iterator<Item = M> + Clone>
	OroBootstrapper<M, I>
{
	/// Creates a new Oro bootloader instance from a memory map iterator.
	///
	/// `stack_pages` specifies the number of 4KiB pages to allocate for the kernel stack.
	///
	/// The iterator must convert any preboot memory region types into
	/// Oro memory region types. See below for how to handle the `used` field.
	///
	/// The `next` field of each entry is ignored and overwritten when booting.
	/// Set it to 0.
	///
	/// The `kernel_module.next` field is ignored. Set it to 0.
	///
	/// Returns an error if mapping the kernel fails.
	///
	/// ## `used` Field
	/// The `used` field on the memory region struct indicates how many bytes
	/// of an otherwise **usable** memory region are being used by the bootloader,
	/// and can be reclaimed after the Kernel has processed any boot-time information.
	///
	/// For all general purpose **immediately** usable regions of memory, set the
	/// type to [`oro_boot_protocol::MemoryMapEntryType::Usable`], and set the
	/// `used` field to 0.
	///
	/// For all bootloader reclaimable regions, set the type to `Usable` as well,
	/// but set the `used` property to the number of bytes in the region.
	///
	/// If the bootloader has used only part of a region, set the `used` field to
	/// the number of bytes used.
	///
	/// These counts **must not** affect the `length` field.
	///
	/// For all non-usable regions, this field should be set to 0 (but is otherwise
	/// ignored by the PFA and kernel).
	///
	/// # Panics
	/// Panics if the linear offset is not representable as a `usize`, or if
	/// `stack_pages` is zero.
	pub fn bootstrap(
		linear_offset: u64,
		stack_pages: usize,
		iter: I,
		kernel_module: oro_boot_protocol::Module,
	) -> Result<Self> {
		let pat =
			OffsetTranslator::new(usize::try_from(linear_offset).expect("linear offset too large"));
		let mut pfa = pfa::PrebootPfa::new(iter, linear_offset);
		let supervisor_space = target::AddressSpace::new_supervisor_space(&mut pfa, &pat)
			.ok_or(Error::MapError(MapError::OutOfMemory))?;

		let (kernel_entry, scanner) = self::map::map_kernel_to_supervisor_space(
			&mut pfa,
			&pat,
			&supervisor_space,
			kernel_module,
		)?;

		// Map in a stack
		let stack_addr =
			self::map::map_kernel_stack(&mut pfa, &pat, &supervisor_space, stack_pages)?;

		Ok(Self {
			pat,
			pfa,
			supervisor_space,
			scanner,
			kernel_entry,
			stack_addr,
		})
	}

	/// Populates the kernel with a response.
	///
	/// The bootloader is encouraged to send as many responses
	/// as it supports, including any revisions to the same
	/// request that it can support.
	///
	/// This method will quielty ignore any responses the bootloader sends
	/// that the kernel does not request - these are not considered
	/// errors.
	///
	/// # When populating [`oro_boot_protocol::ModulesRequest`]
	/// When populating the modules request, you must omit the
	/// kernel itself if the kernel is loaded as a module.
	///
	/// # Panics
	/// Panics if the request is one of the following, which are
	/// automatically handled by this bootstrapper when calling `boot_to_kernel`:
	///
	/// - [`oro_boot_protocol::MemoryMapRequest`]
	#[must_use]
	pub fn send<R: DataRevision>(mut self, response: R) -> Self
	where
		R::Request: RequestData,
	{
		assert_ne!(
			<R::Request as RequestTag>::TAG,
			oro_boot_protocol::MemoryMapRequest::TAG,
			"the `MemoryMap` request is handled automatically by the bootstrapper; do not send \
			 one yourself"
		);

		try_send(&mut self.scanner, response);
		self
	}

	/// Serializes the given item(s) into the kernel's memory,
	/// returning a physical address that can be handed to the kernel
	/// via a `u64` request field.
	///
	/// If no items are yielded from the iterator, this function
	/// returns zero.
	///
	/// # Safety
	/// It is up to the caller to ensure that the datatype that is
	/// serialized is the requested type, as the boot protocol only
	/// works with physical addresses with no associated type information.
	pub fn serialize<T: oro_boot_protocol::util::SetNext>(
		&mut self,
		iter: impl Iterator<Item = T>,
	) -> Result<u64> {
		let mut last_phys = 0;

		for item in iter {
			let (phys, data) = self
				.pfa
				.allocate::<T>()
				.ok_or(Error::MapError(MapError::OutOfMemory))?;
			data.write(item);
			last_phys = phys;
		}

		Ok(last_phys)
	}

	/// Consumes this object and boots into the Oro kernel.
	///
	/// Returns an error if mapping the memory map or boot stubs
	/// fails.
	pub fn boot_to_kernel(mut self) -> Result<!> {
		// SAFETY(qix-): There's nothing we can really do to make this 'safe' by marking it as such;
		// SAFETY(qix-): the bootstrap class removes most of the danger associated with this method.
		#[allow(clippy::let_unit_value, clippy::semicolon_if_nothing_returned)]
		let transfer_data = unsafe {
			self::target::prepare_transfer(&mut self.supervisor_space, &mut self.pfa, &self.pat)?
		};

		// Consume the PFA and write out the memory map.
		let first_entry = self
			.pfa
			.write_memory_map()
			.ok_or(Error::MapError(MapError::OutOfMemory))?;

		// Send the memory map to the kernel.
		try_send(
			&mut self.scanner,
			oro_boot_protocol::memory_map::MemoryMapDataV0 { next: first_entry },
		);

		// Perform the transfer
		// SAFETY(qix-): We can assume the kernel entry point is valid given that it's
		// SAFETY(qix-): coming from the ELF and validated by the mapper.
		unsafe {
			self::target::transfer(
				&mut self.supervisor_space,
				self.kernel_entry,
				self.stack_addr,
				transfer_data,
			)
			.map_err(Error::MapError)?
		}
	}
}

#[expect(clippy::missing_docs_in_private_items)]
fn try_send<R: DataRevision>(scanner: &mut RequestScanner, response: R)
where
	R::Request: RequestData,
{
	match scanner.try_send(response) {
		Ok(()) => {
			dbg!(
				"sent response: {:?} rev {}",
				<R::Request as RequestTag>::TAG,
				R::REVISION
			);
		}
		Err(TrySendError::NotRequested) => {
			dbg_warn!(
				"skipped sent response (kernel didn't request this tag): {:?} rev {}",
				<R::Request as RequestTag>::TAG,
				R::REVISION
			);
		}
		Err(TrySendError::WrongRevision { expected }) => {
			dbg_warn!(
				"skipped sent response (kernel requested a different revision): {:?} rev {} \
				 (expected {})",
				<R::Request as RequestTag>::TAG,
				R::REVISION,
				expected
			);
		}
	}
}
