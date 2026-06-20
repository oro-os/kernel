// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

pub const ROTATE_0: u64 = 0;
pub const ROTATE_90: u64 = 1;
pub const ROTATE_180: u64 = 2;
pub const ROTATE_270: u64 = 3;

#[repr(C)]
pub struct FlantermParams {
    canvas: *mut u8,
    canvas_size: u64,
    pub ansi_colors: [u32; 8],
    pub ansi_bright_colors: [u32; 8],
    pub default_bg: u32,
    pub default_fg: u32,
    pub default_bg_bright: u32,
    pub default_fg_bright: u32,
    font: *mut (),
    font_width: u64,
    font_height: u64,
    font_spacing: u64,
    font_scale_x: u64,
    font_scale_y: u64,
    pub margin: u64,
    pub rotation: u64,
}

#[repr(C)]
pub struct Font<'a> {
    pub font: &'a [u8],
    pub width: u64,
    pub height: u64,
    pub spacing: u64,
    pub scale_x: u64,
    pub scale_y: u64,
}

impl FlantermParams {
    pub const fn canvas(&self) -> Option<&[u8]> {
        if self.canvas.is_null() {
            return None;
        }
        Some(unsafe { &*core::ptr::slice_from_raw_parts(self.canvas, self.canvas_size as usize) })
    }

    pub const fn canvas_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *core::ptr::slice_from_raw_parts_mut(self.canvas, self.canvas_size as usize) }
    }

    const fn font_size(&self) -> usize {
        self.font_width as usize * self.font_height as usize * 256 / 8
    }

    pub const fn font<'a>(&'a self) -> Option<Font<'a>> {
        if self.font.is_null() {
            return None;
        }
        Some(unsafe {
            Font {
                font: &*core::ptr::slice_from_raw_parts(self.font as *const u8, self.font_size()),
                width: self.font_width,
                height: self.font_height,
                spacing: self.font_spacing,
                scale_x: self.font_scale_x,
                scale_y: self.font_scale_y,
            }
        })
    }
}
