//! Implements Oro ports.

use crate::{
	tab::Tab,
	token::{NormalToken, Token},
};

/// A singular port connection.
pub struct Port {
	/// The producer side memory token.
	producer_token: Tab<Token>,
	/// The consumer side memory token.
	consumer_token: Tab<Token>,
}

impl Port {
	/// Creates a new port.
	///
	/// Returns `None` if the system is out of memory.
	#[must_use]
	pub fn new() -> Option<Self> {
		Some(Self {
			producer_token: crate::tab::get().add(Token::SlotMap(NormalToken::new_4kib(1)))?,
			consumer_token: crate::tab::get().add(Token::SlotMap(NormalToken::new_4kib(1)))?,
		})
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
}
