//! Types related to the preboot mapper for the aarch64 architecture.

use crate::mem::layout::Layout;
use oro_common::mem::{
	AddressSpace, AddressSpaceLayout, CloneToken, MapError, PageFrameAllocate, PageFrameFree,
	PhysicalAddressTranslator, PrebootAddressSpace, SupervisorAddressSegment,
	SupervisorAddressSpace, UnmapError,
};

pub struct PrebootMapper<P: PhysicalAddressTranslator> {
	pub(crate) _phantom: core::marker::PhantomData<P>,
}

unsafe impl<P: PhysicalAddressTranslator> PrebootAddressSpace<P> for PrebootMapper<P> {
	type CloneToken = PrebootCloneToken;

	fn new<A>(allocator: &mut A, translator: P) -> Option<Self>
	where
		A: PageFrameAllocate,
	{
		todo!("PrebootMapper::new");
	}

	fn clone_token(&self) -> Self::CloneToken {
		todo!("PrebootMapper::clone_token");
	}

	fn from_token<A>(token: Self::CloneToken, alloc: &mut A) -> Self
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("PrebootMapper::from_token");
	}
}

unsafe impl<P: PhysicalAddressTranslator> AddressSpace for PrebootMapper<P> {
	type Layout = Layout;
}

impl<P: PhysicalAddressTranslator> SupervisorAddressSpace for PrebootMapper<P> {
	/// The type of [`SupervisorAddressSegment`] that this address space returns.
	type Segment<'a> = PrebootSupervisorSegment<'a, P>
	where
		Self: 'a;

	/// Creates a supervisor segment for the given [`AddressSpaceLayout::Descriptor`].
	fn for_supervisor_segment(
		&self,
		descriptor: <Self::Layout as AddressSpaceLayout>::Descriptor,
	) -> Self::Segment<'_> {
		todo!("PrebootMapper::for_supervisor_segment");
	}
}

#[derive(Clone)]
#[repr(C, align(16))]
pub struct PrebootCloneToken(u8);

impl CloneToken for PrebootCloneToken {}

pub struct PrebootSupervisorSegment<'a, P: PhysicalAddressTranslator> {
	_phantom: core::marker::PhantomData<&'a P>,
}

unsafe impl<'a, P: PhysicalAddressTranslator> SupervisorAddressSegment
	for PrebootSupervisorSegment<'a, P>
{
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("PrebootSupervisorSegment::map");
	}

	fn remap<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("PrebootSupervisorSegment::remap");
	}

	/// Unmaps a virtual address, returning the page frame that was mapped.
	fn unmap<A>(&mut self, allocator: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("PrebootSupervisorSegment::unmap");
	}
}
