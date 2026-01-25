#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

mod paging_level;
pub mod reg;

pub use paging_level::PagingLevel;

/// Initializes the architecture-specific components and caches used
/// throughout the kernel.
///
/// # Safety
/// This function must be called only once during the kernel initialization
/// phase, before any other architecture-specific functionality is used,
/// after setting up the CPU in long mode and enabling any required features.
///
/// While this function can be called multiple times "safely" (as in, calling
/// it doesn't itself pose any inherent risk), doing so might invalidate other
/// safety checks that have occurred based on the previous state.
#[expect(
	clippy::missing_inline_in_public_items,
	reason = "inlining is not necessary for init functions"
)]
#[cold]
pub unsafe fn refresh_globals() {
	// SAFETY: Safety is delegated to the caller.
	unsafe {
		PagingLevel::refresh_globals();
	}
}
