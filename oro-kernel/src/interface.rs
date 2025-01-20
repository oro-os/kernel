//! Types and traits for Oro interfaces, exposed by modules and the kernel.

use core::marker::PhantomData;

use oro_mem::alloc::boxed::Box;
use oro_sysabi::syscall::Error as SysError;

use crate::{
	arch::Arch,
	syscall::{InterfaceResponse, SystemCallResponse},
	tab::Tab,
	thread::Thread,
};

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
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse;

	/// Handles a [`oro_sysabi::syscall::Opcode::Set`] system call request to this interface.
	///
	/// System call handling must be quick and non-blocking. Either it can be
	/// serviced immediately, or can be processed "offline", returning a handle
	/// that can be polled for completion.
	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse;
}

/// Implements a scoped interface wrapper, which is an [`Interface`] that is only accessible
/// within a specific ring.
pub struct RingInterface<A: Arch> {
	/// The interface.
	interface: Box<dyn Interface<A>>,
	/// The ring ID.
	ring_id:   u64,
	#[doc(hidden)]
	_arch:     PhantomData<A>,
}

impl<A: Arch> RingInterface<A> {
	/// Creates a new ring interface.
	pub fn new<I: Interface<A> + 'static>(interface: I, ring_id: u64) -> Self {
		Self {
			interface: Box::new(interface),
			ring_id,
			_arch: PhantomData,
		}
	}
}

impl<A: Arch> Interface<A> for RingInterface<A> {
	#[inline]
	fn type_id(&self) -> u64 {
		self.interface.type_id()
	}

	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let ring_id = thread
			.with(|t| t.instance().clone())
			.with(|i| i.ring().id());

		if ring_id != self.ring_id {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadInterface,
				ret:   0,
			});
		}

		self.interface.get(thread, index, key)
	}

	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse {
		let ring_id = thread.with(|t| t.ring()).id();

		if ring_id != self.ring_id {
			return InterfaceResponse::Immediate(SystemCallResponse {
				error: SysError::BadInterface,
				ret:   0,
			});
		}

		self.interface.set(thread, index, key, value)
	}
}
