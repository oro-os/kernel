//! Macros for working with the system ABI.

/// Auxiliary types used by macros exported by this crate.
///
/// **Using this module directly is highly discouraged. It is not stable.**
pub mod private {
	/// Indicates to the kernel that an `(interface_id, key)` is to be used
	/// via system calls.
	#[must_use]
	pub const fn uses(iface: u64, key: u64) -> [u64; 4] {
		[crate::id::meta::USES, 2, iface, key]
	}

	/// Indicates to the kernel that it should allocate a slot for importing
	/// an interface, anonymously.
	#[must_use]
	pub const fn interface_slot(iface: u64) -> ([u64; 4], usize) {
		([crate::id::meta::IFACE_SLOT, 2, 0, iface], 2)
	}

	/// Indicates to the kernel that it should allocate a slot for importing
	/// an interface, with a key.
	#[must_use]
	pub const fn interface_slot_key(iface: u64, key: u64) -> ([u64; 5], usize) {
		([crate::id::meta::IFACE_SLOT, 3, 0, iface, key], 2)
	}

	/// Stub for the `must_use` attribute. This is identical to [`core::hint::must_use`].
	#[expect(clippy::inline_always)]
	#[must_use]
	#[inline(always)]
	pub const fn must_use<T>(e: T) -> T {
		e
	}
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

/// Declares that an `(interface_id, key)` is to be used
/// by the current function.
///
/// This macro is meant to be used **once** directly before
/// any system call callsites.
///
/// # Linker Section
/// Note that this macro generates a static array that is placed
/// into `.oro.uses` linker section. If not using the `oro` crate
/// to build the module, you'll need to ensure that this section
/// is included in the linker script.
#[macro_export]
macro_rules! uses {
	($iface:expr, $key:expr) => {{
		const LEN: usize = $crate::macros::private::uses(const { $iface }, const { $key }).len();
		#[link_section = ".oro.uses"]
		static USES: [u64; LEN] = $crate::macros::private::uses(const { $iface }, const { $key });

		::core::hint::black_box(USES);
	}};
}

/// Declares that an interface slot is to be allocated for the module.
///
/// Should be used once per slot; **these are not idempotent like `uses!`**.
///
/// Can optionally take a key, which will be used to identify the slot to
/// the user; otherwise, the slot receives an anonymous (numeric) key.
///
/// # Linker Section
/// Note that this macro generates a static array that is placed
/// into `.oro.uses` linker section. If not using the `oro` crate
/// to build the module, you'll need to ensure that this section
/// is included in the linker script.
#[macro_export]
macro_rules! interface_slot {
	($iface:expr) => {
		const {
			const SLOT_AND_IDX: ([u64; 4], usize) =
				$crate::macros::private::interface_slot(const { $iface });
			const SLOT_IDX: usize = SLOT_AND_IDX.1;
			#[link_section = ".oro.uses"]
			static SLOT: [u64; SLOT_AND_IDX.0.len()] = SLOT_AND_IDX.0;
			$crate::macros::private::must_use(&SLOT[SLOT_IDX])
		}
	};
	($iface:expr, $key:expr) => {
		const {
			const SLOT_AND_IDX: ([u64; 5], usize) =
				$crate::macros::private::interface_slot_key(const { $iface }, const { $key });
			const SLOT_IDX: usize = SLOT_AND_IDX.1;
			#[link_section = ".oro.uses"]
			static SLOT: [u64; SLOT_AND_IDX.0.len()] = SLOT_AND_IDX.0;
			$crate::macros::private::must_use(&SLOT[SLOT_IDX])
		}
	};
}

/// Performs a `GET` system call to the kernel, automatically emitting
/// a [`uses!`] call for the `(interface_id, key)` pair.
///
/// Requires that the arguments to `interface_id` and `key` are constants.
/// If you need to use runtime values, use [`crate::syscall::get`] instead,
/// and manually call [`uses!`] before the call.
#[macro_export]
macro_rules! syscall_get {
	($interface_id:expr, $interface_handle:expr, $index:expr, $key:expr $(,)?) => {{
		$crate::uses!($interface_id, $key);
		$crate::syscall::get($interface_handle, $index, $key)
	}};
}

/// Performs a `SET` system call to the kernel, automatically emitting
/// a [`uses!`] call for the `(interface_id, key)` pair.
///
/// Requires that the arguments to `interface_id` and `key` are constants.
/// If you need to use runtime values, use [`crate::syscall::set`] instead,
/// and manually call [`uses!`] before the call.
#[macro_export]
macro_rules! syscall_set {
	($interface_id:expr, $interface_handle:expr, $index:expr, $key:expr, $value:expr $(,)?) => {{
		$crate::uses!($interface_id, $key);
		$crate::syscall::set($interface_handle, $index, $key, $value)
	}};
}
