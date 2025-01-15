//! # Oro Kernel System ABI Structures and Functionality
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

pub(crate) mod arch;

pub mod id;
pub mod macros;
pub mod syscall;
