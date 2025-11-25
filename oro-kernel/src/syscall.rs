//! Dispatch point for servicing system calls.

use core::{
	cell::UnsafeCell,
	mem::MaybeUninit,
	sync::atomic::{
		AtomicU8,
		Ordering::{Acquire, Relaxed, Release},
	},
};

use oro::syscall::{Error, Opcode};
use oro_kernel_mem::alloc::sync::Arc;

use crate::{
	arch::Arch,
	event::{SystemCallRequest, SystemCallResponse},
	interface::{Interface, RingInterface},
	tab::Tab,
	thread::Thread,
};

/// Dispatches a syscall on behalf of a thread.
#[must_use]
pub fn dispatch<A: Arch>(
	thread: &Tab<Thread<A>>,
	request: &SystemCallRequest,
) -> InterfaceResponse {
	let error = match request.opcode {
		x if x == const { Opcode::Get as u64 } => {
			if (request.arg1 & oro::id::mask::KERNEL_ID) == 0 {
				if let Some(res) = crate::iface::kernel::try_dispatch_get::<A>(
					thread,
					request.arg1,
					request.arg2,
					request.arg3,
				) {
					return res;
				}

				Error::BadInterface
			} else {
				match crate::tab::get().lookup::<RingInterface<A>>(request.arg1) {
					Some(interface) => {
						return interface.with(|i| i.get(thread, request.arg2, request.arg3));
					}
					None => Error::BadInterface,
				}
			}
		}
		x if x == const { Opcode::Set as u64 } => {
			if (request.arg1 & oro::id::mask::KERNEL_ID) == 0 {
				if let Some(res) = crate::iface::kernel::try_dispatch_set::<A>(
					thread,
					request.arg1,
					request.arg2,
					request.arg3,
					request.arg4,
				) {
					return res;
				}

				Error::BadInterface
			} else {
				match crate::tab::get().lookup::<RingInterface<A>>(request.arg1) {
					Some(interface) => {
						return interface
							.with(|i| i.set(thread, request.arg2, request.arg3, request.arg4));
					}
					None => Error::BadInterface,
				}
			}
		}
		_ => Error::BadOpcode,
	};

	InterfaceResponse::immediate(error, 0)
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

impl InterfaceResponse {
	/// Convience function for creating an immediate response.
	#[inline]
	#[must_use]
	pub const fn immediate(error: Error, ret: u64) -> Self {
		Self::Immediate(SystemCallResponse { error, ret })
	}

	/// Convence function for creating an [`Error::Ok`] immediate response.
	#[inline]
	#[must_use]
	pub const fn ok(ret: u64) -> Self {
		Self::immediate(Error::Ok, ret)
	}
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
