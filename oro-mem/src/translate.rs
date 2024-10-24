//! Memory address translation system. Used globally.

// NOTE(qix-): This module implements a very simplistic module offset
// NOTE(qix-): calculation that might be considered by some to be
// NOTE(qix-): both rudimentary and bad practice.
// NOTE(qix-):
// NOTE(qix-): You wouldn't be wrong for thinking so. However, I'm writing
// NOTE(qix-): this as a sort of historic note to more or less justify
// NOTE(qix-): my decision.
// NOTE(qix-):
// NOTE(qix-): This module used to be complex. In fact the first iteration of
// NOTE(qix-): this system didn't even require that the memory was linear
// NOTE(qix-): mapped, just that a stable physical->virtual translation
// NOTE(qix-): was possible.
// NOTE(qix-):
// NOTE(qix-): It was very overengineered; PAT (phys-addr translation) generics
// NOTE(qix-): were passed around _everywhere_. Getting them from the architectures
// NOTE(qix-): was a nightmare, const generic limitations caused a ton of issues
// NOTE(qix-): and resulted in a bunch of hacks, and as the kernel began to grow
// NOTE(qix-): and evolve, the ergonomics really slowed things down.
// NOTE(qix-):
// NOTE(qix-): Further, as the first few architectures started to flesh out,
// NOTE(qix-): and as more research was done on other architectures, it was clear
// NOTE(qix-): that a linear map would be the defacto way for 64-bit architectures
// NOTE(qix-): to access physical memory. Obviously this won't scale to 32-bit
// NOTE(qix-): architectures but for the first version of the kernel this should
// NOTE(qix-): be sufficient. Given that the kernel is so small compared to
// NOTE(qix-): other mainstream OSes, the goal is to be able to work this
// NOTE(qix-): into a better system in the future without the need for massive
// NOTE(qix-): refactors.
// NOTE(qix-):
// NOTE(qix-): Thus the removal of the generics that were scattered everywhere
// NOTE(qix-): began, and I implemented a less complex but equally as clever
// NOTE(qix-): solution that involved proc macros and extern linked functions
// NOTE(qix-): to model something akin to Rust's own global allocator attribute,
// NOTE(qix-): with the goal of re-using that system for e.g. the global page frame
// NOTE(qix-): allocator, etc.
// NOTE(qix-):
// NOTE(qix-): However, as time went on it was clear _that_ wouldn't work either,
// NOTE(qix-): as the mapping APIs were re-used for early boot-stage PFA variants
// NOTE(qix-): that needed a different way of accessing/marking memory before
// NOTE(qix-): memory was linear mapped, and other solutions such as an enum-switched
// NOTE(qix-): global PFA depending on the boot stage introducing a dynamic dispatch
// NOTE(qix-): or `match` branch seemed like an unacceptably slow solution.
// NOTE(qix-):
// NOTE(qix-): So for now, this is the lowest resistance solution that of course only
// NOTE(qix-): works on 64-bit systems whereby a linear map is possible, which
// NOTE(qix-): will at least get the project to v0 and not convolute the APIs
// NOTE(qix-): so much except for points where translation is really necessary,
// NOTE(qix-): keeping things simple until the time a more complex physical memory
// NOTE(qix-): access layer is necessary.
// NOTE(qix-):
// NOTE(qix-): So, like I said, this is actually the refined and evolved, conscious
// NOTE(qix-): evolution of this subsystem, despite it being simplistic.

/// Holds the linear map offset for the entire system. The resulting
/// value (when added to the physical address in question) must result
/// in a valid virtual address (which also means fitting within a `usize`).
static mut LINEAR_MAP_OFFSET: u64 = 0;

/// Debug flag for whether or not the linear map offset has been populated.
/// Very slow (using `SeqCst`), so only enabled in debug builds.
#[cfg(debug_assertions)]
static LINEAR_MAP_SET: ::core::sync::atomic::AtomicBool =
	::core::sync::atomic::AtomicBool::new(false);

/// Sets the global (kernel-wide) offset for all mapped memory.
///
/// # Safety
/// Must only be called once, before any translations take place.
///
/// Must be called only by the boot core, before other cores are
/// initialized.
#[cfg_attr(debug_assertions, expect(clippy::missing_panics_doc))]
pub unsafe fn set_global_map_offset(offset: u64) {
	#[cfg(debug_assertions)]
	{
		assert!(
			!LINEAR_MAP_SET.swap(true, ::core::sync::atomic::Ordering::SeqCst),
			"global linear map offset already set"
		);
	}

	// SAFETY: Safety requirements of this function indicate this should
	// SAFETY: only be written to once before any other threads access it.
	unsafe {
		LINEAR_MAP_OFFSET = offset;
	}
}

/// Gets the global (kernel-wide) offset for all mapped memory.
///
/// > **NOTE:** Not marked unsafe, but the caller must understand
/// > that this may return `0` in release builds where [`set_global_map_offset`]
/// > has not yet been called. This isn't the typical case so for
/// > that reason it's not marked as unsafe.
#[cfg_attr(debug_assertions, expect(clippy::missing_panics_doc))]
pub fn global_map_offset() -> u64 {
	#[cfg(debug_assertions)]
	{
		assert!(
			LINEAR_MAP_SET.load(::core::sync::atomic::Ordering::SeqCst),
			"global_map_offset() called but has not yet been set"
		);
	}

	// SAFETY: We only ever write once and then return it here.
	unsafe { LINEAR_MAP_OFFSET }
}

/// Translates a physical address to a virtual address.
///
/// # Panics
/// Panics if the resulting virtual address does not fit within
/// a `usize`.
#[must_use]
pub fn to_virtual(phys: u64) -> usize {
	usize::try_from(phys + global_map_offset()).unwrap()
}
