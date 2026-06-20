// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(target_arch = "x86_64")]
include!("x86_64.rs");
#[cfg(target_arch = "aarch64")]
include!("aarch64.rs");
#[cfg(target_arch = "riscv64")]
include!("riscv64.rs");
#[cfg(target_arch = "loongarch64")]
include!("loongarch64.rs");

// Error if we're not on one of the supported architectures.
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
)))]
compile_error!("The current target architecture is not supported by the Limine boot protocol");

/// Signature for the entrypoint on processor hand-over from the bootloader.
pub type MpGotoFunction = unsafe extern "C" fn(&MpInfo) -> !;

impl MpInfo {
    /// Start this processor.
    /// Writes the extra argument to the struct then writes the goto pointer, triggering the CPU to start running the function provided.
    pub fn bootstrap(&self, address: MpGotoFunction, extra_arg: u64) {
        use core::sync::atomic::Ordering;

        self.extra_argument.store(extra_arg, Ordering::Relaxed);
        self.goto_addr.store(address as *mut (), Ordering::Release);
    }

    /// Retrieve the extra argument passed to this processor by [`Self::bootstrap`].
    pub fn extra_argument(&self) -> u64 {
        use core::sync::atomic::Ordering;

        self.extra_argument.load(Ordering::Acquire)
    }
}

impl MpRespData {
    pub const fn cpus(&self) -> &[&MpInfo] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.cpus as _, self.cpu_count as usize) }
    }
}
