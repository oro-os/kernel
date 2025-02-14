//! Implements the address space layout query interface (version 0).

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};
use oro_mem::mapper::{AddressSpace as _, AddressSegment};

use super::KernelInterface;
use crate::{AddressSpace, arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread};

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct AddrLayoutV0<A: Arch>(pub(crate) PhantomData<A>);

impl<A: Arch> KernelInterface<A> for AddrLayoutV0<A> {
	fn get(&self, _thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let segment = match index {
			key!("module") => AddressSpace::<A>::user_data(),
			key!("thrdstck") => AddressSpace::<A>::user_thread_stack(),
			_ => return InterfaceResponse::immediate(SysError::BadIndex, 0),
		};

		::oro_macro::assert::fits_within::<usize, u64>();

		match key {
			key!("start") => InterfaceResponse::ok(segment.range().0 as u64),
			key!("end") => InterfaceResponse::ok(segment.range().1 as u64),
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
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
