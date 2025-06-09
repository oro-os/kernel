//! Implements various register wrapper types on x86_64.

mod cr0;
mod cr4;

pub use self::{cr0::Cr0, cr4::Cr4};
