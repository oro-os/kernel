//! **Testing:** A testing interface to map in a test port between
//! two modules.
//!
//! > **IMPORTANT:** DO NOT USE THIS INTERFACE IN MODULE CODE.
//! > It exists temporarily to test ports.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};

use crate::{
	arch::Arch, interface::Interface, port::Port, syscall::InterfaceResponse, tab::Tab,
	thread::Thread,
};

/// Temporary interface for testing ports.
///
/// # Do not use!
/// Do not use this interface. It's just here to test ports.
pub struct RootTestPorts<A: Arch>(Tab<Port>, PhantomData<A>);

impl<A: Arch> RootTestPorts<A> {
	/// Creates a new `RootTestPorts` instance.
	///
	/// # Panics
	/// Panics if the system is out of memory.
	#[must_use]
	pub fn new() -> Self {
		Self(
			Port::new()
				.and_then(|p| crate::tab::get().add(p))
				.expect("out of memory"),
			PhantomData,
		)
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
			key!("prodtkn") => {
				// Mark it owned by the current thread's instead.
				let tkn = self.0.with(|p| p.producer());
				let instance = thread.with(|t| t.instance().clone());
				instance.with_mut(|i| i.insert_token(tkn.clone()));
				InterfaceResponse::ok(tkn.id())
			}
			key!("cnsmtkn") => {
				// Mark it owned by the current thread's instead.
				let tkn = self.0.with(|p| p.consumer());
				let instance = thread.with(|t| t.instance().clone());
				instance.with_mut(|i| i.insert_token(tkn.clone()));
				InterfaceResponse::ok(tkn.id())
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
