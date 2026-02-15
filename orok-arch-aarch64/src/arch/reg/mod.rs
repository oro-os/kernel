//! Register implementations for AArch64.

pub mod id_aa64mmfr0_el1;
pub mod tcr_el1;

pub use self::{id_aa64mmfr0_el1::IdAa64Mmfr0El1, tcr_el1::TcrEl1};
