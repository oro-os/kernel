//! Implements the cr4 register on x86_64.
#![expect(clippy::inline_always)]

use oro_kernel_macro::bitstruct;

bitstruct! {
	/// The CR4 register on X86_64.
	pub struct Cr4(u64) {
		/// Virtual 8086 mode extensions.
		///
		/// If set, enables support for the virtual interrupt flag (VIF) in virtual-8086 mode.
		pub vme[0] => as bool,
		/// Protected-mode virtual interrupts.
		///
		/// If set, enables support for the virtual interrupt flag (VIF) in protected mode.
		pub pvi[1] => as bool,
		/// Time stamp disable.
		///
		/// If set, **disables** the RDTSC instruction (inverted logic).
		pub tsd[2] => as bool,
		/// Debugging extensions.
		///
		/// If set, enables the use of the `DE` flag in CR0, allowing
		/// for the use of debug register based breaks on I/O space access.
		pub de[3] => as bool,
		/// Page size extensions.
		///
		/// If set, enables the use of 4MB huge pages.
		/// Ignored if PAE is enabled, or in 64-bit mode
		/// (which is always the case on x86_64).
		pub pse[4] => as bool,
		/// Physical address extension.
		///
		/// If set, enables the use of 36-bit physical addresses.
		pub pae[5] => as bool,
		/// Machine check exception.
		///
		/// If set, enables the machine check exception.
		pub mce[6] => as bool,
		/// Page global enable.
		///
		/// If set, enables the global bit in page tables.
		pub pge[7] => as bool,
		/// Performance monitoring counter enable.
		///
		/// If set, enables the use of the performance monitoring counters
		/// from any protection level. If unset, they can only be used in
		/// ring 0.
		pub pce[8] => as bool,
		/// Operating system support for FXSAVE and FXRSTOR instructions.
		///
		/// If set, enables the use of the `FXSAVE` and `FXRSTOR` instructions.
		pub osfxsr[9] => as bool,
		/// Operating system support for unmasked SIMD floating point exceptions.
		///
		/// If set, enables unmasked SSE exceptions.
		pub osxmmexcpt[10] => as bool,
		/// User-mode instruction prevention.
		///
		/// If set, the SGDT, SIDT, SLDT, SMSW and STR instructions cannot be executed if CPL > 0.
		pub umip[11] => as bool,
		/// 57-bit linear addresses.
		///
		/// If set, enables 5-level paging.
		pub la57[12] => as bool,
		/// Virtual machine extensions enable.
		///
		/// If set, enables the use of Intel VT-x x86 virtualization.
		pub vmxe[13] => as bool,
		/// Safer mode extensions enable.
		///
		/// If set, enables the use of Trusted Execution Technology (TXT).
		pub smxe[14] => as bool,

		// NOTE(qix): Bit 15 is reserved and intentionally skipped.

		/// FSGSBASE enable.
		///
		/// If set, enables the use of the RDFSBASE, RDGSBASE, WRFSBASE, and WRGSBASE instructions.
		// TODO(qix-): Mark this as `unsafe`, which will require a mod in `bitstruct!`.
		pub fsgsbase[16] => as bool,
		/// PCID enable.
		///
		/// If set, enables the use of process-context identifiers (PCIDs).
		pub pcide[17] => as bool,
		/// XSAVE and processor extended states enable.
		///
		/// If set, enables the use of XSAVE and processor extended states.
		pub osxsave[18] => as bool,
		/// Key locker enable.
		///
		/// If set, enables the use of the AES key locker instructions.
		pub kl[19] => as bool,
		/// Supervisor mode execution protection enable.
		///
		/// If set, execution of code in a higher ring generates a fault.
		pub smep[20] => as bool,
		/// Supervisor mode access prevention enable.
		///
		/// If set, access of data in a higher ring generates a fault.
		pub smap[21] => as bool,
		/// Protection key enable.
		///
		/// If set, enables the use of protection keys.
		pub pke[22] => as bool,
		/// Control-flow enforcement technology.
		///
		/// If set, enables the use of control-flow enforcement technology.
		pub cet[23] => as bool,
		/// Enable protection keys for supervisor-mode pages.
		///
		/// If set, enables the use of protection keys for supervisor-mode pages.
		pub pks[24] => as bool,
	}
}

impl Cr4 {
	/// Loads the CR4 register.
	#[inline(always)]
	#[must_use]
	pub fn load() -> Self {
		let cr4: u64;
		unsafe {
			core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nostack, nomem, preserves_flags));
		}
		Self(cr4)
	}

	/// Stores the CR4 register.
	///
	/// # Safety
	/// Improperly configuring the CR4 register can lead to undefined behavior,
	/// since many instructions and features depend on the CR4 register, including
	/// certain aspects of the x86_64 memory layout (e.g. page size extensions, and
	/// the la57 bit).
	#[inline(always)]
	pub unsafe fn store(self) {
		unsafe {
			core::arch::asm!("mov cr4, {}", in(reg) self.0, options(nostack, nomem, preserves_flags));
		}
	}
}
