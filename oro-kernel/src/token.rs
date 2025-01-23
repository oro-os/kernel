//! Memory token metdata information.
//!
//! Memory tokens are an abstract identifier that refers to
//! some sort of memory provided by the kernel, be it general
//! memory, priority memory (i.e. for DMA), or memory-mapped
//! I/O (MMIO) regions, either for CPU regisers or for devices
//! (e.g. through PCI).
//!
//! Tokens are non-specific IDs that have a resource number just
//! like ring, instance and thread IDs. Applications use them
//! to specify things like passing them around via ports, or
//! to map them directly into their address space.
//!
//! It's important to remember that tokens have no attached
//! access restrictions; that is up to the interfaces to implement.
//! This allows some flexibility, but can be used unsafely.
//! **Please exercise extreme caution when using tokens.**

use oro_mem::{
	alloc::vec::Vec,
	global_alloc::GlobalPfa,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};
use oro_sysabi::key;

/// A singular memory token. See module level documentation for more information.
// SAFETY(qix-): Do not change discriminant values. Only add new ones.
// SAFETY(qix-): Further, this enum MUST be `repr(u64)` to ensure that the
// SAFETY(qix-): discriminant is a `u64` and can be read as such.
#[repr(u64)]
pub enum Token {
	/// A [`NormalToken`] memory token. Represents one or more physical pages.
	// NOTE(qix-): Do not use `0` here, as it's used by the interfaces
	// NOTE(qix-): as a sentinel value.
	Normal(NormalToken) = key!("normal"),
}

impl Token {
	/// Returns the type ID of the token.
	#[must_use]
	#[inline]
	pub fn type_id(&self) -> u64 {
		// SAFETY: (From the `core` documentation for [`core::mem::discriminant`])
		// SAFETY:
		// SAFETY: Because `Self` is marked `repr(u64)`, its layout is a `repr(C)` `union`
		// SAFETY: between `repr(C)` structs, each of which has the `u64` discriminant as its first
		// SAFETY: field, so we can read the discriminant without offsetting the pointer.
		unsafe { *<*const _>::from(self).cast::<u64>() }
	}
}

/// Normal memory token metadata.
pub struct NormalToken {
	/// The size of each of the pages, in bytes.
	///
	/// Right now, this is always 4096.
	page_size:  usize,
	/// The physical pages allocated to this token.
	///
	/// Values are initial `None`. Architectures can either
	/// pre-emptively allocate the pages, or implement
	/// lazy allocation.
	phys_addrs: Vec<Option<Phys>>,
	/// Committed count. This is cached here, but is
	/// the equivalent of `self.phys_addrs.iter().filter(|x| x.is_some()).count()`.
	committed:  usize,
}

impl NormalToken {
	/// Creates a new normal token with the given number of pages.
	///
	/// The pages are not allocated.
	#[must_use]
	pub(crate) fn new_4kib(pages: usize) -> Self {
		Self {
			page_size:  4096,
			phys_addrs: Vec::with_capacity(pages),
			committed:  0,
		}
	}

	/// Returns the size of each page in bytes.
	#[must_use]
	#[inline]
	pub fn page_size(&self) -> usize {
		self.page_size
	}

	/// Returns the number of pages allocated to this token.
	#[must_use]
	#[inline]
	pub fn page_count(&self) -> usize {
		self.phys_addrs.len()
	}

	/// Returns the total number of bytes allocated to this token.
	///
	/// Equivalent of `self.page_size() * self.page_count()`.
	#[must_use]
	#[inline]
	pub fn size(&self) -> usize {
		self.page_size() * self.page_count()
	}

	/// Returns the physical address of the page at the given index.
	///
	/// Returns `None` if the page is not allocated.
	///
	/// # Panics
	/// Panics if the index is out of bounds.
	#[inline]
	#[must_use]
	pub fn get(&self, idx: usize) -> Option<Phys> {
		self.phys_addrs
			.get(idx)
			.copied()
			.expect("index out of bounds")
	}

	/// Returns the number of pages committed to this token.
	///
	/// This is the number of pages that have been allocated and currently
	/// back the token. Not all pages may be allocated at once,
	/// nor are they guaranteed to be contiguous.
	#[must_use]
	#[inline]
	pub fn commit(&self) -> usize {
		debug_assert!(
			self.committed <= self.phys_addrs.len(),
			"commit count is higher than allocated pages"
		);

		self.committed
	}

	/// Returns the physical address of the page at the given index,
	/// **or allocates** it if it is not already allocated.
	///
	/// Returns an error if the allocation failed (out of memory scenario).
	///
	/// # Panics
	/// Panics if the index is out of bounds.
	#[must_use]
	pub fn get_or_allocate(&mut self, idx: usize) -> Option<Phys> {
		let entry = self.phys_addrs.get_mut(idx).expect("index out of bounds");

		if let Some(phys) = *entry {
			Some(phys)
		} else {
			let phys = GlobalPfa.allocate()?;
			// SAFETY: We just allocated it; we can guarantee it's valid.
			let phys = unsafe { Phys::from_address_unchecked(phys) };
			entry.replace(phys);
			self.committed += 1;
			Some(phys)
		}
	}
}
