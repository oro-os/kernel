//! Implements version 0 of the thread control interface.

use oro_sync::Lock;
use oro_sysabi::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{
	arch::Arch,
	interface::{InterfaceResponse, SystemCallResponse},
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

/// Given a thread, index and key, checks to see if the index (thread ID)
/// is `0` (indicating 'self') or another thread, attempts to look up the
/// thread ID in the instance's thread table, and then executes the given
/// match block with the target thread (either self or the found thread).
macro_rules! with_thread_id {
	($thread:expr, $index:expr, ($source_id:ident, $thr_target:ident) for match $key:ident { $($tt:tt)* }) => {
		if $index == 0 {
			{
				$thread.with_mut(|$thr_target| {
					let $source_id = $thr_target.id();
					match $key {
						$( $tt )*
					}
				})
			}
		} else {
			$thread.with(|thread_lock| {
				let instance = thread_lock.instance();
				let instance_lock = instance.lock();
				let threads = instance_lock.threads();
				if let Some(other_thread) = threads.get($index) {
					{
						let $source_id = thread_lock.id();
						other_thread.with_mut(|$thr_target| {
							match $key {
								$( $tt )*
							}
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
	};
}

impl KernelInterface for ThreadV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_THREAD_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		with_thread_id!(thread, index, (_caller_id, target) for match key {
			key!("id") => InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::Ok,
				ret:   target.id(),
			}),
			key!("status") => InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::Ok,
				ret: target.run_state() as u64,
			}),
			_ => InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadKey,
				ret:   0,
			}),
		})
	}

	fn set<A: Arch>(
		thread: &Tab<Thread<A>>,
		index: u64,
		key: u64,
		value: u64,
	) -> InterfaceResponse {
		with_thread_id!(thread, index, (caller_id, target) for match key {
			key!("id") => InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::ReadOnly,
				ret:   0,
			}),
			key!("status") => {
				let Ok(new_state) = RunState::try_from(value) else {
					return InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::InterfaceError,
						ret:   Error::InvalidState as u64,
					});
				};

				match target.transition_to(caller_id, new_state) {
					Ok(None) => InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::Ok,
						ret:   0,
					}),
					Ok(Some(transition)) => InterfaceResponse::Pending(transition),
					Err(e) => InterfaceResponse::Immediate(SystemCallResponse {
						error: SysError::InterfaceError,
						ret:   match e {
							ChangeStateError::Race => Error::Race,
							ChangeStateError::Terminated => Error::Terminated,
						} as u64,
					}),
				}
			},
			_ => InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadKey,
				ret:   0,
			}),
		})
	}
}
