//! Maps TAIT traits for individual architecture implementations.
//!
//! These mappings are used to build higher level abstractions in `orok-arch`.

#[cfg(target_arch = "aarch64")]
use orok_arch_aarch64::Arch;
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
macro_rules! impl_type {
    (for $arch:ty {
        $(use $base_ty:ident :: $assoc_ty:ident as $trait_ty:ident);* $(;)?
    }) => {
        $(
            pub type $trait_ty = impl orok_arch_base::$trait_ty;
            const _: () = {
                #[doc(hidden)]
                #[define_opaque($trait_ty)]
                const fn _assoc_ty(_: $arch) -> $trait_ty {
                    let u = ::core::mem::MaybeUninit::<<$arch as orok_arch_base::$base_ty>::$assoc_ty>::uninit();
                    unsafe { u.assume_init() }
                }
            };
        )*
    };
}

impl_type! {
	for Arch {
		use ArchAddressScheme::UnsafePhys as UnsafePhys;
		use ArchAddressScheme::UnsafeVirt as UnsafeVirt;
	}
}
