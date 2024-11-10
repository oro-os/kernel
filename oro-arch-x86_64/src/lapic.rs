//! Provides the Local APIC (Advanced Programmable Interrupt Controller)
//! implementation for the Oro kernel.
//!
//! Documentation found in Section 11 of the Intel SDM Volume 3A.

use core::fmt;

/// The LAPIC (Local Advanced Programmable Interrupt Controller (APIC))
/// controller.
pub struct Lapic {
	/// The base address of the LAPIC.
	/// Virtual and pre-translated.
	base: *mut u8,
}

// SAFETY: The pointer is valid across all cores and is thus sendable.
// SAFETY: We can guarantee that the register blocks are mapped into all
// SAFETY: cores and reside at the same location across each.
unsafe impl Send for Lapic {}

impl Lapic {
	/// Creates a new LAPIC controller.
	///
	/// # Panics
	/// Panics if the LAPIC address is not 16-byte aligned.
	///
	/// # Safety
	/// The caller must ensure that the LAPIC base address is valid and aligned.
	pub unsafe fn new(base: *mut u8) -> Self {
		assert_eq!(
			base.align_offset(16),
			0,
			"LAPIC base is not 16-byte aligned"
		);
		Self { base }
	}

	/// Returns the local APIC version.
	#[must_use]
	pub fn version(&self) -> LapicVersion {
		// SAFETY(qix-): The LAPIC base address is trusted to be vali and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		let version32 = unsafe { self.base.add(0x30).cast::<u32>().read_volatile() };
		LapicVersion {
			supports_eoi_broadcast_suppression: (version32 & (1 << 24)) != 0,
			max_lvt_entries: (version32 >> 16) as u8,
			version: version32 as u8,
		}
	}

	/// Returns the local APIC ID.
	#[must_use]
	pub fn id(&self) -> u8 {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		let id32 = unsafe { self.base.add(0x20).cast::<u32>().read_volatile() };
		(id32 >> 24) as u8
	}

	/// Sets the local APIC ID.
	pub fn set_id(&self, id: u8) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x20).cast::<u32>().read_volatile();
			let v = (v & 0x00FF_FFFF) | (u32::from(id) << 24);
			self.base.add(0x20).cast::<u32>().write_volatile(v);
		}
	}

	/// Clears the errors in the local APIC.
	pub fn clear_errors(&self) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0x280).cast::<u32>().write_volatile(0);
		}
	}

	/// Selects the secondary processor we want to interact with.
	pub fn set_target_apic(&self, apic_id: u8) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x310).cast::<u32>().read_volatile();
			let v = (v & 0x00FF_FFFF) | (u32::from(apic_id) << 24);
			self.base.add(0x310).cast::<u32>().write_volatile(v);
		}
	}

	/// Triggers an INIT IPI to the currently selected target secondary processor
	/// (selected via [`Self::set_target_apic()`]).
	pub fn send_init_ipi(&self) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x300).cast::<u32>().read_volatile();
			let v = (v & 0xFFF0_0000) | 0x00_C500;
			// let v = 0x00004500;
			self.base.add(0x300).cast::<u32>().write_volatile(v);
		}
	}

	/// Waits for the IPI to be acknowledged by the target processor.
	pub fn wait_for_ipi_ack(&self) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			while self.base.add(0x300).cast::<u32>().read_volatile() & 0x1000 != 0 {
				core::hint::spin_loop();
			}
		}
	}

	/// Deasserts the INIT IPI.
	pub fn deassert_init_ipi(&self) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x300).cast::<u32>().read_volatile();
			let v = (v & 0xFFF0_0000) | 0x00_8500;
			self.base.add(0x300).cast::<u32>().write_volatile(v);
		}
	}

	/// Sends a startup IPI to the currently selected target secondary processor
	/// (selected via [`Self::set_target_apic()`]).
	pub fn send_startup_ipi(&self, cs_page: u8) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x300).cast::<u32>().read_volatile();
			let v = (v & 0xFFF0_F800) | 0x00_0600 | u32::from(cs_page);
			// let v = 0x00004600 | cs_page as u32;
			self.base.add(0x300).cast::<u32>().write_volatile(v);
		}
	}

	/// Boots a secondary core given its LAPIC ID.
	///
	/// # Panics
	/// Panics in debug mode if the LAPIC ID is
	/// the current core's.
	pub fn boot_core(&self, apic_id: u8, cs_page: u8) {
		debug_assert_ne!(self.id(), apic_id, "boot_core() called for current core");

		self.clear_errors();
		self.set_target_apic(apic_id);
		self.send_init_ipi();
		self.wait_for_ipi_ack();
		self.set_target_apic(apic_id);
		self.deassert_init_ipi();
		self.wait_for_ipi_ack();

		// TODO(qix-): Wait 10ms.
		for _ in 0..100_000 {
			core::hint::spin_loop();
		}

		for _ in 0..2 {
			self.clear_errors();
			self.set_target_apic(apic_id);
			self.send_startup_ipi(cs_page);

			// TODO(qix-): Wait 200us.
			for _ in 0..10_000 {
				core::hint::spin_loop();
			}

			self.wait_for_ipi_ack();
		}
	}

	/// Sends an End Of Interrupt (EOI) signal to the LAPIC.
	pub fn eoi(&self) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0xB0).cast::<u32>().write_volatile(0);
		}
	}

	/// Configures the LAPIC timer.
	pub fn configure_timer(&self, config: ApicTimerConfig) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0x320).cast::<u32>().write_volatile(config.0);
		}
	}

	/// Sets the LAPIC timer divider value.
	pub fn set_timer_divider(&self, divide_by: ApicTimerDivideBy) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base
				.add(0x3E0)
				.cast::<u32>()
				.write_volatile(divide_by as u32);
		}
	}

	/// Reads the LAPIC timer's configuration.
	#[must_use]
	pub fn timer_config(&self) -> ApicTimerConfig {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			ApicTimerConfig(self.base.add(0x320).cast::<u32>().read_volatile())
		}
	}

	/// Reads the LAPIC timer's divide-by value.
	#[must_use]
	pub fn timer_divide_by(&self) -> ApicTimerDivideBy {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned,
		// SAFETY(qix-): and the transmuted bits are always valid.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			let v = self.base.add(0x3E0).cast::<u32>().read_volatile();
			let v = v & 0b1011;
			core::mem::transmute(v)
		}
	}

	/// Sets the LAPIC timer's initial count.
	pub fn set_timer_initial_count(&self, count: u32) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0x380).cast::<u32>().write_volatile(count);
		}
	}

	/// Cancels the timer.
	pub fn cancel_timer(&self) {
		// From the Intel SDM Vol. 3A 11.5.4:
		//
		// > A write of 0 to the initial-count register
		// > effectively stops the local APIC timer,
		// > in both one-shot and periodic mode
		self.set_timer_initial_count(0);
	}

	/// Reads the LAPIC timer's current count.
	#[must_use]
	pub fn timer_current_count(&self) -> u32 {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0x390).cast::<u32>().read_volatile()
		}
	}

	/// Reads the LAPIC's spurrious interrupt vector (SVR) value.
	#[must_use]
	pub fn spurious_vector(&self) -> ApicSvr {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			ApicSvr(self.base.add(0xF0).cast::<u32>().read_volatile())
		}
	}

	/// Sets the LAPIC's spurrious interrupt vector (SVR) value.
	pub fn set_spurious_vector(&self, svr: ApicSvr) {
		// SAFETY(qix-): The LAPIC base address is trusted to be valid and aligned.
		#[expect(clippy::cast_ptr_alignment)]
		unsafe {
			self.base.add(0xF0).cast::<u32>().write_volatile(svr.0);
		}
	}
}

/// A decoded LAPIC version.
#[derive(Debug, Clone)]
pub struct LapicVersion {
	/// Supports EOI broadcast suppression.
	pub supports_eoi_broadcast_suppression: bool,
	/// The maximum number of LVT entries.
	pub max_lvt_entries: u8,
	/// The LAPIC version.
	pub version: u8,
}

/// The configuration for the LAPIC timer.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ApicTimerConfig(u32);

impl fmt::Debug for ApicTimerConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ApicTimerConfig")
			.field("vector", &self.vector())
			.field("mode", &self.mode())
			.field("masked", &self.masked())
			.finish()
	}
}

impl Default for ApicTimerConfig {
	fn default() -> Self {
		Self::new()
	}
}

impl ApicTimerConfig {
	/// Creates a new LAPIC timer configuration.
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Sets the interrupt vector to be called when
	/// the timer fires.
	#[must_use]
	pub const fn with_vector(mut self, vector: u8) -> Self {
		self.0 = (self.0 & 0xFFFF_FF00) | (vector as u32);
		self
	}

	/// Marks the timer interrupt as masked.
	#[must_use]
	pub const fn with_masked(mut self) -> Self {
		self.0 |= 1 << 16;
		self
	}

	/// Sets the timer mode.
	#[must_use]
	pub const fn with_mode(mut self, mode: ApicTimerMode) -> Self {
		self.0 = (self.0 & 0xFFF9_FFFF) | (mode as u32);
		self
	}

	/// Gets the interrupt vector.
	#[must_use]
	pub const fn vector(self) -> u8 {
		(self.0 & 0xFF) as u8
	}

	/// Gets the timer mode. Returns `None` if the mode bits are invalid.
	#[must_use]
	pub const fn mode(self) -> Option<ApicTimerMode> {
		let bits = (self.0 >> 17) & 0b11;
		if bits == 0b11 {
			None
		} else {
			// SAFETY(qix-): The mode bits are always valid.
			Some(unsafe { core::mem::transmute::<u32, ApicTimerMode>(bits) })
		}
	}

	/// Returns whether the timer interrupt is masked.
	#[must_use]
	pub const fn masked(self) -> bool {
		(self.0 & (1 << 16)) != 0
	}
}

/// The mode of the LAPIC timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ApicTimerMode {
	/// Program count-down value in an initial-count register.
	OneShot     = 0,
	/// Program interval value in an initial-count register
	Periodic    = (0b01 << 17),
	/// Program target value in `IA32_TSC_DEADLINE` MSR
	TscDeadline = (0b10 << 17),
}

/// The LAPIC timer divide-by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ApicTimerDivideBy {
	/// Divide by 2.
	Div2   = 0b0000,
	/// Divide by 4.
	Div4   = 0b0001,
	/// Divide by 8.
	Div8   = 0b0010,
	/// Divide by 16.
	Div16  = 0b0011,
	/// Divide by 32.
	Div32  = 0b1000,
	/// Divide by 64.
	Div64  = 0b1001,
	/// Divide by 128.
	Div128 = 0b1010,
	/// Divide by 1.
	Div1   = 0b1011,
}

/// The spurious vector register (SVR) value for the LAPIC.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ApicSvr(u32);

impl Default for ApicSvr {
	fn default() -> Self {
		Self::new()
	}
}

impl ApicSvr {
	/// Creates a new LAPIC SVR value.
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Sets the LAPIC software enable bit.
	#[must_use]
	pub const fn with_software_enable(mut self) -> Self {
		self.0 |= 1 << 8;
		self
	}

	/// Sets the LAPIC spurious interrupt vector.
	#[must_use]
	pub const fn with_vector(mut self, vector: u8) -> Self {
		self.0 = (self.0 & 0xFFFF_FF00) | (vector as u32);
		self
	}

	/// Sets the focus processor bit.
	#[must_use]
	pub const fn with_focus_processor(mut self) -> Self {
		self.0 |= 1 << 9;
		self
	}

	/// Sets the EOI broadcast suppression bit
	/// (calling this *suppresses* EOI broadcast).
	#[must_use]
	pub const fn with_eoi_broadcast_suppression(mut self) -> Self {
		self.0 |= 1 << 12;
		self
	}

	/// Gets the LAPIC software enable bit.
	#[must_use]
	pub const fn software_enable(self) -> bool {
		(self.0 & (1 << 8)) != 0
	}

	/// Gets the LAPIC spurious interrupt vector.
	#[must_use]
	pub const fn vector(self) -> u8 {
		(self.0 & 0xFF) as u8
	}

	/// Gets the focus processor bit.
	#[must_use]
	pub const fn focus_processor(self) -> bool {
		(self.0 & (1 << 9)) != 0
	}

	/// Gets the EOI broadcast suppression bit.
	#[must_use]
	pub const fn eoi_broadcast_suppression(self) -> bool {
		(self.0 & (1 << 12)) != 0
	}
}

impl fmt::Debug for ApicSvr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ApicSvr")
			.field("software_enable", &self.software_enable())
			.field("vector", &self.vector())
			.field("focus_processor", &self.focus_processor())
			.field(
				"eoi_broadcast_suppression",
				&self.eoi_broadcast_suppression(),
			)
			.finish()
	}
}
