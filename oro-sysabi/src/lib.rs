//! # Oro Kernel System ABI Structures and Functionality
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

// SAFETY(qix-): DO NOT INVOKE SYSCALLS FROM THIS CRATE.
// SAFETY(qix-): Enum match semantics for error codes are undefined when
// SAFETY(qix-): the error code is defined in the same crate (even as `#[non_exhaustive]`)
// SAFETY(qix-): and whereby the bit representation of the error code doesn't match
// SAFETY(qix-): an enum variant. It will cause differing behavior under debug vs release,
// SAFETY(qix-): too, and will result in very, very annoying to debug misbehavior.
// SAFETY(qix-):
// SAFETY(qix-): Keep this crate as a declarative reference of important system ABI values,
// SAFETY(qix-): structures and other information and reduce the amount of "functionality",
// SAFETY(qix-): please. This is not a fancy crate. That's per design.

pub(crate) mod arch;

pub mod id;
pub mod macros;
pub mod syscall;
