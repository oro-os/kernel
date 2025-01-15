//! Globally unique identifier allocator.

use core::sync::atomic::{AtomicU64, Ordering};

/// Static, system-wide monotonically increasing resource counter.
///
/// Used for a variety of resources; all resources in the system are guaranteed
/// to have a unique ID, even across resource types.
static COUNTER: AtomicU64 = AtomicU64::new(1);

/// Allocates a new globally unique identifier.
///
/// Guaranteed to be unique across all cores in the system,
/// and monotonically increasing. This function is lock-free.
///
/// Guaranteed never to return 0.
#[inline]
pub fn allocate() -> u64 {
	COUNTER.fetch_add(1, Ordering::Relaxed)
}
