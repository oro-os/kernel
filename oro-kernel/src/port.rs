//! Implements Oro ports.

use oro_mem::phys::PhysAddr;

use crate::{
	tab::Tab,
	token::{NormalToken, SlotMapEndpoint, Token},
};

/// A singular port connection.
pub struct Port {
	/// The producer side memory token.
	producer_token: Tab<Token>,
	/// The consumer side memory token.
	consumer_token: Tab<Token>,
	/// The base address of the producer page.
	///
	/// This is a *volatile* page, exactly 4096 bytes in size (512 `u64`s).
	/// This pointer is guaranteed to be valid for the lifetime
	/// of this `Port`.
	///
	/// **This may be the same as `consumer_page`!**
	producer_page:  *mut u64,
	/// The base address of the consumer page.
	///
	/// This is a *volatile* page, exactly 4096 bytes in size (512 `u64`s).
	/// This pointer is guaranteed to be valid for the lifetime
	/// of this `Port`.
	///
	/// **This may be the same as `producer_page`!**
	consumer_page:  *mut u64,
}

impl Port {
	/// Creates a new port.
	///
	/// Returns `None` if the system is out of memory.
	#[must_use]
	pub fn new() -> Option<Self> {
		let (producer_phys, producer_tab) = {
			let mut t = NormalToken::new_4kib(1);
			let phys = t.get_or_allocate(0)?;
			let t = Token::SlotMap(t, SlotMapEndpoint::Producer);
			let tab = crate::tab::get().add(t)?;
			(phys, tab)
		};

		let (consumer_phys, consumer_tab) = {
			let mut t = NormalToken::new_4kib(1);
			let phys = t.get_or_allocate(0)?;
			let t = Token::SlotMap(t, SlotMapEndpoint::Consumer);
			let tab = crate::tab::get().add(t)?;
			(phys, tab)
		};

		let this = Self {
			producer_token: producer_tab,
			consumer_token: consumer_tab,
			// SAFETY: We just allocated these pages, and they're guaranteed aligned to a u64, so they are valid.
			producer_page:  unsafe { producer_phys.as_mut_ptr_unchecked::<u64>() },
			// SAFETY: We just allocated these pages, and they're guaranteed aligned to a u64, so they are valid.
			consumer_page:  unsafe { consumer_phys.as_mut_ptr_unchecked::<u64>() },
		};

		// Zero out the pages.
		// SAFETY: We just allocated these pages, so they're guaranteed to exist.
		unsafe {
			this.producer_page.cast::<u8>().write_bytes(0, 4096);
			this.consumer_page.cast::<u8>().write_bytes(0, 4096);
		}

		Some(this)
	}

	/// Gets the producer side memory token for this port.
	#[must_use]
	pub fn producer(&self) -> Tab<Token> {
		self.producer_token.clone()
	}

	/// Gets the consumer side memory token for this port.
	#[must_use]
	pub fn consumer(&self) -> Tab<Token> {
		self.consumer_token.clone()
	}

	/// Advances the consumer and producer pages.
	pub fn advance(&self) {
		let _ = self.producer_page;
		let _ = self.consumer_page;
		todo!("advance");
	}
}
