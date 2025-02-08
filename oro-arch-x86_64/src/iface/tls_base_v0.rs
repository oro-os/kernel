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
		match key {
			key!("name") => {
				match index {
					0 => InterfaceResponse::ok(key!("fs")),
					1 => InterfaceResponse::ok(key!("gs")),
					_ => InterfaceResponse::immediate(SysError::BadIndex, 0),
				}
			},
			key!("base") => {
				match index {
					0 => {
						debug_assert_eq!(crate::asm::get_fs_msr(), thread.with(|ctx| ctx.handle().fsbase));
						InterfaceResponse::ok(crate::asm::get_fs_msr())
					}
					1 => {
						debug_assert_eq!(crate::asm::get_gs_msr(), thread.with(|ctx| ctx.handle().gsbase));
						InterfaceResponse::ok(crate::asm::get_gs_msr())
					}
					_ => InterfaceResponse::immediate(SysError::BadIndex, 0),
				}
			},
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
		match key {
			key!("name") => InterfaceResponse::immediate(SysError::ReadOnly, 0),
			key!("base") => {
				match index {
					0 => {
						// Set the FS base pointer. Will get picked up by the
						// next context switch.
						thread.with_mut(|ctx| {
							ctx.handle_mut().fsbase = value;
						});
					},
					1 => {
						// Set the GS base pointer. Will get picked up by the
						// next context switch.
						thread.with_mut(|ctx| {
							ctx.handle_mut().gsbase = value;
						});
					},
					_ => return InterfaceResponse::immediate(SysError::BadIndex, 0),
				}

				InterfaceResponse::ok(0)
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
