//! synchronization primitives used throughout the Oro kernel.
#![cfg_attr(not(test), no_std)]

pub mod barrier;
pub mod spinlock;
