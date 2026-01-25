#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(doc, feature(doc_cfg))]
#![feature(never_type)]

pub(crate) mod atomic;
mod handler;
mod packet;
mod state;
mod stream;

pub use self::{handler::RawPacketHandler, packet::Packet, state::*, stream::process_event_stream};
