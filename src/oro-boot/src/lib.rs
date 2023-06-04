#![no_std]

pub mod x86_64;
pub use oro_ser2mem::{Allocator, Fake, Proxy, Serialize};

pub const BOOT_MAGIC: u64 = u64::from_be_bytes(*b"ORO_BOOT");
