//! Implements interrupt handling for the x86_64 architecture.

use oro_common::interrupt::InterruptHandler;

/// Initializes the interrupt descriptor table
/// and associated entries.
///
/// # Safety
/// Must be called exactly once during boot for each core.
pub unsafe fn initialize_interrupts<H: InterruptHandler>() {}
