// NOTE(qix-): This is NOT a module. It's meant to be `include!`d in another module.

// Generates a dispatch table for kernel interfaces.
// Used only by `mod.rs` in the parent module; not meant to be used elsewhere.
#[doc(hidden)]
macro_rules! make_dispatch {
	($($iface:ty),* $(,)?) => {
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

