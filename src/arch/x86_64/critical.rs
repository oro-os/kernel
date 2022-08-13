//! Utility functions for handling critical procedures.
//! "Critical", in the case of the kernel, typically means
//! putting the CPU into a state whereby code can run
//! uninterrupted, except for cases of edge-case exceptions
//! or other, possibly irrecoverable, failures.

/// Runs a functor with the lowest possible chance of being interrupted.
pub use ::x86_64::instructions::interrupts::without_interrupts as run_critical_section;
