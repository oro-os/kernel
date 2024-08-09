//! Implements the core-local state, interrupt handling, etc.
//! to be used by the rest of the kernel's subsystems.

pub mod interrupt;
pub(super) mod main;
pub mod state;
pub mod volatile;
