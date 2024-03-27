use crate::PageTableEntry;
use oro_common::mem::FiloPageFrameManager;

/// A [`FiloPageFrameManager`] that loads page frames at a fixed address.
///
/// Note that this struct is **VERY** `unsafe` to use unless used correctly.
/// Please check the safety notes on the `new` method before using this struct.
pub struct FixedAddressPageFrameManager {
	virtual_address: u64,
	page_table_entry: &'static mut PageTableEntry,
	currently_allocated: u64,
}

impl FixedAddressPageFrameManager {
	/// Creates a new `FixedAddressPageFrameManager`, loading physical frames
	/// into `virtual_address` by way of the given `page_table_entry` in order
	/// to form a FILO stack.
	///
	/// # Safety
	/// The `page_table_entry` must be the valid page table entry
	/// corresponding to `virtual_address`.
	///
	/// Further, it must **never be modified** by any other part of the kernel,
	/// including other instances of `FixedAddressPageFrameManager`, and the virtual
	/// address must never be accessed outside of this instance.
	///
	/// The virtual address must also be page-aligned.
	#[inline]
	#[must_use]
	pub const unsafe fn new(
		virtual_address: u64,
		page_table_entry: &'static mut PageTableEntry,
	) -> Self {
		Self {
			virtual_address,
			page_table_entry,
			currently_allocated: u64::MAX,
		}
	}

	fn load_page_frame(&mut self, address: u64) {
		if self.currently_allocated != address {
			unsafe {
				self.page_table_entry.set(
					PageTableEntry::new()
						.with_present()
						.with_writable()
						.with_no_exec()
						.with_address(address),
				);
				crate::asm::invlpg(self.virtual_address);
			}

			self.currently_allocated = address;
		}
	}
}

unsafe impl FiloPageFrameManager for FixedAddressPageFrameManager {
	unsafe fn read_u64(&mut self, address: u64) -> u64 {
		self.load_page_frame(address);
		unsafe { core::ptr::read_volatile(self.virtual_address as *const u64) }
	}

	unsafe fn write_u64(&mut self, address: u64, value: u64) {
		self.load_page_frame(address);
		unsafe { core::ptr::write_volatile(self.virtual_address as *mut u64, value) }
	}
}
