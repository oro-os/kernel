#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

/// A block that should result in an effect being emitted is starting.
pub const EFFECT_START: u64 = 0x1;
/// A block that should result in an effect being emitted is ending.
pub const EFFECT_END: u64 = 0x2;
/// The bootloader/kernel has started execution. Should be the very first event
/// emitted by any Oro kernel code.
pub const IN_KERNEL: u64 = 0x3;

/// (x86_64) The effect block will write to the `CR0` control register.
pub const X8664_EFFECT_WRITE_REG_CR0: u64 = 0x100;
/// (x86_64) The effect block will write to the `CR4` control register.
pub const X8664_EFFECT_WRITE_REG_CR4: u64 = 0x101;
/// (x86_64) The effect block will read from the `CR4` control register.
pub const X8664_EFFECT_READ_REG_CR4: u64 = 0x102;

/// (AArch64) The effect block will write to the `TCR_EL1` control register.
pub const AARCH_EFFECT_WRITE_REG_TCR_EL1: u64 = 0x200;
