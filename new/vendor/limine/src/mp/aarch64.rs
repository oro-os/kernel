// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::sync::atomic::{AtomicPtr, AtomicU64};

/// Information about a single processor.
#[repr(C)]
pub struct MpInfo {
    pub processor_id: u32,
    _resvd0: u32,
    pub mpidr: u64,
    _resvd1: u64,
    // Note: I would encode the [`MpGotoFunction`] type in here, but stable Rust has no atomic function pointer support.
    goto_addr: AtomicPtr<()>,
    extra_argument: AtomicU64,
}

#[repr(C)]
pub struct MpRespData {
    pub flags: u64,
    pub bsp_mpidr: u64,
    cpu_count: u64,
    cpus: *const (),
}
