//! Spinlock implementations, used to provide mutual exclusion in the kernel
//! between multiple threads of execution that may access the same resource
//! concurrently.

pub(crate) mod unfair;
pub(crate) mod unfair_critical;
