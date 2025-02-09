//! Interface-related macros.

/// Resolves the target thread from the given index,
/// checking that the caller has permission to access it.
#[macro_export]
macro_rules! iface_resolve_thread_target {
	($A:ty, $thread:expr, $index:expr) => {{
		let thread = $thread;
		let index = $index;
		if index == 0 || index == thread.id() {
			thread.clone()
		} else {
			match $crate::tab::get().lookup::<$crate::thread::Thread<$A>>(index) {
				Some(t) => {
					if t.with(|t| t.ring().id()) != thread.with(|t| t.ring().id()) {
						return $crate::syscall::InterfaceResponse::immediate(
							::oro::syscall::Error::BadIndex,
							0,
						);
					}

					t
				}
				None => {
					return $crate::syscall::InterfaceResponse::immediate(
						::oro::syscall::Error::BadIndex,
						0,
					);
				}
			}
		}
	}};
}
