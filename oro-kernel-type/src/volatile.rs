//! Volatile cells and registers.

use core::cell::UnsafeCell;

/// A volatile cell.
#[repr(transparent)]
pub struct Volatile<T> {
	/// The internal pointer to the value.
	///
	/// `UnsafeCell` is used as it is a special case in the compiler
	/// when it comes to UB prevention and optimization.
	value: UnsafeCell<T>,
}

impl<T> Volatile<T>
where
	T: Copy + Send,
{
	/// Creates a new volatile cell.
	///
	/// # Discouraged
	/// You probably don't want to create a `Volatile` directly.
	/// It's intended to be used by casting a pointer to e.g. memory
	/// mapped registers to a `Volatile`.
	///
	/// If you would like a nice checked cast, you can use the
	/// [`Volatile::try_cast`] method.
	#[inline(always)]
	#[must_use]
	pub const fn new(value: T) -> Self {
		Self {
			value: UnsafeCell::new(value),
		}
	}

	/// Attempts to cast the given pointer to a volatile cell.
	///
	/// Returns `Null` if the pointer is null or unaligned.
	///
	/// # Safety
	/// The pointed-to value must be valid for the lifetime of the returned
	/// `Volatile` reference.
	#[inline(always)]
	#[must_use]
	pub unsafe fn try_cast(ptr: *const T) -> Option<&'static Self> {
		if ptr.is_aligned() {
			// SAFETY: We assume that the pointer is valid for the lifetime of the
			// SAFETY: returned reference.
			unsafe { ptr.cast::<Self>().as_ref() }
		} else {
			None
		}
	}

	/// Gets the value.
	#[inline(always)]
	#[must_use]
	pub fn get(&self) -> T {
		// SAFETY: We assume that since there is a valid reference to `self`,
		// SAFETY: the value has valid pointer semantics.
		unsafe { self.value.get().read_volatile() }
	}

	/// Sets the value.
	#[inline(always)]
	pub fn set(&self, value: T) {
		// SAFETY: We assume that since there is a valid reference to `self`,
		// SAFETY: the value has valid pointer semantics.
		unsafe { self.value.get().write_volatile(value) }
	}
}
