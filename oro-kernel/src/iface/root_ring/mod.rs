//! Root ring interfaces, exposed as non-kernel interfaces only
//! at the root ring.

#[cfg(feature = "boot-vbuf-v0")]
pub mod boot_vbuf_v0;
pub mod debug_out_v0;
pub mod test_ports;
