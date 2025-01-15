//! Types and functionality for submitting Oro system calls.
#![expect(clippy::inline_always)]

/// Type alias for a syscall result that may return an [`Error`] code.
///
/// In some cases, extended error information is returned via the second value
/// (namely, interface-specific errors).
pub type Result<T> = core::result::Result<T, (Error, u64)>;

/// Error codes returned by system calls.
///
/// These error codes are architecture-independent (except for the case
/// of arch-dependent interfaces via [`Error::InterfaceError`]) and are
/// otherwise the same across all architectures.
///
/// # Non-exhaustive
/// For both backwards and forwards compatibility, this enum is marked as non-exhaustive.
/// This means that new error codes can be added in the future without breaking existing code,
/// and there is a greater chance that new code works on older versions of the kernel.
///
/// As such, match arms should always have a catch-all arm (`_ => { ... }`) to handle otherwise
/// unknown error codes.
///
/// # Error Precedence
/// There is no stability or guarantee on the precedence of error codes. Applications should
/// make no assumptions or inferrences about the parameters or context based on which error code
/// is returned first. **The kernel reserves the right to return any error code out of a set
/// of possible error codes - even at random.**
///
/// The **only** guarantee is that the kernel will return an error code related to the operation
/// of the Oro registry before any other table-related error codes.
///
/// It is, of course, guaranteed that [`Error::Ok`] will be returned for successful operations.
#[non_exhaustive]
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
	// NOTE(qix-): Maintainers, please DO NOT replace or re-use error codes,
	// NOTE(qix-): ever. Further, mind bit 31 (it should always be zero for these
	// NOTE(qix-): error codes).
	// NOTE(qix-):
	// NOTE(qix-): Removed error codes should be marked as deprecated and kept
	// NOTE(qix-): around until it's certain that no syscalls would return them.
	/// The requested operation completed successfully. **Not an error.** The returned
	/// value(s), if any, are successful response data from the operation.
	Ok             = 0,
	/// The operation code is invalid.
	BadOpcode      = 1,
	/// The requested operation code is valid, but some portion of the operation
	/// is not implemented in the current version of the kernel (or environment).
	NotImplemented = 2,
	/// The requested interface ID does not exist, or is not available in the caller's
	/// context.
	BadInterface   = 3,
	/// The requested key does not exist, or is outside the bounds of the object
	/// (for list).
	BadKey         = 4,
	/// The requested index does not exist.
	BadIndex       = 5,
	/// The given key is read-only and cannot be modified.
	ReadOnly       = 6,
	/// The interface returned an interface-specific error; check the `value`
	/// code for that error.
	InterfaceError = 0xFFFF_FFFF_FFFF_FFFF,
}

/// Each individual operation that can be performed by the Oro registry.
#[non_exhaustive]
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
	/// Gets a value.
	Get = 0x8888_0000_0000_0001,
	/// Sets a value.
	Set = 0x8888_0000_0000_0002,
}

/// Performs a branchless system call to get a registry value by key.
///
/// Use [`get`] for a safer alternative.
///
/// Returns a tuple of the error code and the value.
///
/// # Safety
/// The interpretation of the result value (second tuple value) is dependent upon
/// the `Error` code returned. If the error code is [`Error::Ok`], the value returned
/// is the returned value from the registry.
///
/// Otherwise, the value is `0` for any `Error` other than [`Error::InterfaceError`],
/// in which case the value's meaning holds the error passed back by the interface;
/// consult the interface's documentation for the meaning of that error code.
#[inline(always)]
#[must_use]
pub unsafe fn get_raw(interface_id: u64, index: u64, key: u64) -> (Error, u64) {
	#[cfg(target_arch = "x86_64")]
	{
		crate::arch::x86_64::syscall(Opcode::Get, interface_id, index, key, 0)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = interface_id;
		let _ = index;
		let _ = key;
		(Error::NotImplemented, 0)
	}
}

/// Performs a branchless system call to set a registry value by key.
///
/// Use [`set`] for a safer alternative.
///
/// Returns a tuple of the error code and the new version.
/// The new version is only valid if the error code is [`Error::Ok`].
///
/// # Safety
/// The interpretation of the result value (second tuple value) is dependent upon
/// the `Error` code returned. If the error code is [`Error::Ok`], the value returned
/// is `0`.
///
/// Otherwise, the value is `0` for any `Error` other than [`Error::InterfaceError`],
/// in which case the value's meaning holds the error passed back by the interface;
/// consult the interface's documentation for the meaning of that error code.
#[inline(always)]
#[must_use]
pub unsafe fn set_raw(interface_id: u64, index: u64, key: u64, value: u64) -> (Error, u64) {
	#[cfg(target_arch = "x86_64")]
	{
		crate::arch::x86_64::syscall(Opcode::Set, interface_id, index, key, value)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = interface_id;
		let _ = index;
		let _ = key;
		let _ = value;
		(Error::NotImplemented, 0)
	}
}

/// Gets a registry value by key.
///
/// Returns the value.
#[inline(always)]
pub fn get(interface_id: u64, index: u64, key: u64) -> Result<u64> {
	let (err, value) = unsafe { get_raw(interface_id, index, key) };
	if err == Error::Ok {
		Ok(value)
	} else {
		Err((err, value))
	}
}

/// Sets a registry value by key.
#[inline(always)]
pub fn set(interface_id: u64, index: u64, key: u64, value: u64) -> Result<()> {
	let (err, value) = unsafe { set_raw(interface_id, index, key, value) };
	if err == Error::Ok {
		Ok(())
	} else {
		Err((err, value))
	}
}
