//! Provides functionality for wrapping physical addresses
//! to be translated into virtual addresses.

use core::mem::MaybeUninit;

/// A physical address. The Oro kernel represents these as
/// 64-bit unsigned integers under the hood, regardless of
/// the underlying architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Phys(u64);

impl PhysAddr for Phys {
	#[inline(always)]
	fn address_u64(&self) -> u64 {
		self.0
	}

	#[inline(always)]
	unsafe fn from_address_unchecked(address: u64) -> Self {
		Self(address)
	}
}

/// A trait for working with physical addresses and translating them
/// into virtual addresses, references, etc.
pub trait PhysAddr: Sized {
	/// Returns the physical address as a u64.
	fn address_u64(&self) -> u64;

	/// Creates a new instance of the address type from the given address.
	///
	/// # Safety
	/// The caller **must** ensure the passed address is valid and allocated.
	///
	/// Further, caller must be aware of the on-drop semantics and alignment
	/// requirements of the backing type.
	unsafe fn from_address_unchecked(address: u64) -> Self;

	/// Returns the translated virtual address as a `usize`.
	#[inline(always)]
	fn virt(&self) -> usize {
		crate::translator().translate(self.address_u64())
	}

	/// Returns a virtual pointer to the physical address
	/// as the given type. Does not check alignment.
	#[inline(always)]
	unsafe fn as_ptr_unchecked<T>(&self) -> *const T {
		crate::translator().translate(self.address_u64()) as *const T
	}

	/// Returns a mutable virtual pointer to the physical address
	/// as the given type. Does not check alignment.
	///
	/// # Safety
	/// The caller **must** ensure the pointer is properly aligned
	/// before dereferencing it.
	///
	/// Further, the caller **must** ensure the pointer is not
	/// used to create multiple mutable references to the same
	/// data in a way that would violate Rust's aliasing rules.
	#[inline(always)]
	unsafe fn as_mut_ptr_unchecked<T>(&self) -> *mut T {
		crate::translator().translate(self.address_u64()) as *mut T
	}

	/// Returns a virtual pointer to the physical address as the given type.
	///
	/// Returns `None` if the pointer would not be properly aligned.
	#[inline(always)]
	fn as_ptr<T>(&self) -> Option<*const T> {
		let ptr = unsafe { self.as_ptr_unchecked::<T>() };
		if ptr.is_aligned() { Some(ptr) } else { None }
	}

	/// Returns a mutable virtual pointer to the physical address as the given type.
	///
	/// Returns `None` if the pointer would not be properly aligned.
	#[inline(always)]
	fn as_mut_ptr<T>(&self) -> Option<*mut T> {
		let ptr = unsafe { self.as_mut_ptr_unchecked::<T>() };
		if ptr.is_aligned() { Some(ptr) } else { None }
	}

	/// Returns a [`MaybeUninit`] reference to the physical address.
	///
	/// # Safety
	/// Pointer alignment is not checked; caller must ensure
	/// the pointer is properly aligned before dereferencing it.
	#[inline(always)]
	unsafe fn as_maybe_uninit_unchecked<T>(&self) -> &MaybeUninit<T> {
		&*(self.as_ptr_unchecked::<T>() as *const MaybeUninit<T>)
	}

	/// Returns a mutable [`MaybeUninit`] reference to the physical address.
	///
	/// # Safety
	/// Pointer alignment is not checked; caller must ensure
	/// the pointer is properly aligned before dereferencing it.
	///
	/// Further, caller must ensure that multiple mutable references
	/// are not created to the same data in a way that would violate
	/// Rust's aliasing rules.
	#[inline(always)]
	unsafe fn as_maybe_uninit_mut_unchecked<T>(&self) -> &mut MaybeUninit<T> {
		&mut *(self.as_mut_ptr_unchecked::<T>() as *mut MaybeUninit<T>)
	}

	/// Returns a reference to the given type.
	///
	/// Equivalent of `self.as_maybe_uninit_unchecked().assume_init_ref()`.
	///
	/// # Safety
	/// The caller **must** ensure the address is properly aligned
	/// to the type and that the data is initialized.
	#[inline(always)]
	unsafe fn as_ref_unchecked<T>(&self) -> &'static T {
		&*self.as_ptr_unchecked()
	}

	/// Returns a mutable reference to the given type.
	///
	/// Equivalent of `self.as_maybe_uninit_mut_unchecked().assume_init_mut()`.
	///
	/// # Safety
	/// The caller **must** ensure the address is properly aligned
	/// to the type and that the data is initialized.
	///
	/// Further, the caller **must** ensure the reference is not
	/// used to create multiple mutable references to the same
	/// data in a way that would violate Rust's aliasing rules.
	#[inline(always)]
	unsafe fn as_mut_unchecked<T>(&self) -> &'static mut T {
		&mut *self.as_mut_ptr_unchecked()
	}

	/// Returns a reference to the given type.
	///
	/// Returns `None` if the pointer would not be properly aligned.
	///
	/// # Safety
	/// The caller **must** ensure the data is initialized.
	#[inline(always)]
	fn as_ref<T>(&self) -> Option<&'static T> {
		self.as_ptr::<T>().map(|ptr| unsafe { &*ptr })
	}

	/// Returns a mutable reference to the given type.
	///
	/// Returns `None` if the pointer would not be properly aligned.
	///
	/// # Safety
	/// The caller **must** ensure the data is initialized.
	///
	/// Further, the caller **must** ensure the reference is not
	/// used to create multiple mutable references to the same
	/// data in a way that would violate Rust's aliasing rules.
	#[inline(always)]
	fn as_mut<T>(&self) -> Option<&'static mut T> {
		self.as_mut_ptr::<T>().map(|ptr| unsafe { &mut *ptr })
	}
}
