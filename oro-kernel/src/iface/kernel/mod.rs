//! Kernel level (`id <= 0xFFFF_FFFF`) interfaces.
//!
//! Always available, regardless of the caller's ring.

use oro_mem::alloc::boxed::Box;

use crate::{arch::Arch, syscall::InterfaceResponse, tab::Tab, table::Table, thread::Thread};

/// Small helper trait for kernel interfaces, which are always
/// available, use no state themselves, and are simple gateways
/// to other internal kernel structures.
pub trait KernelInterface<A: Arch> {
	/// See [`crate::interface::Interface::get`].
	fn get(&self, thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse;

	/// See [`crate::interface::Interface::set`].
	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse;
}

/// Attempts to dispatch a [`oro::syscall::Opcode::Get`] system call to a kernel interface.
///
/// If the type ID is not recognized, returns `None`; callers should delegate
/// to a registry lookup and dispatch.
#[must_use]
pub fn try_dispatch_get<A: Arch>(
	thread: &Tab<Thread<A>>,
	type_id: u64,
	index: u64,
	key: u64,
) -> Option<InterfaceResponse> {
	crate::Kernel::<A>::get()
		.state()
		.kernel_interfaces
		.get(type_id)
		.map(|iface| iface.get(thread, index, key))
}

/// Attempts to dispatch a [`oro::syscall::Opcode::Set`] system call to a kernel interface.
///
/// If the type ID is not recognized, returns `None`; callers should delegate
/// to a registry lookup and dispatch.
#[must_use]
pub fn try_dispatch_set<A: Arch>(
	thread: &Tab<Thread<A>>,
	type_id: u64,
	index: u64,
	key: u64,
	value: u64,
) -> Option<InterfaceResponse> {
	crate::Kernel::<A>::get()
		.state()
		.kernel_interfaces
		.get(type_id)
		.map(|iface| iface.set(thread, index, key, value))
}

/// Registers all cross-architecture kernel interfaces with the current
/// kernel state.
pub(crate) fn register_kernel_interfaces<A: Arch>(table: &mut Table<Box<dyn KernelInterface<A>>>) {
	macro_rules! register_all {
		($($id:ident => $iface_mod:literal $iface:ident),* $(,)?) => {
			$({
				#[path = $iface_mod]
				mod ifacemod;

				table.insert(::oro::id::iface::$id, Box::new(ifacemod::$iface::<A>(core::marker::PhantomData)));
			})*
		};
	}

	register_all! {
		KERNEL_THREAD_V0 => "./thread_v0.rs" ThreadV0,
		KERNEL_IFACE_QUERY_BY_TYPE_V0 => "./iface_query_by_type_v0.rs" IfaceQueryByTypeV0,
		KERNEL_IFACE_QUERY_TYPE_META_V0 => "./iface_query_type_meta_v0.rs" IfaceQueryTypeMetaV0,
		KERNEL_MEM_TOKEN_V0 => "./mem_token_v0.rs" MemTokenV0,
		KERNEL_PAGE_ALLOC_V0 => "./page_alloc_v0.rs" PageAllocV0,
	}
}
