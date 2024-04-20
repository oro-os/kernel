use crate::mem::layout::Layout;
use oro_common::mem::{
	AddressSpace, AddressSpaceLayout, MapError, PageFrameAllocate, PageFrameFree,
	RuntimeAddressSpace, SupervisorAddressSegment, SupervisorAddressSpace, UnmapError,
};

pub struct KernelMapper;

unsafe impl RuntimeAddressSpace for KernelMapper {
	type AddressSpaceHandle = u8;

	unsafe fn take() -> Self {
		todo!("KernelMapper::take");
	}

	unsafe fn make_active(&mut self, handle: Self::AddressSpaceHandle) -> Self::AddressSpaceHandle {
		todo!("KernelMapper::make_active");
	}

	fn handle(&self) -> Self::AddressSpaceHandle {
		todo!("KernelMapper::handle");
	}
}

unsafe impl AddressSpace for KernelMapper {
	type Layout = Layout;
}

impl SupervisorAddressSpace for KernelMapper {
	type Segment<'a> = KernelSupervisorSegment<'a>;

	fn for_supervisor_segment(
		&self,
		descriptor: <Self::Layout as AddressSpaceLayout>::Descriptor,
	) -> Self::Segment<'_> {
		todo!("KernelMapper::for_supervisor_segment");
	}
}

pub struct KernelSupervisorSegment<'a> {
	_phantom: core::marker::PhantomData<&'a ()>,
}

unsafe impl SupervisorAddressSegment for KernelSupervisorSegment<'_> {
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("KernelSupervisorSegment::map");
	}

	fn remap<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("KernelSupervisorSegment::remap");
	}

	fn unmap<A>(&mut self, allocator: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		todo!("KernelSupervisorSegment::unmap");
	}
}
