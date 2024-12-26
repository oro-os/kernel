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
/// Further, the error code may pertain to a specific table. Those codes are not listed here.
/// See _Bit Representation_ for more information.
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
/// # Bit Representation
/// Unlike system call parameters, error codes are standardized
/// as 32-bit unsigned integers. On 64-bit architectures, the upper
/// 32-bits are reserved and should be assumed zero (the kernel will
/// always return zero in the upper 32-bits).
///
/// Note that "reserved" here means that, in the future, the kernel
/// reserves the right to begin using the upper 32-bits for additional
/// information or error codes, at which time a means for retrieving those
/// bits on 32-bit architectures will be provided.
///
/// Further, the high bit - bit 31 - is used to indicate whether the error
/// belongs to the Oro registry (as in, an error occurred while issuing a
/// syscall) or to the given table (as in, a table-specific error occurred).
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
///
/// The error code system does not provide warnings or informational messages.
#[non_exhaustive]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
	// NOTE(qix-): Maintainers, please DO NOT replace or re-use error codes,
	// NOTE(qix-): ever. Further, mind bit 31 (it should always be zero for these
	// NOTE(qix-): error codes).
	// NOTE(qix-):
	// NOTE(qix-): Removed error codes should be marked as deprecated and kept
	// NOTE(qix-): around until it's certain that no syscalls would return them.
	/// The requested operation completed successfully.
	///
	/// This error is **temporary**.
	Ok             = 0,
	/// The operation code is invalid.
	///
	/// This error is **permanent**.
	Invalid        = 1,
	/// The requested operation code is valid, but some portion of the operation
	/// is not implemented in the current version of the kernel (or environment).
	///
	/// This error is **permanent**.
	NotImplemented = 2,
	/// No such entity exists.
	///
	/// Note that this error code may be returned for valid entity identifiers
	/// in cases where the requesting context does not have permission to access
	/// the target entity or its context.
	///
	/// This error is **temporary**.
	BadEntity      = 3,
	/// The requested table does not exist.
	///
	/// This error is **permanent**.
	BadTable       = 4,
	/// The requested key does not exist, or is outside the bounds of the table
	/// (for list-like tables).
	///
	/// The permanence of this error depends on the table.
	///
	/// - For fixed-key tables, this error is **permanent**.
	/// - For list-like tables, this error is **temporary**.
	/// - For dynamic tables, this error is **temporary**.
	BadKey         = 5,
	/// The given key is read-only and cannot be modified.
	///
	/// This may apply only to certain keys in a table, or to the entire table.
	/// The error code makes no distinction between these cases.
	///
	/// This error is **permanent**.
	ReadOnly       = 6,
	/// The given key is write-only and cannot be read.
	///
	/// This may apply only to certain keys in a table, or to the entire table.
	/// The error code makes no distinction between these cases.
	///
	/// This error is **permanent**.
	WriteOnly      = 7,
	/// The table is not a list-like or dynamic table, and cannot have its length
	/// queried.
	///
	/// This error is **permanent**.
	NoLength       = 8,
	/// The given table is a fixed-key table and cannot have its elements removed.
	///
	/// This error is **permanent**.
	NoDelete       = 9,
}

/// Each individual operation that can be performed by the Oro registry.
#[non_exhaustive]
#[repr(u64)]
pub enum Opcode {
	/// Get a registry value by key.
	RegGet = 0,
	/// Set a registry value by key.
	RegSet = 1,
}

/// Performs a branchless system call to get a registry value by key.
///
/// Use [`reg_get`] for a safer alternative.
///
/// # Safety
/// Any interpretation of the result value (first tuple value) is undefined
/// if the error is not [`Error::Ok`].
#[inline(always)]
#[must_use]
pub unsafe fn reg_get_raw(entity: u64, table: u64, key: u64) -> (u64, Error) {
	#[cfg(target_arch = "x86_64")]
	{
		crate::arch::x86_64::syscall_reg_get_raw(entity, table, key)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = entity;
		let _ = table;
		let _ = key;
		(0, Error::NotImplemented)
	}
}

/// Performs a branchless system call to set a registry value by key.
///
/// Use [`reg_set`] for a safer alternative.
///
/// # Safety
/// The error code returned may be [`Error::Ok`] and should be checked.
#[inline(always)]
#[must_use]
pub unsafe fn reg_set_raw(entity: u64, table: u64, key: u64, value: u64) -> Error {
	#[cfg(target_arch = "x86_64")]
	{
		crate::arch::x86_64::syscall_reg_set_raw(entity, table, key, value)
	}

	#[cfg(not(target_arch = "x86_64"))]
	{
		// NOTE(qix-): Avoids unused variable warnings.
		let _ = entity;
		let _ = table;
		let _ = key;
		let _ = value;
		Error::NotImplemented
	}
}

/// Gets a registry value by key.
#[inline(always)]
pub fn reg_get(entity: u64, table: u64, key: u64) -> Result<u64> {
	let (ret, err) = unsafe { reg_get_raw(entity, table, key) };
	if err == Error::Ok { Ok(ret) } else { Err(err) }
}

/// Sets a registry value by key.
#[inline(always)]
pub fn reg_set(entity: u64, table: u64, key: u64, value: u64) -> Result<()> {
	let err = unsafe { reg_set_raw(entity, table, key, value) };
	if err == Error::Ok { Ok(()) } else { Err(err) }
}
