//! x86_64-specific kernel interfaces.

use oro_kernel::{iface::kernel::KernelInterface, table::Table};
use oro_kernel_mem::alloc::boxed::Box;

/// Registers the x86_64-specific kernel interfaces.
pub(crate) fn register_kernel_interfaces(table: &mut Table<Box<dyn KernelInterface<crate::Arch>>>) {
	macro_rules! register_all {
		($($id:ident => $iface_mod:literal $iface:ident),* $(,)?) => {
			$({
				#[path = $iface_mod]
				mod ifacemod;

				table.insert(::oro::id::iface::$id, Box::new(ifacemod::$iface));
			})*
		};
	}

	register_all! {
		KERNEL_X86_64_TLS_BASE_V0 => "tls_base_v0.rs" TlsBaseV0,
	}
}
