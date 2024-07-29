//! Provides extension traits and implementations for common, high-level
//! operations on [`syn`] types.

mod attr;
mod repr;

pub use self::{
	attr::Attributes,
	repr::{ReprArgs, ReprAttribute},
};
