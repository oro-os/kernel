//! Holds the core state and initialization routines.
//!
//! Much of the CPU-local state is stored in this module.
#![allow(clippy::inline_always)]

use crate::local::volatile::Volatile;
use oro_arch::Target;
use oro_common::{
	arch::Arch,
	mem::{
		mapper::{AddressSegment, AddressSpace, MapError},
		pfa::alloc::{PageFrameAllocate, PageFrameFree},
		translate::PhysicalAddressTranslator,
	},
};
use oro_common_assertions as assert;

/// Stores the pointer to the core-local state.
///
/// The pointer is the same value for all cores,
/// but the backing pages are set to be core-local
/// during boot.
///
/// This might seem strange. And it is. Due to the
/// lack of proper TLS due to lack of more flexible,
/// primitive and non-ELF-based threading models,
/// we have to manufacture our own TLS.
///
/// The kernel, during boot, will allocate enough
/// pages to store the core-local state for its own
/// core at the same location in the supervisor memory
/// (see [`oro_common::mem::mapper::AddressSpace::kernel_core_local()`]),
/// and then the primary will set the pointer to the base
/// of that segment.
///
/// This is a bit of a hack, but it's relatively clean.
/// In the future, assuming more flexible threading models
/// are implemented in either LLVM/Rust, we can switch
/// to using those.
static mut CORE_STATE_PTR: *mut CoreState = core::ptr::null_mut();

/// Returns a reference to the core-local state.
///
/// # Safety
/// Must not be called prior to [`initialize_core_state()`].
#[inline(always)]
pub unsafe fn core_state() -> &'static mut CoreState {
	&mut *CORE_STATE_PTR
}

/// Initializes the core-local state.
///
/// # Safety
/// Must be called exactly once per core.
///
/// Exactly one invocation of this function must
/// be passed `is_primary = true`. No other
/// address space handles may exist that intend
/// to interact with segments overlapping the
/// [`oro_common::mem::mapper::AddressSpace::kernel_core_local()`].
///
/// Caller MUST barrier after calling this function.
pub unsafe fn initialize_core_state<P, Alloc>(
	alloc: &mut Alloc,
	translator: &P,
	is_primary: bool,
) -> Result<(), MapError>
where
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree,
{
	// TODO(qix-): Assumes 4KiB pages; in the future, if we support
	// TODO(qix-): larger pages, this will need to be adjusted.
	let num_pages = ((core::mem::size_of::<CoreState>() + 4095) & !4095) >> 12;

	// SAFETY(qix-): As stated by the safety documentation, we only
	// SAFETY(qix-): modify the non-overlapping core-local segments here,
	// SAFETY(qix-): which barring a bug in the architecture-specific
	// SAFETY(qix-): address space implementation, should be safe.
	let mapper =
		<<Target as Arch>::AddressSpace as AddressSpace>::current_supervisor_space(translator);
	let segment = <<Target as Arch>::AddressSpace as AddressSpace>::kernel_core_local();

	for i in 0..num_pages {
		let phys = alloc.allocate().ok_or(MapError::OutOfMemory)?;
		// TODO(qix-): Again, this assumes 4KiB pages. Might need to adjust
		// TODO(qix-): in the future.
		let virt = segment.range().0 + i * 4096;
		segment.map(&mapper, alloc, translator, virt, phys)?;
	}

	// TODO(qix-): This once again assumes 4KiB pages. Might need to adjust
	// TODO(qix-): in the future.
	assert::aligns_to::<CoreState, 4096>();

	let core_state = segment.range().0 as *mut CoreState;

	// SAFETY(qix-): This is the only place we should be initializng
	// SAFETY(qix-): the core-local volatile structures.
	core_state.write_volatile(CoreState {
		ticked: Volatile::new(0),
	});

	if is_primary {
		CORE_STATE_PTR = core_state;
	}

	Target::strong_memory_barrier();

	Ok(())
}

/// Local state for the core, manipulated by the interrupt handler
/// and used by the main kernel thread.
///
/// # Safety
/// This is a race condition minefield of a structure.
/// Values should never be assumed to have stayed the same
/// between accesses.
///
/// This structure is not thread-safe. It must only be accessed
/// within the local core. It is intended to be allocated into
/// a well-known location only accessible to the core.
#[repr(align(4096))]
pub struct CoreState {
	/// A tick has occurred.
	pub ticked: Volatile<u8>,
}
