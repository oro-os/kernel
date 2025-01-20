//! `oro-sync` backing functionality.
//!
//! These are mostly just implementations to allow certain advanced features of
//! the `oro-sync` crate work (e.g. `ReentrantMutex`).

use core::mem::MaybeUninit;

use crate::arch::Arch;

/// Holds the "global" kernel ID function pointer, which retrieves the **core local**
/// kernel ID for the calling context's current core.
///
/// This is an ergonomics hack to avoid `A: Arch` in a lot of places.
static mut KERNEL_ID_FN: MaybeUninit<fn() -> u32> = MaybeUninit::uninit();

/// Debug field for ensuring the kernel ID function is set.
#[cfg(debug_assertions)]
static HAS_SET_KERNEL_ID_FN: core::sync::atomic::AtomicBool =
	core::sync::atomic::AtomicBool::new(false);

/// Retrieves the current core's kernel ID. This is linked to by `oro-sync` for the
/// [`oro_sync::ReentrantLock`] implementation.
#[doc(hidden)]
#[no_mangle]
pub(crate) unsafe extern "C" fn oro_sync_current_core_id() -> u32 {
	#[cfg(debug_assertions)]
	{
		assert!(
			HAS_SET_KERNEL_ID_FN.load(core::sync::atomic::Ordering::Relaxed),
			"kernel ID function not set"
		);
	}

	let id = KERNEL_ID_FN.assume_init()();
	::oro_dbgutil::__oro_dbgutil_core_id_fn_was_called(id);
	id
}

/// The generic kernel ID fetcher, based on the [`Arch`] type.
#[doc(hidden)]
fn get_arch_kernel_id<A: Arch>() -> u32 {
	crate::Kernel::<A>::get().id()
}

/// Initializes the kernel ID function pointer.
///
/// # Safety
/// This must be called **exactly once** prior to any [`oro_sync::ReentrantMutex`] locking.
///
/// Further, `A` **must** be the same [`Arch`] type as the kernel being initialized,
/// and **must** be the same across **all cores**.
pub unsafe fn initialize_kernel_id_fn<A: Arch>() {
	#[cfg(debug_assertions)]
	{
		HAS_SET_KERNEL_ID_FN.store(true, core::sync::atomic::Ordering::Relaxed);
	}

	// SAFETY(qix-): We have offloaded safety considerations to the caller here.
	#[expect(static_mut_refs)]
	{
		::oro_dbgutil::__oro_dbgutil_core_id_fn_was_set(get_arch_kernel_id::<A>());
		KERNEL_ID_FN.write(get_arch_kernel_id::<A>);
	}
}

/// Installs a dummy kernel ID for use during early boot.
///
/// # Safety
/// This function *may* be called prior to [`initialize_kernel_id_fn`], but
/// **must** be called prior to any locking with [`oro_sync::ReentrantMutex`]
/// if it is.
///
/// **The handler installed by this function must not be used once multiple
/// cores are active.**
pub unsafe fn install_dummy_kernel_id_fn() {
	#[cfg(debug_assertions)]
	{
		HAS_SET_KERNEL_ID_FN.store(false, core::sync::atomic::Ordering::Relaxed);
	}

	// SAFETY(qix-): We have offloaded safety considerations to the caller here.
	#[expect(static_mut_refs)]
	{
		::oro_dbgutil::__oro_dbgutil_core_id_fn_was_set(0xDEAD_DEAD);
		KERNEL_ID_FN.write(|| 0xDEAD_DEAD);
	}
}
