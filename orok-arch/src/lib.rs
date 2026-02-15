#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]
#![feature(type_alias_impl_trait)]

#[cfg(target_arch = "aarch64")]
use orok_arch_aarch64::Arch;
use orok_arch_base::{self as base, Arch as BaseArch, CheckUnsafePhys, CheckUnsafeVirt};
#[cfg(target_arch = "riscv64")]
use orok_arch_riscv64::Arch;
#[cfg(target_arch = "x86_64")]
use orok_arch_x86_64::Arch;

#[cfg(not(any(
	target_arch = "x86_64",
	target_arch = "aarch64",
	target_arch = "riscv64"
)))]
compile_error!("unsupported architecture selected for orok-arch");

#[doc(hidden)]
macro_rules! impl_tait {
	($alias:ty, $concrete:ty) => {
		const _: () = {
			#[define_opaque($alias)]
			const fn _impl_tait(val: $concrete) -> $alias {
				val
			}
		};
	};
}

/// An unsafe physical address type for the architecture.
pub type UnsafePhys = impl base::UnsafePhys;
impl_tait!(UnsafePhys, <Arch as BaseArch>::UnsafePhys);
/// An unsafe virtual address type for the architecture.
pub type UnsafeVirt = impl base::UnsafeVirt;
impl_tait!(UnsafeVirt, <Arch as BaseArch>::UnsafeVirt);

/// A valid physical address.
///
/// The architecture ensures that all [`Phys`]'s are valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Phys(UnsafePhys);

impl Phys {
	/// Creates a new validated [`Phys`] from an [`UnsafePhys`].
	///
	/// # Errors
	/// Returns an error if the address is not valid for the architecture.
	#[inline]
	pub fn new(value: UnsafePhys) -> Result<Self, <UnsafePhys as CheckUnsafePhys>::Error> {
		value.check_phys()?;
		Ok(Self(value))
	}
}

/// A valid virtual address.
///
/// The architecture ensures that all [`Virt`]'s are valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Virt(UnsafeVirt);

impl Virt {
	/// Creates a new validated [`Virt`] from an [`UnsafeVirt`].
	///
	/// # Errors
	/// Returns an error if the address is not valid for the architecture.
	#[inline]
	pub fn new(value: UnsafeVirt) -> Result<Self, <UnsafeVirt as CheckUnsafeVirt>::Error> {
		value.check_virt()?;
		Ok(Self(value))
	}
}
