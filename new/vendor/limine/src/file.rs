// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::ffi::{CStr, c_char};
#[cfg(feature = "ipaddr")]
use core::net::Ipv4Addr;

use crate::uuid::Uuid;

pub const LIMINE_MEDIA_TYPE_GENERIC: u32 = 0;
pub const LIMINE_MEDIA_TYPE_OPTICAL: u32 = 1;
pub const LIMINE_MEDIA_TYPE_TFTP: u32 = 2;

#[repr(C)]
pub struct File {
    pub revision: u64,
    address: *mut (),
    size: u64,
    path: *const c_char,
    cmdline: *const c_char,
    pub media_type: u32,
    pub tftp_ip: u32,
    pub tftp_port: u32,
    pub partition_index: u32,
    pub mbr_disk_id: u32,
    pub gpt_disk_uuid: Uuid,
    pub gpt_part_uuid: Uuid,
    pub part_uuid: Uuid,
}

impl File {
    pub fn data(&self) -> &[u8] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.address as *const u8, self.size as usize) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe {
            &mut *core::ptr::slice_from_raw_parts_mut(self.address as *mut u8, self.size as usize)
        }
    }

    pub fn path(&self) -> &str {
        unsafe { CStr::from_ptr(self.path).to_str().unwrap() }
    }

    pub fn cmdline(&self) -> &str {
        unsafe { CStr::from_ptr(self.cmdline).to_str().unwrap() }
    }

    #[cfg(feature = "ipaddr")]
    pub fn tftp_ipv4(&self) -> Option<Ipv4Addr> {
        if self.tftp_ip == 0 {
            return None;
        }
        let bytes = self.tftp_ip.to_le_bytes();
        Some(Ipv4Addr::new(bytes[3], bytes[2], bytes[1], bytes[0]))
    }
}
