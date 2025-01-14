//! Macros for working with the system ABI.

/// Auxiliary types used by macros exported by this crate.
///
/// **Using this module directly is highly discouraged. It is not stable.**
pub mod private {}

/// Declares that an `(interface_id, key)` is to be used
/// by the current function.
#[macro_export]
macro_rules! uses {
	($iface:expr, $key:expr) => {
		todo!();
	};
}
