#![no_std]

use limine::{
	BaseRevision,
	request::{HhdmRequest, ModulesRequest},
};

/// Provides Limine with a base revision of the protocol
/// that this "kernel" (in Limine terms) expects.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::new();

/// Requests the boot modules from Limine.
#[used]
static REQ_MODS: ModulesRequest = ModulesRequest::new();

/// In debug builds, stack size is very quickly exhausted. At time
/// of writing, Limine allocates 64KiB of stack space per core, but
/// this is not enough for debug builds.
///
/// Further, since there are no stack fences or automatic stack growing
/// implemented in this stage, we must ensure there's enough stack space
/// available for the debug build to avoid a stack overflow and subsequent
/// corruption of kernel memory.
///
/// Thus, we expand the stack size here, fairly substantially.
#[cfg(debug_assertions)]
#[used]
static REQ_STKSZ: limine::request::StackSizeRequest =
	limine::request::StackSizeRequest::new(16 * 1024 * 1024);

/// Requests that Limine performs a Higher Half Direct Map (HHDM)
/// of all physical memory. Provides an offset for the HHDM.
///
/// Note that the boot stage does not rely on an identity map as we
/// will overwrite certain lower-half memory mappings when implementing
/// the stubs (as prescribed by the Oro architectures Limine supports).
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::new();

/// Requests the frame buffer from Limine.
#[used]
static REQ_FB: limine::request::FramebufferRequest = limine::request::FramebufferRequest::new();

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// # Panics
/// Panics if required responses aren't populated by Limine.
#[expect(
	clippy::unwrap_used,
	clippy::panic,
	reason = "kernel has no choice but to panic in this function"
)]
pub unsafe fn init() -> ! {
	let _offs: u64 = REQ_HHDM.response().unwrap().offset;

	let mods = REQ_MODS.response().unwrap().modules();
	let mut ramdisk_module = None;
	for module in mods {
		if module.data().is_empty() {
			continue;
		}

		if module.path().ends_with(".rd") {
			if ramdisk_module.replace(module).is_some() {
				panic!("multiple ramdisks found");
			}
		}
	}

	let Some(ramdisk_module) = ramdisk_module else {
		panic!("no ramdisk found");
	};

	let ramdisk = match oro_ramdisk::Decoder::new(ramdisk_module.data()) {
		Ok(decoder) => decoder,
		Err(err) => panic!("failed to decode ramdisk: {err:?}"),
	};

	let mut kernel_item = None;

	for (i, item) in ramdisk.index_items().iter().enumerate() {
		if item.is_a::<oro_ramdisk::item::Kernel>() {
			let item = match ramdisk.item::<oro_ramdisk::item::Kernel>(i) {
				Ok(item) => item,
				Err(err) => panic!("failed to get kernel item from ramdisk at index {i}: {err:?}"),
			};

			let correct_arch: Option<bool> = None;

			#[cfg(target_arch = "aarch64")]
			let correct_arch = Some(item.flags.arch == oro_ramdisk::item::KernelArch::Aarch64);
			#[cfg(target_arch = "riscv64")]
			let correct_arch = Some(item.flags.arch == oro_ramdisk::item::KernelArch::Riscv64);
			#[cfg(target_arch = "x86_64")]
			let correct_arch = Some(item.flags.arch == oro_ramdisk::item::KernelArch::X86_64);

			let correct_arch = correct_arch.expect(
				"failed to determine correct architecture for kernel item (arch not supported)",
			);

			if correct_arch {
				kernel_item = Some(item);
				break;
			}
		}
	}

	let kernel_item =
		kernel_item.expect("no suitable kernel found for current architecture in ramdisk");
	let kernel_decompression_stream = kernel_item.decompression_stream();
	match kernel_decompression_stream {
		Some(Ok(stream)) => {
			// Kernel is compressed; we have to copy it.
		}
		Some(Err(err)) => {
			// Kernel is compressed but creation of the decompression stream failed.
			panic!("failed to create decompression stream for kernel: {err:?}");
		}
		None => {
			// Kernel is not compressed; perform a copy.
		}
	}

	panic!();
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
	// TODO
	loop {
		core::hint::spin_loop();
	}
}

#[cfg(test)]
fn main() {
	panic!("Don't run this directly.");
}
