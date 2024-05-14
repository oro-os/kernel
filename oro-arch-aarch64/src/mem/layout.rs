use oro_common::mem::AddressSpaceLayout;

pub struct Layout;

unsafe impl AddressSpaceLayout for Layout {
	/// The descriptor type that is passed to mapper methods to create
	/// address space segments.
	type Descriptor = Descriptor;

	fn kernel_code() -> Self::Descriptor {
		todo!("Layout::kernel_code");
	}

	fn kernel_data() -> Self::Descriptor {
		todo!("Layout::kernel_data");
	}

	fn kernel_rodata() -> Self::Descriptor {
		todo!("Layout::kernel_rodata");
	}

	fn direct_map() -> Self::Descriptor {
		todo!("Layout::direct_map");
	}
}

pub struct Descriptor(u8);
