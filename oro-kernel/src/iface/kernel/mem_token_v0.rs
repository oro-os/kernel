//! Allows for querying information about memory tokens.
//!
//! # ⚠️ Stability Note ⚠️
//! **This interface is incomplete.** Mappings that work now may not work in the future.
//!
//! The interface currently **does not** check for executable mapping conflicts -
//! i.e. code, data, and rodata program segments mapped in as part of the module.
//!
//! Mapping operations to set the base of a token that overlaps with those regions
//! **will not fail**, but also **will not be mapped**, as there will be no page
//! fault for accesses to those regions.
//!
//! In the future, setting a `base` that *does* conflict **will** fail. Please be
//! extra careful about your base addresses and spans when using this interface
//! in order to be future-proof.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};

use super::KernelInterface;
use crate::{
	arch::Arch,
	syscall::InterfaceResponse,
	tab::Tab,
	thread::{Thread, TokenMapError},
	token::Token,
};

/// Interface specific errors.
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum Error {
	/// An address conflict (existing mapping) was encountered when mapping a token.
	Conflict   = key!("conflict"),
	/// The requested address is not aligned to the page size.
	NotAligned = key!("align"),
	/// The requested address is out of the address space range.
	OutOfRange = key!("range"),
}

/// Version 0 of the memory token query interface.
#[repr(transparent)]
pub struct MemTokenV0<A: Arch>(pub(crate) PhantomData<A>);

impl<A: Arch> KernelInterface<A> for MemTokenV0<A> {
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let Some(token) = thread.with(|t| t.token(index)) else {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		};

		token.with(|t| {
			match t {
				Token::Normal(token) | Token::NormalThreadStack(token) => {
					// SAFETY(qix-): Ensure that the `usize` fits within a `u64`,
					// SAFETY(qix-): otherwise the below `as` casts will truncate.
					::oro_macro::assert::fits_within::<usize, u64>();

					match key {
						key!("type") => InterfaceResponse::ok(t.type_id()),
						key!("subtype") => InterfaceResponse::ok(0),
						key!("forget") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
						key!("pagesize") => InterfaceResponse::ok(token.page_size() as u64),
						key!("pages") => InterfaceResponse::ok(token.page_count() as u64),
						key!("size") => InterfaceResponse::ok(token.size() as u64),
						key!("commit") => InterfaceResponse::ok(token.commit() as u64),
						key!("base") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
						_ => InterfaceResponse::immediate(SysError::BadKey, 0),
					}
				}
				Token::PortEndpoint(token) => {
					match key {
						key!("type") => InterfaceResponse::ok(t.type_id()),
						key!("subtype") => InterfaceResponse::ok(token.side() as u64),
						key!("forget") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
						key!("pagesize") => InterfaceResponse::ok(4096),
						key!("pages") => InterfaceResponse::ok(1),
						key!("size") => InterfaceResponse::ok(4096),
						key!("commit") => InterfaceResponse::ok(1),
						key!("base") => InterfaceResponse::immediate(SysError::WriteOnly, 0),
						_ => InterfaceResponse::immediate(SysError::BadKey, 0),
					}
				}
			}
		})
	}

	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse {
		match key {
			key!("forget") => {
				thread.with_mut(|t| t.forget_token(index)).map_or_else(
					|| InterfaceResponse::immediate(SysError::BadIndex, 0),
					|_| InterfaceResponse::ok(0),
				)
			}
			key!("base") => {
				thread.with_mut(|t| {
					let Some(token) = t.token(index) else {
						return InterfaceResponse::immediate(SysError::BadIndex, 0);
					};

					let Ok(virt) = usize::try_from(value) else {
						return InterfaceResponse::immediate(
							SysError::InterfaceError,
							Error::OutOfRange as u64,
						);
					};

					t.try_map_token_at(&token, virt).map_or_else(
						|err| {
							InterfaceResponse::immediate(
								SysError::InterfaceError,
								match err {
									TokenMapError::Conflict => Error::Conflict as u64,
									TokenMapError::VirtNotAligned => Error::NotAligned as u64,
									TokenMapError::VirtOutOfRange => Error::OutOfRange as u64,
									// NOTE(qix-): We already handled this at the beginning of the match.
									TokenMapError::BadToken => unreachable!(),
								},
							)
						},
						|()| InterfaceResponse::ok(0),
					)
				})
			}
			_ => InterfaceResponse::immediate(SysError::ReadOnly, 0),
		}
	}
}
