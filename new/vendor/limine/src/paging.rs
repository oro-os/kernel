// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(non_camel_case_types)]

/// What paging mode to use on kernel entry.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PagingMode {
    /// 4-level x86_64 paging.
    #[cfg(target_arch = "x86_64")]
    X86_64_4LVL = 0,
    /// 5-level x86_64 paging.
    #[cfg(target_arch = "x86_64")]
    X86_64_5LVL = 1,

    /// 4-level aarch64 paging.
    #[cfg(target_arch = "aarch64")]
    AARCH64_4LVL = 0,
    /// 5-level aarch64 paging.
    #[cfg(target_arch = "aarch64")]
    AARCH64_5LVL = 1,

    /// RISC-V Sv39 paging.
    #[cfg(target_arch = "riscv64")]
    RISCV_SV39 = 0,
    /// RISC-V Sv48 paging.
    #[cfg(target_arch = "riscv64")]
    RISCV_SV48 = 1,
    /// RISC-V Sv57 paging.
    #[cfg(target_arch = "riscv64")]
    RISCV_SV57 = 2,

    /// Loongarch64 4-level paging.
    #[cfg(target_arch = "loongarch64")]
    LOONGARCH64_4LVL = 0,
}

impl PagingMode {
    /// Minimum supported paging mode.
    #[cfg(target_arch = "x86_64")]
    pub const MIN: PagingMode = PagingMode::X86_64_4LVL;
    /// Maximum supported paging mode.
    #[cfg(target_arch = "x86_64")]
    pub const MAX: PagingMode = PagingMode::X86_64_5LVL;

    /// Minimum supported paging mode.
    #[cfg(target_arch = "aarch64")]
    pub const MIN: PagingMode = PagingMode::AARCH64_4LVL;
    /// Maximum supported paging mode.
    #[cfg(target_arch = "aarch64")]
    pub const MAX: PagingMode = PagingMode::AARCH64_5LVL;

    /// Minimum supported paging mode.
    #[cfg(target_arch = "riscv64")]
    pub const MIN: PagingMode = PagingMode::RISCV_SV39;
    /// Maximum supported paging mode.
    #[cfg(target_arch = "riscv64")]
    pub const MAX: PagingMode = PagingMode::RISCV_SV57;

    /// Minimum supported paging mode.
    #[cfg(target_arch = "loongarch64")]
    pub const MIN: PagingMode = PagingMode::LOONGARCH64_4LVL;
    /// Maximum supported paging mode.
    #[cfg(target_arch = "loongarch64")]
    pub const MAX: PagingMode = PagingMode::LOONGARCH64_4LVL;
}
