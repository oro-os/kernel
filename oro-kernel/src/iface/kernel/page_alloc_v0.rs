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

/// Version 0 of the memory token query interface.
#[repr(transparent)]
pub struct PageAllocV0;

impl KernelInterface for PageAllocV0 {
	const TYPE_ID: u64 = oro::id::iface::KERNEL_PAGE_ALLOC_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		if key == 0 {
			return InterfaceResponse::immediate(SysError::InterfaceError, Error::ZeroSize as u64);
		}

		match index {
			key!("4kib") => {
				usize::try_from(key).map_or(
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
									InterfaceResponse::ok(
										thread
											.with(|t| t.instance().clone())
											.with_mut(|i| i.insert_token(tab)),
									)
								},
							)
					},
				)
			}
			_ => InterfaceResponse::immediate(SysError::BadIndex, 0),
		}
	}

	fn set<A: Arch>(
		_thread: &Tab<Thread<A>>,
		_index: u64,
		_key: u64,
		_value: u64,
	) -> InterfaceResponse {
		InterfaceResponse::immediate(SysError::ReadOnly, 0)
	}
}
