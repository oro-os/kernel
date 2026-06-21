// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::sync::atomic::{AtomicPtr, AtomicU64};

/// Multi-Processor request flag: Enable x2APIC mode if supported.
pub const MP_FLAG_X2APIC: u64 = 1 << 0;

/// Information about a single processor.
#[repr(C)]
pub struct MpInfo {
    pub processor_id: u32,
    pub lapic_id: u32,
    _resvd0: u64,
    // Note: I would encode the [`MpGotoFunction`] type in here, but stable Rust has no atomic function pointer support.
    goto_addr: AtomicPtr<()>,
    extra_argument: AtomicU64,
}

#[repr(C)]
pub struct MpRespData {
    pub flags: u32,
    pub bsp_lapic_id: u32,
    cpu_count: u64,
    cpus: *const (),
}
