//! Types and traits for Oro interfaces, exposed by modules and the kernel.

use core::{
	cell::UnsafeCell,
	marker::PhantomData,
	mem::MaybeUninit,
	sync::atomic::{
		AtomicU8,
		Ordering::{Acquire, Relaxed, Release},
	},
};

use oro_mem::alloc::sync::Arc;
use oro_sync::{Lock, ReentrantMutex};
use oro_sysabi::syscall::Error as SysError;

use crate::{arch::Arch, thread::Thread};

/// Implements an interface, which is a flat namespace of `(index, flay_keyvals)`.
///
/// Typically, these are interacted with via system calls. Modules (and the kernel)
/// can consume or provide interfaces to the ring in order for applications to
/// interact with them.
///
/// Interfaces can be backed by a number of mechanisms; notably, the kernel implements
/// them as direct interfaces to kernel data structures and machinery. They can also
/// be implemented by userspace applications, which can then be interacted with by
/// system calls or ports (or perhaps other interfaces).
pub trait Interface<A: Arch>: Send + Sync {
	/// Returns the type ID of the interface. Note that IDs with all high 32 bits
	/// cleared are reserved for kernel usage.
	fn type_id(&self) -> u64;

	/// Handles a [`oro_sysabi::syscall::Opcode::Get`] system call request to this interface.
	///
	/// System call handling must be quick and non-blocking. Either it can be
	/// serviced immediately, or can be processed "offline", returning a handle
	/// that can be polled for completion.
	fn get(
		&self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		index: u64,
		key: u64,
	) -> InterfaceResponse;

	/// Handles a [`oro_sysabi::syscall::Opcode::Set`] system call request to this interface.
	///
	/// System call handling must be quick and non-blocking. Either it can be
	/// serviced immediately, or can be processed "offline", returning a handle
	/// that can be polled for completion.
	fn set(
		&self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		index: u64,
		key: u64,
		value: u64,
	) -> InterfaceResponse;
}

/// Implements a scoped interface wrapper, which is an [`Interface`] that is only accessible
/// within a specific ring.
pub struct RingInterface<A: Arch, I: Interface<A>> {
	/// The interface.
	interface: I,
	/// The ring ID.
	ring_id:   u64,
	#[doc(hidden)]
	_arch:     PhantomData<A>,
}

impl<A: Arch, I: Interface<A>> RingInterface<A, I> {
	/// Creates a new ring interface.
	pub fn new(interface: I, ring_id: u64) -> Self {
		Self {
			interface,
			ring_id,
			_arch: PhantomData,
		}
	}
}

impl<A: Arch, I: Interface<A>> Interface<A> for RingInterface<A, I> {
	#[inline]
	fn type_id(&self) -> u64 {
		self.interface.type_id()
	}

	fn get(
		&self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		index: u64,
		key: u64,
	) -> InterfaceResponse {
		let ring_id = {
			let thread_lock = thread.lock();
			let instance = thread_lock.instance();
			let instance_lock = instance.lock();
			let ring = instance_lock.ring();
			if let Some(ring) = ring.upgrade() {
				ring.lock().id()
			} else {
				0
			}
		};

		if ring_id != self.ring_id {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadInterface,
				ret:   0,
			});
		}

		self.interface.get(thread, index, key)
	}

	fn set(
		&self,
		thread: &Arc<ReentrantMutex<Thread<A>>>,
		index: u64,
		key: u64,
		value: u64,
	) -> InterfaceResponse {
		let ring_id = {
			let thread_lock = thread.lock();
			let instance = thread_lock.instance();
			let instance_lock = instance.lock();
			let ring = instance_lock.ring();
			if let Some(ring) = ring.upgrade() {
				ring.lock().id()
			} else {
				0
			}
		};

		if ring_id != self.ring_id {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadInterface,
				ret:   0,
			});
		}

		self.interface.set(thread, index, key, value)
	}
}

/// Response from an interface after handling a system call.
///
/// When performing an [`Interface`] syscall operation, the interface can either
/// respond immediately, or defer the response to a later time. In the latter case,
/// a pollable handle is returned.
pub enum InterfaceResponse {
	/// The interface has handled the request and the response is ready.
	Immediate(SystemCallResponse),
	/// The interface has received the request, but the response is not ready yet.
	Pending(InFlightSystemCallHandle),
}

/// The producer side of an in-flight system call.
///
/// Interfaces should hold onto this handle when they defer a system call response,
/// allowing to submit the response at a later time.
#[repr(transparent)]
pub struct InFlightSystemCall(Arc<InFlightSystemCallInner>);

/// Inner shared state for an in-flight system call.
struct InFlightSystemCallInner {
	/// The response is ready. Used as an atomic rather than an `Option`.
	state:    AtomicU8,
	/// The response data; only valid if `state` is [`InFlightState::Ready`].
	response: UnsafeCell<MaybeUninit<SystemCallResponse>>,
}

// SAFETY: We strictly control access to the inner state and control exactly
// SAFETY: how it is used, and can guarantee that it is safe to share across
// SAFETY: threads. Any misbehavior is a bug.
unsafe impl Sync for InFlightSystemCallInner {}
// SAFETY: The inner state is valid across threads, despite using an unsafe cell.
unsafe impl Send for InFlightSystemCallInner {}

/// Indicates the state of the in-flight system call.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InFlightState {
	/// Pending
	Pending,
	/// The response is ready.
	Ready,
	/// The caller has canceled the system call.
	CallerCanceled,
	/// The interface has canceled the system call.
	InterfaceCanceled,
	/// The interface serviced the response, and the caller has
	/// already taken the response. This is considered an error.
	Finished,
}

/// The caller side of an in-flight system call.
pub struct InFlightSystemCallHandle(Arc<InFlightSystemCallInner>);

impl InFlightSystemCall {
	/// Creates a new in-flight system call interface/caller pair.
	///
	/// The interface should call this, hold on to the first element
	/// (the interface side), and return the second element (the caller side)
	/// via [`InterfaceResponse::Pending`].
	///
	/// Once the interface has a response, it can submit it via [`Self::submit`],
	/// which consumes this handle and notifies the caller.
	#[must_use]
	pub fn new() -> (Self, InFlightSystemCallHandle) {
		let inner = Arc::new(InFlightSystemCallInner {
			state:    AtomicU8::new(InFlightState::Pending as u8),
			response: UnsafeCell::new(MaybeUninit::uninit()),
		});

		(Self(inner.clone()), InFlightSystemCallHandle(inner))
	}

	/// Checks if the caller canceled the system call.
	///
	/// There's no need to check this before submitting a response; however,
	/// if the caller is doing something long-running, it may be useful to
	/// check, as if this returns `true`, the response will be dropped immediately.
	#[inline]
	#[must_use]
	pub fn canceled(&self) -> bool {
		self.0.state.load(Relaxed) == (InFlightState::CallerCanceled as u8)
	}

	/// Submits a response to the in-flight system call, consuming the
	/// interface side of the handle.
	///
	/// **Note:** there is no guarantee that the response will be taken by the
	/// caller; if the caller canceled the system call, the response will be
	/// dropped. Further, the caller may cancel the system call after this
	/// method is called, but before the response is taken.
	///
	/// If the interface's processing of this sytem call request is long-winded,
	/// and side effects are not a concern, it may be useful to check if the
	/// system call was canceled before submitting the response via [`Self::canceled`].
	pub fn submit(self, response: SystemCallResponse) {
		// SAFETY: We are the only ones that can write to the response,
		// SAFETY: and it can only happen here, once.
		unsafe { self.0.response.get().write(MaybeUninit::new(response)) };
		// NOTE(qix-): Even if the caller canceled the system call, this is
		// NOTE(qix-): innocuous; it's going to get dropped anyway, and we
		// NOTE(qix-): save a branch.
		self.0.state.store(InFlightState::Ready as u8, Release);
	}
}

impl Drop for InFlightSystemCall {
	fn drop(&mut self) {
		self.0
			.state
			.compare_exchange(
				InFlightState::Pending as u8,
				InFlightState::InterfaceCanceled as u8,
				Relaxed,
				Relaxed,
			)
			.ok();
	}
}

impl InFlightSystemCallHandle {
	/// Get the current state of the in-flight system call.
	#[inline]
	#[must_use]
	pub fn state(&self) -> InFlightState {
		// SAFETY: We control the value entirely; we can transmute it back.
		unsafe { ::core::mem::transmute::<u8, InFlightState>(self.0.state.load(Acquire)) }
	}

	/// Try to take the response from the in-flight system call.
	///
	/// - If the response is ready, it is returned as `Ok(Some(response))`.
	/// - If the response is not ready, `Ok(None)` is returned.
	/// - If the system call was canceled by the interface, `Err(InFlightState::InterfaceCanceled)`
	///   is returned.
	/// - Attempts to take the response after it has been taken by the caller will return
	///   `Err(InFlightState::Finished)`.
	#[inline]
	pub fn try_take_response(&self) -> Result<Option<SystemCallResponse>, InFlightState> {
		match self.state() {
			InFlightState::Pending => Ok(None),
			InFlightState::CallerCanceled => unreachable!(),
			InFlightState::Ready => {
				// SAFETY: We are the only ones that can write to the response,
				// SAFETY: and it can only happen once.
				self.0.state.store(InFlightState::Finished as u8, Release);
				Ok(Some(unsafe { self.0.response.get().read().assume_init() }))
			}
			status @ (InFlightState::Finished | InFlightState::InterfaceCanceled) => Err(status),
		}
	}
}

impl Drop for InFlightSystemCallHandle {
	fn drop(&mut self) {
		self.0
			.state
			.compare_exchange(
				InFlightState::Pending as u8,
				InFlightState::CallerCanceled as u8,
				Relaxed,
				Relaxed,
			)
			.ok();
	}
}

/// System call request data.
#[derive(Debug, Clone)]
pub struct SystemCallRequest {
	/// The opcode.
	pub opcode: oro_sysabi::syscall::Opcode,
	/// The first argument.
	pub arg1:   u64,
	/// The second argument.
	pub arg2:   u64,
	/// The third argument.
	pub arg3:   u64,
	/// The fourth argument.
	pub arg4:   u64,
}

/// System call response data.
#[derive(Debug, Clone, Copy)]
pub struct SystemCallResponse {
	/// The error code.
	pub error: oro_sysabi::syscall::Error,
	/// The return value.
	pub ret:   u64,
}
