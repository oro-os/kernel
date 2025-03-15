//! Interrupt handling for x86_64 architecture.
#![expect(rustdoc::private_intra_doc_links)]

use core::{cell::UnsafeCell, mem::MaybeUninit};

use idt::IdtEntry;
use oro_kernel::event::{InvalidInstruction, PageFault, PageFaultAccess, PreemptionEvent};
use oro_sync::{Lock, Mutex};

pub mod default;
pub mod idt;
pub mod install;
pub mod isr;

/// The static IDT, used by all cores.
static IDT: Mutex<Option<UnsafeCell<[idt::IdtEntry; 256]>>> = Mutex::new(None);

/// Initializes the default IDT if it isn't already.
fn initialize_default_idt() {
	let mut idt = IDT.lock();

	if idt.is_none() {
		let new_idt = default::new_default();

		// SAFETY: We can guarantee the memory is valid and we're the only
		// SAFETY: ones writing to it.
		unsafe {
			core::ptr::write_volatile(&mut *idt, Some(UnsafeCell::new(new_idt)));
		}
	}

	assert!(idt.is_some());
}

/// Initializes and installs default interrupt handling for the x86_64 architecture.
///
/// This IDT isn't modifiable; it's only useful for early-stage booting. A core local
/// [`Idt`] should be created and installed for the local core as soon as possible.
///
/// # Safety
/// See [`install::install_idt`] for safety considerations.
#[expect(clippy::missing_panics_doc)]
pub unsafe fn install_default() {
	initialize_default_idt();

	// SAFETY: Safety considerations offloaded to caller.
	// SAFETY: Further, we can guarantee that the unsafe cell
	// SAFETY: is only ever referenced immutably.
	unsafe {
		install::install_idt(&*IDT.lock().as_ref().unwrap().get());
	}
}

/// A core-local IDT.
pub struct Idt {
	/// The IDT entries.
	entries: [IdtEntry; 256],
}

impl Idt {
	/// Creates a new IDT, usable by the local core.
	#[expect(clippy::missing_panics_doc)]
	pub fn new() -> Self {
		initialize_default_idt();

		// Just copy from the global default IDT.
		Self {
			// SAFETY: This is always safe since we've initialized
			// SAFETY: the global IDT.
			entries: unsafe { *IDT.lock().as_ref().unwrap().get() },
		}
	}

	/// Installs the IDT.
	///
	/// # Safety
	/// See [`install::install_idt`] for safety considerations.
	pub unsafe fn install(&'static self) {
		install::install_idt(&self.entries);
	}
}

/// A stack frame for an interrupt handler.
#[expect(missing_docs)]
#[derive(Debug)]
#[repr(C, align(8))]
pub struct StackFrame {
	/// May not be fully initialized; do NOT inspect this data.
	/// It's for the stubs to use, and is a maximum bound for the
	/// size needed with full AVX-512 support.
	pub zmm:    MaybeUninit<[[u64; 8]; 32]>,
	// NOTE(qix-): Following fields MUST total a multiple of 64
	// NOTE(qix-): or else the ZMMn stores will fault.
	pub gsbase: u64,
	pub fsbase: u64,
	pub r15:    u64,
	pub r14:    u64,
	pub r13:    u64,
	pub r12:    u64,
	pub r11:    u64,
	pub r10:    u64,
	pub r9:     u64,
	pub r8:     u64,
	pub rbp:    u64,
	pub rsi:    u64,
	pub rdx:    u64,
	pub rcx:    u64,
	pub rbx:    u64,
	pub rax:    u64,
	pub rdi:    u64,
	pub iv:     u64,
	pub err:    u64,
	pub ip:     u64,
	pub cs:     u64,
	pub flags:  u64,
	pub sp:     u64,
	pub ss:     u64,
}

const _: () = {
	::oro_macro::assert::fits::<StackFrame, 4096>();
};

impl Default for StackFrame {
	#[inline]
	fn default() -> Self {
		// SAFETY: This is safe, as it's all essentially "maybe uninit" anyway.
		// SAFETY: Moreover, all fields are already safely represented by zeros.
		unsafe { core::mem::zeroed() }
	}
}
