#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

#[cfg(all(feature = "emit", not(feature = "mmio")))]
compile_error!("'emit' feature enabled without any emission backend");

#[doc(inline)]
pub use orok_test_consts as consts;
pub(crate) mod base;
mod macros;
#[cfg(feature = "mmio")]
pub use base::get_vmm_base;
pub use base::set_vmm_base;
pub use orok_test_proc::*;
