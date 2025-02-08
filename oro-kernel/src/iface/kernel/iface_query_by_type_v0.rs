//! Kernel interface for querying the ring's interfaces
//! based on the interface type.

use core::marker::PhantomData;

use oro::syscall::Error as SysError;

use super::KernelInterface;
use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread};

/// Version 0 of interface ID query by type kernel interface.
#[repr(transparent)]
pub struct IfaceQueryByTypeV0<A: Arch>(pub(crate) PhantomData<A>);

impl<A: Arch> KernelInterface<A> for IfaceQueryByTypeV0<A> {
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let ring = thread.with(|t| t.ring());

		ring.with(|ring| {
			let interfaces = ring.interfaces_by_type();

			if let Some(iface_list) = interfaces.get(index) {
				if let Some(iface) = iface_list.get(key as usize) {
					InterfaceResponse::ok(iface.id())
				} else {
					InterfaceResponse::immediate(SysError::BadKey, 0)
				}
			} else {
				InterfaceResponse::immediate(SysError::BadIndex, 0)
			}
		})
	}

	fn set(
		&self,
		_thread: &Tab<Thread<A>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::immediate(SysError::ReadOnly, 0)
	}
}
