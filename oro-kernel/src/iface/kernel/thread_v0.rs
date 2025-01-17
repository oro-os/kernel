//! Implements version 0 of the thread control interface.

use oro_sysabi::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{
	arch::Arch,
	syscall::{InterfaceResponse, SystemCallResponse},
	tab::Tab,
	thread::{ChangeStateError, RunState, Thread},
};

/// Error codes specific to the thread control interface.
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum Error {
	/// Invalid run state when setting `status`.
	InvalidState = 1,
	/// Another thread is waiting to change the target thread's state; try again.
	Race         = 2,
	/// Cannot change state; thread is terminated.
	Terminated   = 3,
}

/// Version 0 of the thread control kernel interface.
#[repr(transparent)]
pub struct ThreadV0;

/// Resolves the target thread from the given index,
/// checking that the caller has permission to access it.
macro_rules! resolve_target {
	($thread:expr, $index:expr) => {{
		let thread = $thread;
		let index = $index;
		if index == 0 || index == thread.id() {
			thread.clone()
		} else {
			match crate::tab::get().lookup::<Thread<A>>(index) {
				Some(t) => {
					if t.with(|t| t.ring().id()) != thread.with(|t| t.ring().id()) {
						return InterfaceResponse::Immediate(SystemCallResponse {
							error: SysError::BadIndex,
							ret:   0,
						});
					}

					t
				}
				None => {
					return InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::BadIndex,
						ret:   0,
					});
				}
			}
		}
	}};
}

impl KernelInterface for ThreadV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_THREAD_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let target = resolve_target!(thread, index);

		match key {
			key!("id") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   target.id(),
				})
			}
			key!("status") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::Ok,
					ret:   target.with(|t| t.run_state()) as u64,
				})
			}
			_ => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::BadKey,
					ret:   0,
				})
			}
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
			key!("id") => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::ReadOnly,
					ret:   0,
				})
			}
			key!("status") => {
				let Ok(new_state) = RunState::try_from(value) else {
					return InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::InterfaceError,
						ret:   Error::InvalidState as u64,
					});
				};

				match target.with_mut(|t| t.transition_to(thread.id(), new_state)) {
					Ok(None) => {
						InterfaceResponse::Immediate(SystemCallResponse {
							error: SysError::Ok,
							ret:   0,
						})
					}
					Ok(Some(transition)) => InterfaceResponse::Pending(transition),
					Err(e) => {
						InterfaceResponse::Immediate(SystemCallResponse {
							error: SysError::InterfaceError,
							ret:   match e {
								ChangeStateError::Race => Error::Race,
								ChangeStateError::Terminated => Error::Terminated,
							} as u64,
						})
					}
				}
			}
			_ => {
				InterfaceResponse::Immediate(SystemCallResponse {
					error: SysError::BadKey,
					ret:   0,
				})
			}
		}
	}
}
