// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::ops::Deref;

pub const FRAMEBUFFER_RGB: u8 = 1;

/// Alternate video modes as used by [`FramebufferRev1`].
#[repr(C)]
pub struct VideoMode {
    pub pitch: u64,
    pub width: u64,
    pub height: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

/// Revision 0 version of the framebuffer struct.
#[repr(C)]
pub struct Framebuffer {
    address: *mut (),
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    _resvd0: [u8; 7],
    edid_size: u64,
    edid: *const (),
}

impl Framebuffer {
    pub fn address(&self) -> *mut () {
        self.address
    }

    pub fn size(&self) -> usize {
        (self.height * self.pitch) as usize
    }

    pub fn edid(&self) -> &[u8] {
        unsafe { &*core::ptr::from_raw_parts(self.edid as _, self.edid_size as usize) }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { &*core::ptr::from_raw_parts(self.address as *const u8, self.size()) }
    }

    pub unsafe fn as_slice_mut(&self) -> &mut [u8] {
        unsafe { &mut *core::ptr::from_raw_parts_mut(self.address as *mut u8, self.size()) }
    }
}

/// Revision 1 version of the framebuffer struct.
#[repr(C)]
pub struct FramebufferRev1 {
    rev0: Framebuffer,
    mode_count: u64,
    modes: *const &'static VideoMode,
}

impl FramebufferRev1 {
    pub fn modes(&self) -> &[&VideoMode] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.modes, self.mode_count as usize) }
    }
}

impl Deref for FramebufferRev1 {
    type Target = Framebuffer;

    fn deref(&self) -> &Self::Target {
        &self.rev0
    }
}
