//! Allocates memory page regions and returns tokens.
//!
//! **Important note**: Allocations **do not** immediately
//! allocate physical memory in all cases, and this behavior
//! will certainly differ between architectures and even configurations.
//! Instead, they reserve a region whereby first access will trigger
//! the actual allocation. This is to allow for more efficient
//! memory management and to avoid unnecessary allocations.
//!
//! Userspace applications should not rely on the behavior of
//! allocations, and should instead treat them as opaque tokens.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{
	arch::Arch,
	syscall::InterfaceResponse,
	tab::Tab,
	thread::Thread,
	token::{NormalToken, Token},
};

/// Error type for the interface.
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum Error {
	/// Out of memory.
	OutOfMemory  = key!("oom"),
	/// Zero size allocation.
	ZeroSize     = key!("zero"),
	/// Too many pages
	TooManyPages = key!("toomany"),
}

/// Version 0 of the page allocation interface.
#[repr(transparent)]
pub struct PageAllocV0<A: Arch>(pub(crate) PhantomData<A>);

impl<A: Arch> KernelInterface<A> for PageAllocV0<A> {
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		if key == 0 {
			return InterfaceResponse::immediate(SysError::InterfaceError, Error::ZeroSize as u64);
		}

		match key {
			key!("4kib") => {
				usize::try_from(index).map_or(
					InterfaceResponse::immediate(
						SysError::InterfaceError,
						Error::TooManyPages as u64,
					),
					|page_count| {
						crate::tab::get()
							.add(Token::Normal(NormalToken::new_4kib(page_count)))
							.map_or_else(
								|| {
									InterfaceResponse::immediate(
										SysError::InterfaceError,
										Error::OutOfMemory as u64,
									)
								},
								|tab| {
									InterfaceResponse::ok(thread.with_mut(|t| t.insert_token(tab)))
								},
							)
					},
				)
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}

	fn set(
		&self,
		_thread: &Tab<Thread<A>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::immediate(SysError::ReadOnly, 0)
	}
}
