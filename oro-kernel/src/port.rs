//! Implements Oro ports.

use core::sync::atomic::{AtomicU64, Ordering::SeqCst};

use oro::key;
use oro_mem::{
	global_alloc::GlobalPfa,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use crate::{tab::Tab, token::Token};

/// "Internal" state of a port.
pub struct PortState {
	/// The physical page belonging to the producer.
	///
	/// **This may be the same as `consumer_page`!**
	producer_phys:  Phys,
	/// The physical page belonging to the consumer.
	///
	/// **This may be the same as `producer_page`!**
	consumer_phys:  Phys,
	/// The producer's current tab index, or `0` if the producer is not active.
	producer_index: AtomicU64,
	/// The consumer's current tab index, or `0` if the consumer is not active.
	consumer_index: AtomicU64,
}

impl PortState {
	/// Creates a new port.
	///
	/// Returns `None` if the system is out of memory.
	#[must_use]
	pub fn new() -> Option<Tab<Self>> {
		// SAFETY: We're allocating the page right as we're constructing the `Phys`.
		let producer_phys = unsafe { Phys::from_address_unchecked(GlobalPfa.allocate()?) };
		// SAFETY: We're allocating the page right as we're constructing the `Phys`.
		let consumer_phys = unsafe {
			Phys::from_address_unchecked(GlobalPfa.allocate().or_else(|| {
				GlobalPfa.free(producer_phys.address_u64());
				None
			})?)
		};

		// Zero out the pages.
		// SAFETY: We just allocated these pages, so they're guaranteed to exist.
		// SAFETY: Further, it's always going to be aligned to a u8.
		// SAFETY: Lastly, these writes have exclusive access to the memory.
		unsafe {
			producer_phys
				.as_mut_ptr_unchecked::<u8>()
				.write_bytes(0, 4096);
			consumer_phys
				.as_mut_ptr_unchecked::<u8>()
				.write_bytes(0, 4096);
		}

		// Make sure all cores see the zero.
		::core::sync::atomic::fence(SeqCst);

		crate::tab::get()
			.add(Self {
				producer_phys,
				consumer_phys,
				producer_index: AtomicU64::new(0),
				consumer_index: AtomicU64::new(0),
			})
			.or_else(|| {
				// Free the pages; the Tab allocation failed.
				// SAFETY: We had just allocated it; we can free it safely.
				unsafe {
					GlobalPfa.free(producer_phys.address_u64());
					GlobalPfa.free(consumer_phys.address_u64());
				}

				None
			})
	}

	/// Tries to create a port endpoint from the given `PortState`.
	///
	/// Returns `Some(Ok(tab))` if the endpoint was successfully created and is thus
	/// unused elsewhere, or `Some(Err(tab))` if the endpoint already exists and is
	/// still live.
	///
	/// Returns `None` if the system is out of memory.
	///
	/// **This is a relatively slow operation; do not call in tight loops!**
	#[must_use]
	pub fn endpoint(state: &Tab<Self>, end: PortEnd) -> Option<Result<Tab<Token>, Tab<Token>>> {
		state.with(|this| {
			let index_ref = match end {
				PortEnd::Producer => &this.producer_index,
				PortEnd::Consumer => &this.consumer_index,
			};

			let mut current_index = index_ref.load(SeqCst);

			loop {
				if current_index != 0 {
					// Is it still live?
					if let Some(existing_tab) = crate::tab::get().lookup(current_index) {
						return Some(Err(existing_tab));
					}
				}

				let tab = crate::tab::get().add(Token::PortEndpoint(PortEndpointToken {
					state: state.clone(),
					end,
				}))?;

				let id = tab.id();

				if let Err(existing_index) =
					index_ref.compare_exchange(current_index, id, SeqCst, SeqCst)
				{
					// Another thread got to it first; check the liveness again.
					current_index = existing_index;
				} else {
					// We got it!
					return Some(Ok(tab));
				}
			}
		})
	}
}

impl Drop for PortState {
	fn drop(&mut self) {
		// Make sure that, somehow, there are no active endpoints.
		// Given the design of tabs, this should never be the case.
		// However, it's still a good idea to check.
		debug_assert!(
			{
				let v = self.producer_index.load(SeqCst);
				v == 0 || crate::tab::get().lookup_any(v).is_none()
			},
			"producer endpoint still active"
		);
		debug_assert!(
			{
				let v = self.consumer_index.load(SeqCst);
				v == 0 || crate::tab::get().lookup_any(v).is_none()
			},
			"consumer endpoint still active"
		);

		// SAFETY: We're freeing the pages right as we're dropping the `Phys`.
		unsafe {
			GlobalPfa.free(self.producer_phys.address_u64());
			GlobalPfa.free(self.consumer_phys.address_u64());
		}
	}
}

/// A slot map endpoint - either producer or consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PortEnd {
	/// The producer side of the slot map.
	Producer = key!("producer"),
	/// The consumer side of the slot map.
	Consumer = key!("consumer"),
}

/// A port endpoint token.
pub struct PortEndpointToken {
	/// The internal port state tab.
	state: Tab<PortState>,
	/// Which end the endpoint is.
	end:   PortEnd,
}

impl PortEndpointToken {
	/// Returns the [`PortEnd`] of the endpoint.
	#[inline]
	#[must_use]
	pub fn side(&self) -> PortEnd {
		self.end
	}

	/// Returns the [`Phys`] address of the endpoint's page.
	#[inline]
	#[must_use]
	pub fn phys(&self) -> Phys {
		match self.end {
			PortEnd::Producer => self.state.with(|s| s.producer_phys),
			PortEnd::Consumer => self.state.with(|s| s.consumer_phys),
		}
	}

	/// Advances the port's internal copy state.
	///
	/// For direct-mapped ports, this is a no-op.
	///
	/// This is also a no-op for producers.
	pub fn advance(&self) {
		if self.end == PortEnd::Producer {
			return;
		}

		self.state.with(|st| {
			if st.consumer_phys == st.producer_phys {
				// Direct-mapped port; no need to advance.
				return;
			}

			todo!("advance!");
		});
	}
}
