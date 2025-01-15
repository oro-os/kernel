//! High-level runtime support for Oro modules.

pub use ::oro_sysabi as sysabi;

#[cfg(feature = "panic_handler")]
#[panic_handler]
#[doc(hidden)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
	// TODO(qix-): Send panic information somewhere.
	unsafe { terminate() }
}

extern "C" {
	fn main();
}

#[doc(hidden)]
#[no_mangle]
extern "C" fn _oro_start() -> ! {
	unsafe {
		main();
		terminate()
	}
}

/// Terminates the current thread.
///
/// If the current thread is the last thread in module instance, the module
/// instance is unmounted from all rings and subsequently destroyed.
///
/// If the module instance was the last on a given ring, the ring is also destroyed.
///
/// Note that this does not guarantee that references to the module instance are invalidated;
/// in some cases the kernel may still respond to operations pertaining to this module instance,
/// invoked by other module instances that were previously interacting with this module instance,
/// in order to allow them to gracefully handle the termination of this module instance.
///
/// # Thread Cleanup
/// All resources allocated by the thread are freed. This includes, but is not limited to, memory
/// allocations and any ports.
///
/// Any ports that applications wish to continue using must be explicitly transferred to another
/// thread or module instance prior to calling this function.
///
/// # Safety
/// This function is inherently unsafe as it immediately terminates the current thread.
pub unsafe fn terminate() -> ! {
	use crate::sysabi::{key, syscall as s};

	// SAFETY: MUST NOT PANIC.
	let _ = s::set_raw(sysabi::id::iface::THREAD_V0, 0, key!("kill"), 1);

	force_crash()
}

/// Attempts to crash the application.
///
/// Used in the event that a typical, "supposed to invariably work" operation
/// invariably doesn't work (such as terminating the program).
///
/// This should be a minefield of different ways to try to crash the application
/// in increasingly more aggressive ways, finally resulting in a spin loop (zombie
/// state) if all else fails.
///
/// # Safety
/// This function is inherently unsafe as it attempts to crash the application.
///
/// Do not call unless you intend to... crash the application.
pub unsafe fn force_crash() -> ! {
	// NOTE(qix-): UNDER NO CIRCUMSTANCE SHOULD THIS FUNCTION PERFORM A MEMORY WRITE OPERATION.
	// NOTE(qix-): FURTHER, DO NOT PANIC AS IT WILL LOOP INDEFINITELY.

	// NOTE(qix-): Do not try the null-pointer trick on any architecture
	// NOTE(qix-): as on some chips 0x0 is a valid address, might be MMIO'd,
	// NOTE(qix-): and could even incur external side effects. The chance of this
	// NOTE(qix-): is almost literally zero given how the kernel is designed
	// NOTE(qix-): but I want to remain defensive on this front.

	// (x86_64) Try to use a 'sane' undefined handler.
	#[cfg(target_arch = "x86_64")]
	{
		core::arch::asm!("ud2");
	}

	// (aarch64) Try to use a 'sane' undefined handler.
	#[cfg(target_arch = "aarch64")]
	{
		core::arch::asm!("udf #0");
	}

	// (x86_64) Try to read from cr3.
	#[cfg(target_arch = "x86_64")]
	{
		core::arch::asm!("mov rax, cr3");
	}

	// (aarch64) Try to read from spsr_el1.
	#[cfg(target_arch = "aarch64")]
	{
		core::arch::asm!("mrs x0, spsr_el1");
	}

	// Otherwise, spin loop. We should never get here.
	loop {
		core::hint::spin_loop();
	}
}

/// IDs used throughout the Oro kernel.
pub mod id {
	/// Kernel IDs; re-exported from [`oro_sysabi::id`].
	pub use ::oro_sysabi::id as kernel;

	/// Common non-kernel IDs.
	///
	/// > **NOTE:** This is not a comprehensive list of all IDs in the Oro ecosystem,
	/// > but - notably - the IDs used by the standard library (`oro-std` or, in the future,
	/// > `std`). They can be used directly or by using non-`core` types provided by the `[oro-]std` module.
	pub mod common {
		// TODO(qix-): Intentionally empty for now.
	}
}

/// Syscall helper macros.
///
/// These are just re-exports from [`oro_sysabi::macros`].
pub mod syscall {
	pub use ::oro_sysabi::{interface_slot, key, syscall_get as get, syscall_set as set, uses};
}
