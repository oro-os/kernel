//! AArch64 abstraction layer for the Oro operating system kernel.
//!
//! All functionality in this crate is AArch64 specific but entirely
//! _platform_ agnostic. Oro-specific functionality should go
//! into `oro-kernel-arch-aarch64`.
#![no_std]
#![cfg_attr(doc, feature(doc_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod reg;
