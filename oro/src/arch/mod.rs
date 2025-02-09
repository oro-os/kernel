//! Architecture-specific stubs or wrapper calls.

#[cfg(target_arch = "x86_64")]
#[path = "x86_64.rs"]
pub mod imp;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
pub mod imp;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("unsupported target architecture");

#[allow(unused_imports)]
pub(crate) use imp::*;
