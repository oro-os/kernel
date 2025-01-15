//! Implements version 0 of the thread control interface.

use oro_mem::alloc::sync::Arc;
use oro_sync::ReentrantMutex;
use oro_sysabi::syscall::Error;

use super::KernelInterface;
use crate::{
	arch::Arch,
	interface::{InterfaceResponse, SystemCallResponse},
	thread::Thread,
};

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct ThreadV0;

impl KernelInterface for ThreadV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::THREAD_V0;

	fn get<A: Arch>(
		_thread: &Arc<ReentrantMutex<Thread<A>>>,
		_index: u64,
		_key: u64,
	) -> InterfaceResponse {
		InterfaceResponse::Immediate(SystemCallResponse {
			error: Error::NotImplemented,
			ret:   0,
		})
	}

	fn set<A: Arch>(
		_thread: &oro_mem::alloc::sync::Arc<oro_sync::ReentrantMutex<crate::thread::Thread<A>>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::Immediate(SystemCallResponse {
			error: Error::NotImplemented,
			ret:   0,
		})
	}
}
