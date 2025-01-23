//! Allows for querying information about memory tokens.

use oro_sysabi::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread, token::Token};

/// Version 0 of the memory token query interface.
#[repr(transparent)]
pub struct MemTokenV0;

impl KernelInterface for MemTokenV0 {
	const TYPE_ID: u64 = oro_sysabi::id::iface::KERNEL_MEM_TOKEN_V0;

	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let Some(token) = thread
			.with(|t| t.instance().clone())
			.with(|i| i.token(index))
		else {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		};

		token.with(|t| {
			match t {
				Token::Normal(token) => {
					// SAFETY(qix-): Ensure that the `usize` fits within a `u64`,
					// SAFETY(qix-): otherwise the below `as` casts will truncate.
					::oro_macro::assert::fits_within::<usize, u64>();

					match key {
						key!("type") => InterfaceResponse::ok(t.type_id()),
						key!("forget") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
						key!("pagesize") => InterfaceResponse::ok(token.page_size() as u64),
						key!("pages") => InterfaceResponse::ok(token.page_count() as u64),
						key!("size") => InterfaceResponse::ok(token.size() as u64),
						key!("commit") => InterfaceResponse::ok(token.commit() as u64),
						_ => InterfaceResponse::immediate(SysError::BadKey, 0),
					}
				}
			}
		})
	}

	fn set<A: Arch>(
		thread: &Tab<Thread<A>>,
		index: u64,
		key: u64,
		_value: u64,
	) -> InterfaceResponse {
		if key == key!("forget") {
			let instance = thread.with(|t| t.instance().clone());
			return instance.with_mut(|i| i.forget_token(index)).map_or_else(
				|| InterfaceResponse::immediate(SysError::BadIndex, 0),
				|_| InterfaceResponse::ok(0),
			);
		}

		InterfaceResponse::immediate(SysError::ReadOnly, 0)
	}
}
