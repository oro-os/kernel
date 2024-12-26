//! # Oro Kernel System ABI Structures and Functionality
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

pub(crate) mod arch;

pub mod syscall;
pub mod table;

/// Special entity ID that can be used to refer to the
/// currently running thread.
///
/// This is the equivalent of an entity ID with all bits
/// set high.
pub const THIS_THREAD: u64 = !0;
