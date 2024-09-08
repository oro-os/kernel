//! Provides abstractions for certain x86_64 registers.

use core::arch::asm;

/// The CR0 register.
#[repr(transparent)]
pub struct Cr0(u64);

#[expect(clippy::missing_docs_in_private_items)]
macro_rules! field {
	($name:ident, $shift:expr, $doc:literal) => {
		#[doc = $doc]
		#[must_use]
		pub const fn $name(self) -> Self {
			Self(self.0 | (1 << $shift))
		}
	};
}

impl Cr0 {
	field!(with_protected_mode_enable, 0, "Enables protected mode.");

	field!(
		with_monitor_coprocessor,
		1,
		"Enables the coprocessor monitor bit, which determines if WAIT/FWAIT causes an exception \
		 (CR0.MP=1, CR0.TS=1) or not (CR0.MP=0 _OR_ CR0.TS=0)."
	);

	field!(
		with_emulation,
		2,
		"Enables emulation. If 1, #UD is raised when executing x87 instructions such that they \
		 can be emulated."
	);

	field!(
		with_task_switch,
		3,
		"Enables context-saving on x87-related task switching. CR0.MP controls if WAIT/FWAIT \
		 causes an #NM when CR0.TS=1."
	);

	// NOTE: Extension type bit is skipped; it's now reserved and for x86_64 we'll treat it as such.
	field!(
		with_write_protect,
		16,
		"Enables write protection for read-only pages at the supervisor level."
	);

	field!(
		with_alignment_mask,
		18,
		"Enables alignment checking. Under CPL3, alignment checks are performed on every data \
		 reference and raise #AC if they fail."
	);

	// NOTE: Not-writethrough is ignored by the CPU.
	field!(
		with_cache_disable,
		30,
		"When DISABLED, the CPU cache is ENABLED. When disabled (CR0.CD=1), the CPU does not \
		 bring new instructions/data into caches."
	);

	field!(
		with_paging_enable,
		31,
		"Enables paging. If disabled, the CPU operates in real mode."
	);

	/// Creates a new CR0 register with all bits cleared.
	#[expect(clippy::new_without_default)]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Returns a mask of all supported bits.
	/// ANDing this with an existing CR0 value will retain
	/// all unsupported bits and zero the supported bits
	/// such that the value can be OR'd with new bits.
	#[must_use]
	pub const fn mask() -> u64 {
		!Self::new()
			.with_protected_mode_enable()
			.with_monitor_coprocessor()
			.with_emulation()
			.with_task_switch()
			.with_write_protect()
			.with_alignment_mask()
			.with_cache_disable()
			.with_paging_enable()
			.0
	}

	/// Returns the raw bits of the CR0 register.
	// TODO(qix-): When const traits are stabilized, remove this
	// TODO(qix-): in lieu of a const `From` trait impl.
	#[must_use]
	pub const fn bits(self) -> u64 {
		self.0
	}

	/// Inherits any unused bits from the current CR0 register.
	///
	/// Should be called after setting any new bits.
	///
	/// # Safety
	/// Interrupts should be disabled before calling this
	/// function if the value is to be immediately loaded,
	/// in order to make sure no race conditions occur.
	#[must_use]
	pub unsafe fn inherit(mut self) -> Self {
		let mut current = 0;
		asm!("mov {}, cr0", inout(reg) current);
		self.0 |= current & Self::mask();
		self
	}

	/// Loads the CR0 register.
	pub fn load(self) {
		// SAFETY(qix-): This is safe because the CR0 register is a
		// SAFETY(qix-): well-defined register in the x86_64 architecture.
		// SAFETY(qix-): One might argue this is unsafe because it can crash
		// SAFETY(qix-): the kernel but there's nothing regarding the `unsafe`
		// SAFETY(qix-): keyword that can prevent that.
		unsafe {
			asm!("mov cr0, {}", in(reg) self.0);
		}
	}

	/// Gets the current CR0 register.
	#[must_use]
	pub fn read() -> Self {
		// SAFETY(qix-): This is always safe.
		let cr0: u64;
		unsafe {
			asm!("mov {}, cr0", out(reg) cr0);
		}
		Self(cr0)
	}
}

impl From<Cr0> for u64 {
	fn from(cr0: Cr0) -> u64 {
		cr0.0
	}
}

impl From<u64> for Cr0 {
	fn from(val: u64) -> Cr0 {
		Cr0(val)
	}
}
