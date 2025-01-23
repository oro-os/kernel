//! Implements version 0 of the thread control interface.

use oro_sysabi::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{
	arch::Arch,
	syscall::InterfaceResponse,
	tab::Tab,
	thread::{ChangeStateError, RunState, Thread},
};

include!("macro/resolve_target.rs");

/// Error codes specific to the thread control interface.
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum Error {
	/// Invalid run state when setting `status`.
	InvalidState = key!("invlst"),
	/// Another thread is waiting to change the target thread's state; try again.
	Race         = key!("race"),
	/// Cannot change state; thread is terminated.
	Terminated   = key!("term"),
}

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct ThreadV0;

impl KernelInterface for ThreadV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_THREAD_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let target = resolve_target!(thread, index);

		match key {
			key!("id") => InterfaceResponse::ok(target.id()),
			key!("status") => InterfaceResponse::ok(target.with(|t| t.run_state()) as u64),
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}

	fn set<A: Arch>(
		thread: &Tab<Thread<A>>,
		index: u64,
		key: u64,
		value: u64,
	) -> InterfaceResponse {
		let target = resolve_target!(thread, index);

		match key {
			key!("id") => InterfaceResponse::immediate(SysError::ReadOnly, 0),
			key!("status") => {
				let Ok(new_state) = RunState::try_from(value) else {
					return InterfaceResponse::immediate(
						SysError::InterfaceError,
						Error::InvalidState as u64,
					);
				};

				match target.with_mut(|t| t.transition_to(thread.id(), new_state)) {
					Ok(None) => InterfaceResponse::ok(0),
					Ok(Some(transition)) => InterfaceResponse::Pending(transition),
					Err(e) => {
						InterfaceResponse::immediate(
							SysError::InterfaceError,
							match e {
								ChangeStateError::Race => Error::Race,
								ChangeStateError::Terminated => Error::Terminated,
							} as u64,
						)
					}
				}
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
