//! Kernel level (`id <= 0xFFFF_FFFF`) interfaces.
//!
//! Always available, regardless of the caller's ring.

use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, thread::Thread};

mod iface_query_by_type_v0;
mod iface_query_type_meta_v0;
mod mem_token_v0;
mod page_alloc_v0;
mod thread_v0;

include!("macro/make_dispatch.rs");

make_dispatch! {
	thread_v0::ThreadV0,
	iface_query_by_type_v0::IfaceQueryByTypeV0,
	iface_query_type_meta_v0::IfaceQueryTypeMetaV0,
	mem_token_v0::MemTokenV0,
	page_alloc_v0::PageAllocV0,
}

/// Small helper trait for kernel interfaces, which are always
/// available, use no state themselves, and are simple gateways
/// to other internal kernel structures.
pub trait KernelInterface {
	/// The stable kernel interface ID for this interface.
	const TYPE_ID: u64;

	/// See [`crate::interface::Interface::get`].
	fn get<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse;

	/// See [`crate::interface::Interface::set`].
	fn set<A: Arch>(thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64)
	-> InterfaceResponse;
}
