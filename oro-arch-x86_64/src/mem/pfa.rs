use oro_common::FiloPageFrameManager;

/// A [`FiloPageFrameManager`] that loads page frames at a fixed address.
pub struct FixedAddressPageFrameManager<const ADDRESS: u64> {
	_currently_allocated: u64,
}

impl<const ADDRESS: u64> FixedAddressPageFrameManager<ADDRESS> {
	/// Creates a new `FixedAddressPageFrameManager`.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			_currently_allocated: u64::MAX,
		}
	}
}

unsafe impl<const ADDRESS: u64> FiloPageFrameManager for FixedAddressPageFrameManager<ADDRESS> {
	unsafe fn read_u64(&mut self, _address: u64) -> u64 {
		todo!();
	}

	unsafe fn write_u64(&mut self, _address: u64, _value: u64) {
		todo!();
	}
}
