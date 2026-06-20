// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::sync::atomic::{AtomicPtr, AtomicU64};

/// Information about a single processor.
#[repr(C)]
pub struct MpInfo {
    pub processor_id: u64,
    pub hartid: u64,
    _resvd0: u64,
    // Note: I would encode the [`MpGotoFunction`] type in here, but stable Rust has no atomic function pointer support.
    goto_addr: AtomicPtr<()>,
    extra_argument: AtomicU64,
}

#[repr(C)]
pub struct MpRespData {
    pub flags: u64,
    pub bsp_hartid: u64,
    cpu_count: u64,
    cpus: *const (),
}
