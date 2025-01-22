//! Allows the querying of interface metdata given an interface type ID.

use oro_sysabi::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread};

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct IfaceQueryTypeMetaV0;

impl KernelInterface for IfaceQueryTypeMetaV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_IFACE_QUERY_TYPE_META_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let ring = thread.with(|t| t.ring());

		ring.with(|ring| {
			let interfaces = ring.interfaces_by_type();

			if let Some(iface_list) = interfaces.get(index) {
				match key {
					key!("icount") => InterfaceResponse::ok(iface_list.len() as u64),
					_ => InterfaceResponse::immediate(SysError::BadKey, 0),
				}
			} else {
				InterfaceResponse::immediate(SysError::BadIndex, 0)
			}
		})
	}

	fn set<A: Arch>(
		_thread: &Tab<Thread<A>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::immediate(SysError::ReadOnly, 0)
	}
}
