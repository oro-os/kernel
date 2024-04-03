//! For the preboot phase, we need to be able to allocate frames,
//! but never have to free them. Instead of somehow mandating
//! a virtual address area is available during the preboot stage
//! to map in and out physical frames for the [`crate::mem::FiloPageFrameAllocatpr`]
//! (which would incur some potentially very uncomfortable constraints),
//! and we don't want to accidentally free frames since they'll _never_
//! be recovered by the kernel otherwise, we instead make an allocator
//! wrapper that panics on free.
#![allow(clippy::inline_always)]

use crate::mem::{PageFrameAllocate, PageFrameFree};

/// A page frame allocator that panics on free.
///
/// Used only by the preboot initialization routine.
#[repr(transparent)]
pub(crate) struct PanicOnFreeAllocator<A>(pub A)
where
	A: PageFrameAllocate;

unsafe impl<A> PageFrameAllocate for PanicOnFreeAllocator<A>
where
	A: PageFrameAllocate,
{
	#[inline(always)]
	fn allocate(&mut self) -> Option<u64> {
		self.0.allocate()
	}
}

unsafe impl<A> PageFrameFree for PanicOnFreeAllocator<A>
where
	A: PageFrameAllocate,
{
	#[inline(always)]
	unsafe fn free(&mut self, _frame: u64) {
		panic!("PanicOnFreeAllocator: attempted to free a frame");
	}
}
