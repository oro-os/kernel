//! Aarch64 register types and definitions.

mod mair;

pub use self::mair::{
	MairAttributes, MairCacheability, MairDeviceAttribute, MairMemoryAttributes, MairRegister,
};
