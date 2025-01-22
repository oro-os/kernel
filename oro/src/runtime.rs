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

	// SAFETY(qix-): MUST NOT PANIC.
	let _ = s::set_raw(
		sysabi::id::iface::KERNEL_THREAD_V0,
		0, // self
		key!("status"),
		key!("term"), // (not a key, but a value)
	);

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
}

/// Syscall low-level ABI elements.
///
/// These are mostly just re-exports from [`oro_sysabi::macros`], some with
/// more ergonomic names.
pub mod syscall {
	pub use ::oro_sysabi::{
		interface_slot, key, syscall::Error, syscall_get as get, syscall_set as set, uses,
	};
}

/// Common root ring interfaces.
pub mod root_ring {
	/// Debug output (version 0) interface abstraction.
	pub mod debug_out_v0 {
		use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

		use crate::{id, syscall};

		/// The `KERNEL_DEBUG_OUT_V0` interface ID, or `0` if it's not
		/// been resolved.
		static DEBUG_OUT_V0_ID: AtomicU64 = AtomicU64::new(0);

		/// Returns the `KERNEL_DEBUG_OUT_V0` interface ID, resolving
		/// it if necessary.
		///
		/// Returns `None` if the interface could not be resolved.
		pub fn id() -> Option<u64> {
			let id = DEBUG_OUT_V0_ID.load(Relaxed);
			if id == 0 {
				let Ok(iface) = syscall::get!(
					id::kernel::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
					id::kernel::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0,
					id::kernel::iface::ROOT_DEBUG_OUT_V0,
					0
				) else {
					DEBUG_OUT_V0_ID.store(!0, Relaxed);
					return None;
				};

				DEBUG_OUT_V0_ID.store(iface, Relaxed);
				Some(iface)
			} else if id == !0 {
				// Positive failure cache; if there's no root ring
				// debug interface, there won't be one in the future
				// (at least, we assume).
				None
			} else {
				Some(id)
			}
		}

		/// Writes a byte slice to the debug output.
		///
		/// Note that this function behaves in a somewhat unusual way:
		///
		/// - There is no implicit encoding, though UTF-8 should be used
		///   where possible.
		/// - Newlines (`\n`, `0x0A`) are treated as line breaks, and flush
		///   the buffer.
		/// - The interface is **not thread safe**; multiple threads writing
		///   to the debug output interface will result in interleaved output,
		///   potentially flushing the buffer at unexpected times.
		/// - The buffer has a per-line minimum and maximum before a line
		///   is force-flushed.
		/// - There is no `flush` command; simply send a newline to flush.
		/// - Data is sent up in 8-byte increments over synchronous syscalls.
		pub fn write_bytes(bytes: &[u8]) {
			if bytes.is_empty() {
				return;
			}

			let Some(iface) = id() else {
				return;
			};

			for chunk in bytes.chunks(8) {
				let mut word = 0u64;
				for b in chunk {
					word = (word << 8) | u64::from(*b);
				}

				syscall::set!(
					id::kernel::iface::ROOT_DEBUG_OUT_V0,
					iface,
					0,
					syscall::key!("write"),
					word
				)
				.unwrap();
			}
		}

		/// A [`core::fmt::Write`] implementation for the debug output interface.
		///
		/// This is a unit struct; it requires no initialization.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		pub struct DebugV0Write;

		impl ::core::fmt::Write for DebugV0Write {
			fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
				write_bytes(s.as_bytes());
				Ok(())
			}
		}

		/// `println!()` macro that prints to the root ring debug output (version 0)
		/// interface.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		#[macro_export]
		macro_rules! debug_v0_println {
			($($arg:tt)*) => {
				{
					use $crate::root_ring::debug_out_v0::DebugV0Write;
					use ::core::fmt::Write;
					let _ = ::core::writeln!(&mut DebugV0Write, $($arg)*);
				}
			}
		}

		/// `print!()` macro that prints to the root ring debug output (version 0)
		/// interface.
		///
		/// **Important:** Failures are _silent_; if no root ring debug output
		/// exists, the data will be silently dropped.
		#[macro_export]
		macro_rules! debug_v0_print {
			($($arg:tt)*) => {
				{
					use $crate::root_ring::debug_out_v0::DebugV0Write;
					use ::core::fmt::Write;
					let _ = ::core::write!(&mut DebugV0Write, $($arg)*);
				}
			}
		}
	}
}
