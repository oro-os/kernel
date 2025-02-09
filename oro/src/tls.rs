//! Thread local storage facilities.
#![deny(unsafe_op_in_unsafe_fn)]

use core::ptr::NonNull;

/// The "Key" type for thread-local storage slots.
pub type Key = usize;

/// The error returned by TLS functions in this module.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
	/// The platform does not support thread-local storage.
	Unsupported,
	/// The base pointer is larger than a `u64`.
	PointerTooLarge,
	/// A syscall `(error, extended_err)` occurred
	/// when interacting with the kernel interface.
	Syscall(crate::syscall::Error, u64),
}

/// Sets the thread-local base pointer for the given thread ID.
///
/// The thread ID of `0` is the current thread.
///
/// # Performance
/// This function is not optimized for performance, but
/// instead for correctness and portability. Thus, it is
/// marked `#[cold]`. It is meant to be called primarily
/// once, at the start of a module instance's execution.
///
/// For cases whereby the base pointer is set frequently,
/// callers should eschew the use of this function and
/// default to using more direct, optimized, architecture-specific
/// mechanisms instead (where available).
///
/// # Architecture-specific
/// Some architecture-specific information (note that
/// the omission if a section here does not necessarily
/// preclude support for that architecture):
///
/// ## x86_64
/// On x86_64, the `fs` segment register is used to
/// access the thread-local storage base pointer.
///
/// The Oro kernel _also_ tracks `gsbase` and allows
/// for modification of that MSR via the same kernel
/// interface, but this function does not use it; only
/// `fsbase` is modified.
///
/// This function **does not** check if the target supports
/// `wrfsbase`, and always defers to the kernel interface
/// to set the base pointer. If that is undesirable, the
/// caller should opt to perform a `CPUID` check and manually
/// perform the `wrfsbase` instruction.
///
/// # Safety
/// This function is highly architecture-specific. The
/// implications of using this function are not well
/// defined, and using it generally requires careful
/// code generation considerations.
///
/// Further, `ptr` must be a valid pointer with all
/// of the alignment and access guarantees that are
/// specified by the architecture.
#[cold]
pub unsafe fn set_tls_base(thread_id: u64, ptr: NonNull<u8>) -> Result<(), Error> {
	let ptr: u64 = ptr
		.as_ptr()
		.addr()
		.try_into()
		.map_err(|_| Error::PointerTooLarge)?;

	#[cfg(target_arch = "x86_64")]
	{
		// SAFETY: This is safe because the kernel interface
		// SAFETY: should always exist, and the ramifications
		// SAFETY: of changing the FS base are offloaded to the
		// SAFETY: caller.
		unsafe {
			let r = crate::syscall_set!(
				crate::id::iface::KERNEL_X86_64_TLS_BASE_V0,
				crate::id::iface::KERNEL_X86_64_TLS_BASE_V0,
				thread_id,
				crate::key!("fsbase"),
				ptr
			);

			return match r {
				Ok(()) => Ok(()),
				Err((err, ext)) => Err(Error::Syscall(err, ext)),
			};
		}
	}

	// No architecture support.
	// TODO(qix-): Might be better to use `cfg_if!` here.
	#[allow(unreachable_code)]
	{
		let _ = (ptr, thread_id);
		Err(Error::Unsupported)
	}
}

/// Returns the thread-local base pointer for the given
/// thread ID.
///
/// The thread ID of `0` is the current thread.
///
/// # Architecture-specific
/// For architecture-specific information, see the
/// documentation for [`set_tls_base`].
///
/// # Safety
/// This function is highly architecture-specific. The
/// implications of using this function are not well
/// defined, and using it generally requires careful
/// code generation considerations.
///
/// Further, the returned pointer is not guaranteed to
/// be valid.
///
/// In the event that the kernel returns `0` for the value,
/// a value of [`core::ptr::null`] is returned.
pub unsafe fn tls_base(thread_id: u64) -> Result<*const u8, Error> {
	#[cfg(target_arch = "x86_64")]
	{
		// SAFETY: This is safe because the kernel interface
		// SAFETY: should always exist, and the ramifications
		// SAFETY: of changing the FS base are offloaded to the
		// SAFETY: caller.
		unsafe {
			let r = crate::syscall_get!(
				crate::id::iface::KERNEL_X86_64_TLS_BASE_V0,
				crate::id::iface::KERNEL_X86_64_TLS_BASE_V0,
				thread_id,
				crate::key!("fsbase")
			);

			return match r {
				Ok(ptr) => {
					Ok(if ptr == 0 {
						core::ptr::null()
					} else {
						ptr as *const u8
					})
				}
				Err((err, ext)) => Err(Error::Syscall(err, ext)),
			};
		}
	}

	// No architecture support.
	// TODO(qix-): Might be better to use `cfg_if!` here.
	#[allow(unreachable_code)]
	{
		let _ = thread_id;
		Err(Error::Unsupported)
	}
}
