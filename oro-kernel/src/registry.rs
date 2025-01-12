//! Oro kernel object registry implementation.
#![expect(clippy::inline_always)]

use oro_mem::alloc::sync::Arc;
use oro_sync::{Lock, ReentrantMutex};
use oro_sysabi::{
	key,
	syscall::{Error, Opcode, Result},
};
use stash::Stash;

use crate::{
	arch::Arch,
	scheduler::{SystemCallAction, SystemCallRequest, SystemCallResponse},
	thread::Thread,
};

/// The root registry object for the Oro kernel.
///
/// This structure manages the open object handles and is typically
/// scoped to a module instance.
#[derive(Default)]
pub struct Registry {
	/// The list of open object handles.
	// TODO(qix-): This stash is a stop-gap solution for now. The conversion to/from `usize` is a hack
	// TODO(qix-): and will be remedied in the future - namely, to prevent re-use of handles.
	handles: Stash<Arc<ReentrantMutex<dyn Object>>, Tag>,
}

impl Registry {
	/// Looks up a key in the root of the registry (which is a "logical" object, not backed by a handle).
	fn get_root_key<A: Arch>(
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		key: u64,
	) -> Result<Arc<ReentrantMutex<dyn Object>>> {
		match key {
			key!("thread") => Ok(RootThreadObject::from_thread(thread.clone())),
			_ => Err(Error::BadKey),
		}
	}

	/// Attempts to service an [`Opcode::Open`] system call.
	fn try_open<A: Arch>(
		&mut self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		handle: u64,
		key: u64,
	) -> Result<u64> {
		let object = if handle == 0 {
			Self::get_root_key(thread, key)?
		} else {
			self.handles
			 	// SAFETY: We have already checked if handle == 0.
				.get(unsafe { handle.unchecked_sub(1) }.into())
				.ok_or(Error::BadHandle)?
				.lock()
				.try_get_object(key)?
		};

		let handle: u64 = self.handles.put(object).into();

		// SAFETY(qix-): We add one here to ensure that the handle is not 0.
		// SAFETY(qix-): Technically, if you exhaust the entirety of a 64-bit handle space, you will
		// SAFETY(qix-): cause UB. I'm choosing not to worry about this. If you exhaust the handle space,
		// SAFETY(qix-): you have bigger problems.
		Ok(unsafe { handle.unchecked_add(1) })
	}

	/// Handles a system call request dispatched, typically, by the scheduler.
	///
	/// This function operates in the context of a thread, taking a _reference_
	/// to an `Arc`-wrapped thread handle in case a clone is _not_ needed (e.g.
	/// in cases where early-stage validation fails).
	pub fn dispatch_system_call<A: Arch>(
		&mut self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		request: &SystemCallRequest,
	) -> SystemCallAction {
		match request.opcode {
			Opcode::Open => {
				match self.try_open(thread, request.arg1, request.arg2) {
					Ok(handle) => {
						SystemCallAction::RespondImmediate(SystemCallResponse {
							error: Error::Ok,
							ret1:  handle,
							ret2:  0,
						})
					}
					Err(error) => {
						SystemCallAction::RespondImmediate(SystemCallResponse {
							error,
							ret1: 0,
							ret2: 0,
						})
					}
				}
			}
			_ => {
				SystemCallAction::RespondImmediate(SystemCallResponse {
					error: Error::BadOpcode,
					ret1:  0,
					ret2:  0,
				})
			}
		}
	}
}

/// Represents an object in the object registry.
trait Object {
	/// Attempts to retrieve an object from the registry given its
	/// parent and a key.
	fn try_get_object(&self, key: u64) -> Result<Arc<ReentrantMutex<dyn Object>>>;
}

/// Represents the root `thread` object in the object registry. Contextualized around a given thread.
struct RootThreadObject<A: Arch> {
	/// The context thread handle.
	self_thread: Arc<ReentrantMutex<Thread<A>>>,
}

impl<A: Arch> RootThreadObject<A> {
	/// Creates a new `RootThreadObject` contextualized around the given thread handle.
	fn from_thread(self_thread: Arc<ReentrantMutex<Thread<A>>>) -> Arc<ReentrantMutex<dyn Object>> {
		Arc::new(ReentrantMutex::new(Self { self_thread }))
	}
}

impl<A: Arch> Object for RootThreadObject<A> {
	fn try_get_object(&self, key: u64) -> Result<Arc<ReentrantMutex<dyn Object>>> {
		match key {
			key!("self") => Ok(ThreadObject::from_thread(self.self_thread.clone())),
			_ => Err(Error::BadKey),
		}
	}
}

/// Wraps a [`Thread`] for interaction via the object registry.
struct ThreadObject<A: Arch> {
	/// The target thread handle.
	#[expect(dead_code)]
	thread: Arc<ReentrantMutex<Thread<A>>>,
}

impl<A: Arch> ThreadObject<A> {
	/// Creates a new `ThreadObject` from a thread handle.
	fn from_thread(thread: Arc<ReentrantMutex<Thread<A>>>) -> Arc<ReentrantMutex<dyn Object>> {
		Arc::new(ReentrantMutex::new(Self { thread }))
	}
}

impl<A: Arch> Object for ThreadObject<A> {
	fn try_get_object(&self, key: u64) -> Result<Arc<ReentrantMutex<dyn Object>>> {
		#[expect(clippy::match_single_binding)]
		match key {
			_ => Err(Error::BadKey),
		}
	}
}

/// Inner type for the handle map keys.
///
/// Stop-gap solution for now; conversion to/from `usize` is a hack and will be remedied in the future.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct Tag(u64);

impl From<usize> for Tag {
	#[inline(always)]
	fn from(val: usize) -> Self {
		::oro_macro::assert::size_eq::<usize, u64>();
		Self(val as u64)
	}
}

impl From<Tag> for usize {
	#[inline(always)]
	fn from(val: Tag) -> Self {
		::oro_macro::assert::size_eq::<usize, u64>();
		val.0 as usize
	}
}

impl From<u64> for Tag {
	#[inline(always)]
	fn from(val: u64) -> Self {
		Self(val)
	}
}

impl From<Tag> for u64 {
	#[inline(always)]
	fn from(val: Tag) -> Self {
		val.0
	}
}
