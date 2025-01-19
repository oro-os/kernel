//! Kernel level (`id <= 0xFFFF_FFFF`) interfaces.
//!
//! Always available, regardless of the caller's ring.

use crate::{arch::Arch, interface::InterfaceResponse, tab::Tab, thread::Thread};

mod thread_v0;

/// Small helper trait for kernel interfaces, which are always
/// available, use no state themselves, and are simple gateways
/// to other internal kernel structures.
pub trait KernelInterface {
	/// The stable kernel interface ID for this interface.
	const TYPE_ID: u64;

	/// See [`crate::interface::Interface::get`].
	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse;

	/// See [`crate::interface::Interface::set`].
	fn set<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64)
	-> InterfaceResponse;
}

#[doc(hidden)]
macro_rules! make_dispatch {
	($($iface:ty),*) => {
		const _: () = const {
			$(assert!(
				(<$iface as KernelInterface>::TYPE_ID & ::oro_sysabi::id::mask::KERNEL_ID) == 0,
				concat!("kernel interface specifies non-kernel ID: ", stringify!($ty))
			);)*
		};

		/// Attempts to dispatch a [`oro_sysabi::syscall::Opcode::Get`] system call to a kernel interface.
		///
		/// If the type ID is not recognized, returns `None`; callers should delegate
		/// to a registry lookup and dispatch.
		#[must_use]
		pub fn try_dispatch_get<A: Arch>(
			thread: &Tab<Thread<A>>,
			type_id: u64,
			index: u64,
			key: u64,
		) -> Option<InterfaceResponse> {
			match type_id {
				$(
					<$iface as KernelInterface>::TYPE_ID => {
						Some(<$iface as KernelInterface>::get::<A>(thread, index, key))
					}
				)*
				_ => None,
			}
		}

		/// Attempts to dispatch a [`oro_sysabi::syscall::Opcode::Set`] system call to a kernel interface.
		///
		/// If the type ID is not recognized, returns `None`; callers should delegate
		/// to a registry lookup and dispatch.
		#[must_use]
		pub fn try_dispatch_set<A: Arch>(
			thread: &Tab<Thread<A>>,
			type_id: u64,
			index: u64,
			key: u64,
			value: u64,
		) -> Option<InterfaceResponse> {
			match type_id {
				$(
					<$iface as KernelInterface>::TYPE_ID => {
						Some(<$iface as KernelInterface>::set::<A>(thread, index, key, value))
					}
				)*
				_ => None,
			}
		}
	};
}

make_dispatch! {
	thread_v0::ThreadV0
}
