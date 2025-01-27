//! Boot-time video buffer interface for the root ring.
//!
//! If the bootloader has provided a video buffer, this interface
//! allows modules to map it in and write to it.
//!
//! # Safety
//!
//! This interface is **very unsafe**. **DO NOT EXPOSE IT TO
//! UNTRUSTED MODULES**. There is a small chance that physical
//! memory that exists at the end of the tail pages could be
//! SENSITIVE or otherwise important to the system. **DO NOT
//! EXPOSE THIS INTERFACE TO UNTRUSTED MODULES**.
//!
//! **This interface is **NOT** thread safe.** Multiple mappings
//! of the video buffer can cause race conditions if multiple
//! modules map it in at the same time (which isn't enforced
//! by the kernel).
//!
//! The interface **will** map the video buffer as normal memory,
//! anywhere into the address space requested, for as many pages
//! as the video buffer's size.

use core::marker::PhantomData;

use oro::{key, syscall::Error as SysError};
use oro_boot_protocol::{RGBVideoBuffer, VideoBuffersRequest, video_buffers::VideoBuffersKind};
use oro_debug::{dbg, dbg_warn};
use oro_mem::{
	alloc::vec::Vec,
	mapper::{AddressSegment, AddressSpace, MapError},
	phys::{Phys, PhysAddr},
};
use oro_sync::{Lock, Mutex};

use crate::{
	arch::Arch, interface::Interface, syscall::InterfaceResponse, tab::Tab, thread::Thread,
};

/// The video buffer kernel request.
///
/// Optional.
#[used]
#[link_section = ".oro_boot"]
pub static VBUF_REQUEST: VideoBuffersRequest = VideoBuffersRequest::with_revision(0);

/// Interface-specific error type.
#[repr(u64)]
pub enum Error {
	/// The given virtual address to map is not aligned to a page.
	Unaligned      = key!("badvalgn"),
	/// One or more pages already exist starting at the given virtual
	/// address. No pages have been mapped.
	ConflictingMap = key!("conflict"),
	/// The virtual address of one or more page mappings is outside the valid
	/// range of the address space.
	OutOfRange     = key!("range"),
	/// The system ran out of memory while mapping the video buffer.
	OutOfMemory    = key!("oom"),
}

/// Inner state of the debug output stream.
struct Inner {
	/// A list of all of the buffers available to the system.
	buffers: Vec<RGBVideoBuffer>,
}

impl Default for Inner {
	fn default() -> Self {
		let mut this = Self {
			buffers: Vec::new(),
		};

		if let Some(VideoBuffersKind::V0(vbufs)) = VBUF_REQUEST.response() {
			// SAFETY: The `response()` method has done as much checking as is possible.
			// SAFETY: This is just inherently unsafe.
			let buffers = unsafe { vbufs.assume_init_ref() };
			let mut current_phys = unsafe { core::ptr::read_volatile(&buffers.next) };

			while current_phys != 0 {
				let phys = unsafe { Phys::from_address_unchecked(current_phys) };
				let buffer: Option<&RGBVideoBuffer> = phys.as_ref();
				let Some(buffer) = buffer else {
					dbg_warn!(
						"bootloader provided a misaligned video buffer structure; stopping: \
						 {current_phys:#016x}"
					);
					break;
				};

				// SAFETY: We assume the buffer is valid here since it comes from the bootloader;
				// SAFETY: there's really no way to assure this.
				let buffer = unsafe { core::ptr::read_volatile(buffer) };

				current_phys = buffer.next;

				if buffer.base & 0xFFF != 0 {
					dbg_warn!(
						"bootloader provided a misaligned video buffer; skipping: {:#016x}",
						buffer.base
					);
					continue;
				}

				dbg!("found video buffer: {:#016x}", buffer.base);

				this.buffers.push(buffer);
			}
		}

		dbg!("discovered {} video buffer(s)", this.buffers.len());

		this
	}
}

/// See the module level documentation for information about
/// the root ring boot virtual buffer interface.
pub struct BootVbufV0<A: Arch>(Mutex<Inner>, PhantomData<A>);

impl<A: Arch> BootVbufV0<A> {
	/// Creates a new `DebugOutV0` instance.
	#[must_use]
	pub fn new() -> Self {
		Self(Mutex::new(Inner::default()), PhantomData)
	}
}

impl<A: Arch> Interface<A> for BootVbufV0<A> {
	fn type_id(&self) -> u64 {
		oro::id::iface::ROOT_BOOT_VBUF_V0
	}

	fn get(&self, _thread: &Tab<Thread<A>>, index: u64, key: u64) -> InterfaceResponse {
		let this = self.0.lock();

		let Some(buffer) = this.buffers.get(index as usize) else {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		};

		let value = match key {
			key!("width") => buffer.width,
			key!("height") => buffer.height,
			key!("pitch") => buffer.row_pitch,
			key!("bit_pp") => buffer.bits_per_pixel.into(),
			key!("red_size") => buffer.red_mask.into(),
			key!("grn_size") => buffer.green_mask.into(),
			key!("blu_size") => buffer.blue_mask.into(),
			key!("red_shft") => buffer.red_shift.into(),
			key!("grn_shft") => buffer.green_shift.into(),
			key!("blu_shft") => buffer.blue_shift.into(),
			key!("!vmbase!") => {
				return InterfaceResponse::immediate(SysError::WriteOnly, 0);
			}
			_ => {
				return InterfaceResponse::immediate(SysError::BadKey, 0);
			}
		};

		InterfaceResponse::ok(value)
	}

	fn set(&self, thread: &Tab<Thread<A>>, index: u64, key: u64, value: u64) -> InterfaceResponse {
		let this = self.0.lock();

		let Some(buffer) = this.buffers.get(index as usize) else {
			return InterfaceResponse::immediate(SysError::BadIndex, 0);
		};

		match key {
			key!("width")
			| key!("height")
			| key!("pitch")
			| key!("bit_pp")
			| key!("red_size")
			| key!("grn_size")
			| key!("blu_size")
			| key!("red_shft")
			| key!("grn_shft")
			| key!("blu_shft") => InterfaceResponse::immediate(SysError::ReadOnly, 0),
			key!("!vmbase!") => {
				if value & 0xFFF != 0 {
					return InterfaceResponse::immediate(
						SysError::InterfaceError,
						Error::Unaligned as u64,
					);
				}

				let num_pages = ((buffer.row_pitch * buffer.height) + 0xFFF) >> 12;

				let res = thread.with(|t| {
					let mapper = t.mapper();

					for i in 0..num_pages {
						let res: Result<(), InterfaceResponse> = (|| {
							let vaddr = value.checked_add(i << 12).ok_or_else(|| {
								InterfaceResponse::immediate(
									SysError::InterfaceError,
									Error::OutOfRange as u64,
								)
							})?;

							// NOTE(qix-): Oftentimes you'd map MMIO as Device-nGnRnE (or equivalent), but
							// NOTE(qix-): video buffers are a bit of a special case in that write combining,
							// NOTE(qix-): caching, etc. aren't as important as just getting the data out.
							// NOTE(qix-):
							// NOTE(qix-): For now, we'll just map the video buffer as normal memory. Might
							// NOTE(qix-): need to revisit this in the future.
							<A::AddressSpace as AddressSpace>::user_data()
								.map(mapper, vaddr as usize, buffer.base + (i << 12))
								.map_err(|err| {
									match err {
										MapError::Exists => {
											InterfaceResponse::immediate(
												SysError::InterfaceError,
												Error::ConflictingMap as u64,
											)
										}
										MapError::OutOfMemory => {
											InterfaceResponse::immediate(
												SysError::InterfaceError,
												Error::OutOfMemory as u64,
											)
										}
										MapError::VirtNotAligned => {
											InterfaceResponse::immediate(
												SysError::InterfaceError,
												Error::Unaligned as u64,
											)
										}
										MapError::VirtOutOfAddressSpaceRange
										| MapError::VirtOutOfRange => {
											InterfaceResponse::immediate(
												SysError::InterfaceError,
												Error::OutOfRange as u64,
											)
										}
									}
								})?;

							Ok(())
						})();

						if let Err(err) = res {
							// Unmap all of the pages we've mapped so far.
							for j in 0..i {
								let vaddr = value + (j << 12);
								// Best effort unmap.
								<A::AddressSpace as AddressSpace>::user_data()
									.unmap(mapper, vaddr as usize)
									.ok();
							}

							return Err(err);
						}
					}

					Ok(())
				});

				res.map_or_else(|e| e, |()| InterfaceResponse::ok(num_pages))
			}
			_ => InterfaceResponse::immediate(SysError::BadKey, 0),
		}
	}
}
