//! Implements the core-local volatile type.
#![allow(clippy::inline_always)]

use core::cell::UnsafeCell;

/// A core-local volatile value.
///
/// # Safety
/// This structure is NOT multicore-safe. It is intended to be used
/// in a core-local context only.
///
/// It is inner-mutable, whereby reads and writes are volatile
/// but the write method is marked as immutable. It's intended
/// to safely work around some of the Rust mutability concerns,
/// in a bit of a black-magic way.
///
/// **Using this for values that require a destructor is UB.**
///
/// **Using this for values that are shared between cores is UB.**
///
/// **Using this for values outside of the core-local supervisor
/// address space is UB.**
///
/// It's really only meant to be used by the [`crate::local`] module.
#[repr(transparent)]
pub struct Volatile<T: Copy + Sized>(UnsafeCell<T>);

impl<T: Copy + Sized> Volatile<T> {
	/// Creates a new volatile wrapper.
	///
	/// # Safety
	/// See the safety documentation for the struct.
	///
	/// **Do not use this type outside of the core-local supervisor
	/// address space state structure. It is _very, very, VERY_
	/// unsafe and any improper use WILL INCUR IMMEDIATE UB.**
	#[inline(always)]
	pub const unsafe fn new(inner: T) -> Self {
		Self(UnsafeCell::new(inner))
	}

	/// Gets the inner value.
	#[inline(always)]
	pub fn read(&self) -> T {
		// SAFETY(qix-): We own the value, there's no way to violate guarantees.
		unsafe { core::ptr::read_volatile(self.0.get()) }
	}

	/// Sets the inner value.
	#[inline(always)]
	pub fn write(&self, value: T) {
		// SAFETY(qix-): We own the value, there's no way to violate guarantees.
		unsafe {
			core::ptr::write_volatile(self.0.get(), value);
		}
	}
}
