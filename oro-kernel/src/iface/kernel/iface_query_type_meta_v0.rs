//! Allows the querying of interface metdata given an interface type ID.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread};

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct IfaceQueryTypeMetaV0<A: Arch>(pub(crate) PhantomData<A>);

impl<A: Arch> KernelInterface<A> for IfaceQueryTypeMetaV0<A> {
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let ring = thread.with(|t| t.ring());

		ring.with(|ring| {
			let interfaces = ring.interfaces_by_type();

			if let Some(iface_list) = interfaces.get(index) {
				match key {
					k if k == key!("icount") => InterfaceResponse::ok(iface_list.len() as u64),
					_ => InterfaceResponse::immediate(SysError::BadKey, 0),
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
