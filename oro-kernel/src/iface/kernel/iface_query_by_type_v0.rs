//! Kernel interface for querying the ring's interfaces
//! based on the interface type.

use oro_sysabi::syscall::Error as SysError;

use super::KernelInterface;
use crate::{
	arch::Arch,
	syscall::{InterfaceResponse, SystemCallResponse},
	tab::Tab,
	thread::Thread,
};

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct IfaceQueryByTypeV0;

impl KernelInterface for IfaceQueryByTypeV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_IFACE_QUERY_BY_TYPE_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let ring = thread.with(|t| t.ring());

		ring.with(|ring| {
			let interfaces = ring.interfaces_by_type();

			if let Some(iface_list) = interfaces.get(index) {
				if key == 0 {
					InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::Ok,
						ret:   iface_list.len() as u64,
					})
				} else if let Some(iface) = iface_list.get(key as usize - 1) {
					InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::Ok,
						ret:   iface.id(),
					})
				} else {
					InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::BadKey,
						ret:   0,
					})
				}
			} else {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::BadIndex,
					ret:   0,
				})
			}
		})
	}

	fn set<A: Arch>(
		_thread: &Tab<Thread<A>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::Immediate(SystemCallResponse {
			error: SysError::ReadOnly,
			ret:   0,
		})
	}
}
