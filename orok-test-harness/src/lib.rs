#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(doc, feature(doc_cfg))]
#![feature(never_type)]
#![expect(
	clippy::indexing_slicing,
	reason = "lots of indexing into packet registers, but it's up to the caller to ensure that \
	          the packet is well-formed and has enough registers"
)]
#![expect(
	clippy::as_conversions,
	reason = "packets are frequently converted from u64 to other types, but the packet format \
	          ensures that this is always safe and not lossy"
)]

pub(crate) mod atomic;
mod handler;
mod packet;
mod state;
mod stream;

pub use self::{handler::RawPacketHandler, packet::Packet, state::*, stream::process_event_stream};
