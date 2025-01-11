//! Types and functionality for submitting Oro system calls.
#![expect(clippy::inline_always)]

/// Type alias for a syscall result that may return an [`Error`] code.
pub type Result<T> = core::result::Result<T, Error>;

/// Error codes returned by system calls.
///
/// These error codes are architecture-independent,
/// and are the same across all architectures.
///
/// # Non-exhaustive
/// For both backwards and forwards compatibility, this enum is marked as non-exhaustive.
/// This means that new error codes can be added in the future without breaking existing code,
/// and there is a greater chance that new code works on older versions of the kernel.
///
/// As such, match arms should always have a catch-all arm (`_ => { ... }`) to handle otherwise
/// unknown error codes.
///
/// # Error Permanence
/// Error codes are marked as either **temporary** or **permanent** (sometimes determined by the
/// table upon which is being operated).
///
/// Temporary errors may be resolved by retrying the operation at a later time, or by changing
/// the operation parameters.
///
/// Permanent errors are not expected to be resolved with the same parameters, possibly ever.
/// In some select cases (marked as such), the error may be resolved in a newer version of the kernel,
/// but application code should treat these errors as indicators of likely bugs or misconfigurations.
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
	/// The requested operation completed successfully. **Not an error.**
	///
	/// This error is **temporary**.
	Ok              = 0,
	/// The operation code is invalid.
	///
	/// This error is **permanent**.
	BadOpcode       = 1,
	/// The requested operation code is valid, but some portion of the operation
	/// is not implemented in the current version of the kernel (or environment).
	///
	/// This error is **permanent**.
	NotImplemented  = 2,
	/// The requested key does not exist, or is outside the bounds of the object
	/// (for list).
	///
	/// This error is **temporary**.
	BadKey          = 3,
	/// The given key is read-only and cannot be modified.
	///
	/// This error is **permanent**.
	ReadOnly        = 4,
	/// The object is not a list; the requested opcode cannot be performed on it.
	///
	/// This error is **permanent**.
	NotList         = 5,
	/// The requested type is mismatched for the given key.
	///
	/// This error is **permanent**.
	WrongType       = 6,
	/// The requested version mismatched.
	///
	/// This error is **permanent**.
	VersionMismatch = 7,
	/// The given handle is invalid.
	///
	/// This error is **temporary** (really, _permanent_ but technically _temporary_ since
	/// it's unlikely that a bad handle is passed, an open operation returns that exact handle,
	/// and passing the previously bad handle results in a system call that is meaningfully the same).
	BadHandle       = 8,
}

/// Each individual operation that can be performed by the Oro registry.
#[non_exhaustive]
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
	/// Opens an object for reading and writing.
	Open  = 0x8888_0000_0000_0000,
	/// Closes an object handle opened with [`Opcode::Open`].
	Close = 0x8888_0000_0000_0001,
	/// Gets a value.
	Get   = 0x8888_0000_0000_0002,
	/// Sets a value.
	Set   = 0x8888_0000_0000_0003,
}

/// Performs a branchless system call to get a registry value by key.
///
/// Use [`reg_get`] for a safer alternative.
///
/// Returns a tuple of the error code, the value, and the version.
/// The value and version are only valid if the error code is [`Error::Ok`].
///
/// # Safety
/// Any interpretation of the result value (first tuple value) is undefined
/// if the error is not [`Error::Ok`].
#[inline(always)]
#[must_use]
pub unsafe fn reg_get_raw(object_id: u64, key: u64, version: u64) -> (Error, u64, u64) {
	#[cfg(target_arch = "x86_64")]
	{
		crate::arch::x86_64::syscall(Opcode::Get, object_id, key, 0, version)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = object_id;
		let _ = key;
		let _ = version;
		(Error::NotImplemented, 0, 0)
	}
}

/// Performs a branchless system call to set a registry value by key.
///
/// Use [`reg_set`] for a safer alternative.
///
/// Returns a tuple of the error code and the new version.
/// The new version is only valid if the error code is [`Error::Ok`].
///
/// # Safety
/// The error code returned may be [`Error::Ok`] and should be checked.
#[inline(always)]
#[must_use]
pub unsafe fn reg_set_raw(object_id: u64, key: u64, value: u64, version: u64) -> (Error, u64) {
	#[cfg(target_arch = "x86_64")]
	{
		let (err, _, version) =
			crate::arch::x86_64::syscall(Opcode::Set, object_id, key, value, version);
		(err, version)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = object_id;
		let _ = key;
		let _ = version;
		let _ = value;
		(Error::NotImplemented, 0)
	}
}

/// Performs a branchless system call to open an object.
///
/// Use [`reg_open`] for a safer alternative.
///
/// Returns the object's handle.
///
/// # Safety
/// The returned handle is only valid if the error code is [`Error::Ok`].
#[inline(always)]
#[must_use]
pub unsafe fn reg_open_raw(object_id: u64, key: u64) -> (Error, u64) {
	#[cfg(target_arch = "x86_64")]
	{
		let (err, handle, _) = crate::arch::x86_64::syscall(Opcode::Open, object_id, key, 0, 0);
		(err, handle)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = object_id;
		let _ = key;
		(Error::NotImplemented, 0)
	}
}

/// Performs a branchless system call to close an object.
///
/// Use [`reg_close`] for a safer alternative.
///
/// # Safety
/// The error code returned may be [`Error::Ok`] and should be checked.
#[inline(always)]
#[must_use]
pub unsafe fn reg_close_raw(handle: u64) -> Error {
	#[cfg(target_arch = "x86_64")]
	{
		let (err, _, _) = crate::arch::x86_64::syscall(Opcode::Close, handle, 0, 0, 0);
		err
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = handle;
		Error::NotImplemented
	}
}

/// Gets a registry value by key.
///
/// Returns a tuple of the value and the version.
#[inline(always)]
pub fn reg_get(object_id: u64, key: u64, version: u64) -> Result<(u64, u64)> {
	let (err, value, version) = unsafe { reg_get_raw(object_id, key, version) };
	if err == Error::Ok {
		Ok((value, version))
	} else {
		Err(err)
	}
}

/// Sets a registry value by key.
///
/// Returns the new version of the value.
#[inline(always)]
pub fn reg_set(object_id: u64, key: u64, value: u64, version: u64) -> Result<u64> {
	let (err, version) = unsafe { reg_set_raw(object_id, key, value, version) };
	if err == Error::Ok {
		Ok(version)
	} else {
		Err(err)
	}
}

/// Opens an object for reading and writing.
///
/// Returns the object's handle.
///
/// Must be closed with [`reg_close`] when no longer needed.
#[inline(always)]
pub fn reg_open(object_id: u64, key: u64) -> Result<u64> {
	let (err, handle) = unsafe { reg_open_raw(object_id, key) };
	if err == Error::Ok {
		Ok(handle)
	} else {
		Err(err)
	}
}

/// Closes an object handle.
#[inline(always)]
pub fn reg_close(handle: u64) -> Result<()> {
	let err = unsafe { reg_close_raw(handle) };
	if err == Error::Ok { Ok(()) } else { Err(err) }
}

/// Converts a literal string into a 64-bit object key.
///
/// # Panics
/// Panics if the string is not 8 bytes or less.
#[macro_export]
macro_rules! key {
	($key:literal) => {
		const {
			const KEY_RAW: &str = $key;

			assert!(
				KEY_RAW.len() <= 8,
				concat!("object keys too long (must be <= 8 bytes): ", $key)
			);

			const KEY: &str = concat!($key, "\0\0\0\0\0\0\0\0");

			let bytes = KEY.as_bytes();

			((bytes[0] as u64) << 56)
				| ((bytes[1] as u64) << 48)
				| ((bytes[2] as u64) << 40)
				| ((bytes[3] as u64) << 32)
				| ((bytes[4] as u64) << 24)
				| ((bytes[5] as u64) << 16)
				| ((bytes[6] as u64) << 8)
				| (bytes[7] as u64)
		}
	};
}
