//! **Testing:** A testing interface to map in a test port between
//! two modules.
//!
//! > **IMPORTANT:** DO NOT USE THIS INTERFACE IN MODULE CODE.
//! > It exists temporarily to test ports.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};

use crate::{
	arch::Arch,
	interface::Interface,
	port::{PortEnd, PortState},
	syscall::InterfaceResponse,
	tab::Tab,
	thread::Thread,
};

/// Interface specific errors
#[repr(u64)]
pub enum Error {
	/// The endpoint is already claimed.
	Claimed     = key!("claimed"),
	/// The system is out of memory.
	OutOfMemory = key!("oom"),
}

/// Temporary interface for testing ports.
///
/// # Do not use!
/// Do not use this interface. It's just here to test ports.
pub struct RootTestPorts<A: Arch>(Tab<PortState>, PhantomData<A>);

impl<A: Arch> RootTestPorts<A> {
	/// Creates a new `RootTestPorts` instance.
	///
	/// # Panics
	/// Panics if the system is out of memory.
	#[must_use]
	pub fn new() -> Self {
		// NOTE(qix-): The field count here must match that in the examples (minus one).
		// NOTE(qix-): The test cases don't query the field count or anything.
		Self(PortState::new(1).expect("out of memory"), PhantomData)
	}
}

impl<A: Arch> Interface<A> for RootTestPorts<A> {
	fn type_id(&self) -> u64 {
		1_737_937_612_428
	}

	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		}

		match key {
			key!("health") => InterfaceResponse::ok(1337),
			key!("prodtkn") | key!("cnsmtkn") => {
				match PortState::endpoint(
					&self.0,
					if key == key!("prodtkn") {
						PortEnd::Producer
					} else {
						PortEnd::Consumer
					},
				)
				.map(|r| {
					r.map_err(|_| {
						InterfaceResponse::immediate(
							SysError::InterfaceError,
							Error::Claimed as u64,
						)
					})
				})
				.ok_or(InterfaceResponse::immediate(
					SysError::InterfaceError,
					Error::OutOfMemory as u64,
				))
				.flatten()
				.map(|tkn| {
					let id = tkn.id();
					thread.with_mut(|t| t.insert_token(tkn));
					InterfaceResponse::ok(id)
				}) {
					Ok(r) => r,
					Err(e) => e,
				}
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}

	fn set(
		&self,
		_thread: &Tab<Thread<A>>,
		index: u64,
		key: u64,
		_value: u64,
	) -> InterfaceResponse {
		if index != 0 {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		}

		match key {
			key!("health") | key!("prodtkn") | key!("cnsmtkn") => {
				InterfaceResponse::immediate(SysError::ReadOnly, 0)
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
