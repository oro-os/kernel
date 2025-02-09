//! x86_64 TLS base pointer (FS/GS) interface.

//! Kernel interface for querying the ring's interfaces
//! based on the interface type.

use crate::Arch as A;
use oro::{key, syscall::Error as SysError};
use oro_kernel::{syscall::InterfaceResponse, tab::Tab, thread::Thread, iface::kernel::KernelInterface};

/// Version 0 of TLS base (FS/GS) kernel interface for x86_64.
#[repr(transparent)]
pub struct TlsBaseV0;

impl KernelInterface<A> for TlsBaseV0 {
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let thread = oro_kernel::iface_resolve_thread_target!(crate::Arch, thread, index);

		match key {
			key!("fsbase") => {
				let v = thread.with(|ctx| ctx.handle().fsbase);
				InterfaceResponse::ok(v)
			}
			key!("gsbase") => {
				let v = thread.with(|ctx| ctx.handle().gsbase);
				InterfaceResponse::ok(v)
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}

	fn set(
		&self,
		thread: &Tab<Thread<A>>,
		index: u64,
		key: u64,
		value: u64,
	) -> InterfaceResponse {
		let thread = oro_kernel::iface_resolve_thread_target!(crate::Arch, thread, index);

		match key {
			key!("fsbase") => {
				// Set the FS base pointer. Will get picked up by the
				// next context switch.
				thread.with_mut(|ctx| {
					ctx.handle_mut().fsbase = value;
				});
				InterfaceResponse::ok(0)
			},
			key!("gsbase") => {
				// Set the GS base pointer. Will get picked up by the
				// next context switch.
				thread.with_mut(|ctx| {
					ctx.handle_mut().gsbase = value;
				});
				InterfaceResponse::ok(0)
			},
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
