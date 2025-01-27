//! Architecture-specific stubs or wrapper calls.

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("unsupported target architecture");
