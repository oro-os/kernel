//! Provides the Local APIC (Advanced Programmable Interrupt Controller)
//! implementation for the Oro kernel.
//!
//! Documentation found in Section 11 of the Intel SDM Volume 3A.

use core::fmt;

use oro_type::Volatile;

/// A single 32-bit, read-only LAPIC register.
#[repr(C, align(16))]
struct RO32 {
	/// The register value.
	value: Volatile<u32>,
}

impl RO32 {
	/// Reads the register value.
	#[must_use]
	fn read(&self) -> u32 {
		self.value.get()
	}
}

/// A single 32-bit, read-write LAPIC register.
#[repr(C, align(16))]
struct RW32 {
	/// The register value.
	value: Volatile<u32>,
}

impl RW32 {
	/// Reads the register value.
	#[must_use]
	fn read(&self) -> u32 {
		self.value.get()
	}

	/// Writes the register value.
	fn write(&self, value: u32) {
		self.value.set(value);
	}
}

/// A single 32-bit write-only LAPIC register.
#[repr(C, align(16))]
struct WO32 {
	/// The register value.
	value: Volatile<u32>,
}

impl WO32 {
	/// Writes the register value.
	fn write(&self, value: u32) {
		self.value.set(value);
	}
}

/// A padding entry for the LAPIC register block.
#[repr(C, align(16))]
struct Padding([u8; 16]);

/// The LAPIC register block.
///
/// # Reference
/// From the Intel SDM Volume 3, December 2024, Table 12-1 _Local APIC Register Address Map_,
/// page 398.
#[repr(C)]
#[expect(clippy::missing_docs_in_private_items)]
struct LapicRegisterBlock {
	_reserved0: [Padding; 2],
	/// Local APIC ID.
	id:         RO32,
	/// Version register.
	version:    RO32,
	_reserved1: [Padding; 4],
	/// Task Priority Register (TPR).
	tpr:        RW32,
	/// Arbitraration Priority Register (APR).
	/// Not available on all processors, so we exclude it here.
	_reserved2: Padding,
	/// Processor Priority Register (PPR).
	ppr:        RO32,
	/// EOI Register.
	eoi:        WO32,
	/// Remote read register (RRD).
	/// Not available on all processors, so we exclude it here.
	_reserved3: Padding,
	/// Logical Destination Register.
	ldr:        RW32,
	/// Destination Format Register.
	dfr:        RW32,
	/// Spurious Interrupt Vector Register.
	svr:        RW32,
	/// In-Service Register.
	///
	/// These are 32-bit registers, where index `0` is the first 32-bits (bits 31:0),
	/// index `1` is the next 32-bits (bits 63:32), and so on.
	isr:        [RO32; 8],
	/// Trigger Mode Register.
	///
	/// These are 32-bit registers, where index `0` is the first 32-bits (bits 31:0),
	/// index `1` is the next 32-bits (bits 63:32), and so on.
	tmr:        [RO32; 8],
	/// Interrupt Request Register.
	///
	/// These are 32-bit registers, where index `0` is the first 32-bits (bits 31:0),
	/// index `1` is the next 32-bits (bits 63:32), and so on.
	irr:        [RO32; 8],
	/// Error Status Register.
	esr:        RW32,
	_reserved4: [Padding; 6],
	/// LVT corrected machine check interrupt (CMCI) register.
	lvt_cmci:   RW32,
	/// Interrupt Command Register.
	///
	/// These are 32-bit registers, where index `0` is the first 32-bits (bits 31:0),
	/// index `1` is the next 32-bits (bits 63:32), and so on.
	icr:        [RW32; 2],
	/// LVT Timer Register.
	lvt_timer:  RW32,
	/// LVT Thermal Sensor Register.
	/// Not available on all processors, so we exclude it here.
	_reserved5: Padding,
	/// LVT Performance Monitoring Counters Register.
	/// Not available on all processors, so we exclude it here.
	_reserved6: Padding,
	/// LVT LINT0 Register.
	lvt_lint0:  RW32,
	/// LVT LINT1 Register.
	lvt_lint1:  RW32,
	/// LVT Error Register.
	lvt_error:  RW32,
	/// Initial Count Register.
	icr_timer:  RW32,
	/// Current Count Register.
	ccr_timer:  RO32,
	_reserved7: [Padding; 4],
	/// Divide Configuration Register.
	dcr_timer:  RW32,
	_reserved8: Padding,
}

/// The LAPIC (Local Advanced Programmable Interrupt Controller (APIC))
/// controller.
pub struct Lapic {
	/// The base address of the LAPIC register block.
	base: *const LapicRegisterBlock,
}

// SAFETY: The pointer is valid across all cores and is thus sendable.
// SAFETY: We can guarantee that the register blocks are mapped into all
// SAFETY: cores and reside at the same location across each.
unsafe impl Send for Lapic {}

impl Lapic {
	/// Creates a new LAPIC controller.
	///
	/// # Panics
	/// Panics if the LAPIC address is not properly aligned to a 16-byte boundary.
	///
	/// # Safety
	/// The caller must ensure that the LAPIC base address is valid and aligned.
	#[must_use]
	#[inline(never)]
	#[cold]
	pub unsafe fn new(base: *const u8) -> Self {
		assert!(!base.is_null(), "LAPIC base is null");
		assert_eq!(
			base.align_offset(16),
			0,
			"LAPIC base is not 16-byte aligned"
		);
		Self { base: base.cast() }
	}

	/// Returns the local APIC version.
	#[must_use]
	pub fn version(&self) -> LapicVersion {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		let version32 = unsafe { (*self.base).version.read() };
		LapicVersion {
			supports_eoi_broadcast_suppression: (version32 & (1 << 24)) != 0,
			max_lvt_entries: (version32 >> 16) as u8,
			version: version32 as u8,
		}
	}

	/// Returns the local APIC ID.
	#[must_use]
	pub fn id(&self) -> u8 {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		let id32 = unsafe { (*self.base).id.read() };
		(id32 >> 24) as u8
	}

	/// Clears the errors in the local APIC.
	pub fn clear_errors(&self) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).esr.write(0);
		}
	}

	/// Selects the secondary processor we want to interact with.
	pub fn set_target_apic(&self, apic_id: u8) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			let v = (*self.base).isr[1].read();
			let v = (v & 0x00FF_FFFF) | (u32::from(apic_id) << 24);
			(*self.base).icr[1].write(v);
		}
	}

	/// Triggers an INIT IPI to the currently selected target secondary processor
	/// (selected via [`Self::set_target_apic()`]).
	pub fn send_init_ipi(&self) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			let v = (*self.base).icr[0].read();
			let v = (v & 0xFFF0_0000) | 0x00_C500;
			// let v = 0x00004500;
			(*self.base).icr[0].write(v);
		}
	}

	/// Waits for the IPI to be acknowledged by the target processor.
	pub fn wait_for_ipi_ack(&self) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			while (*self.base).icr[0].read() & 0x1000 != 0 {
				core::hint::spin_loop();
			}
		}
	}

	/// Deasserts the INIT IPI.
	pub fn deassert_init_ipi(&self) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			let v = (*self.base).icr[0].read();
			let v = (v & 0xFFF0_0000) | 0x00_8500;
			(*self.base).icr[0].write(v);
		}
	}

	/// Sends a startup IPI to the currently selected target secondary processor
	/// (selected via [`Self::set_target_apic()`]).
	pub fn send_startup_ipi(&self, cs_page: u8) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			let v = (*self.base).icr[0].read();
			let v = (v & 0xFFF0_F800) | 0x00_0600 | u32::from(cs_page);
			// let v = 0x00004600 | cs_page as u32;
			(*self.base).icr[0].write(v);
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
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).eoi.write(0);
		}
	}

	/// Configures the LAPIC timer.
	pub fn configure_timer(&self, config: ApicTimerConfig) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).lvt_timer.write(config.0);
		}
	}

	/// Sets the LAPIC timer divider value.
	pub fn set_timer_divider(&self, divide_by: ApicTimerDivideBy) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).dcr_timer.write(divide_by as u32);
		}
	}

	/// Reads the LAPIC timer's configuration.
	#[must_use]
	pub fn timer_config(&self) -> ApicTimerConfig {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe { ApicTimerConfig((*self.base).lvt_timer.read()) }
	}

	/// Reads the LAPIC timer's divide-by value.
	#[must_use]
	pub fn timer_divide_by(&self) -> ApicTimerDivideBy {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned,
		// SAFETY: and the transmuted bits are always valid.
		unsafe {
			let v = (*self.base).dcr_timer.read();
			let v = v & 0b1011;
			core::mem::transmute(v)
		}
	}

	/// Sets the LAPIC timer's initial count.
	pub fn set_timer_initial_count(&self, count: u32) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).icr_timer.write(count);
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
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe { (*self.base).ccr_timer.read() }
	}

	/// Reads the LAPIC's spurrious interrupt vector (SVR) value.
	#[must_use]
	pub fn spurious_vector(&self) -> ApicSvr {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe { ApicSvr((*self.base).svr.read()) }
	}

	/// Sets the LAPIC's spurrious interrupt vector (SVR) value.
	pub fn set_spurious_vector(&self, svr: ApicSvr) {
		// SAFETY: The LAPIC base address is trusted to be valid and aligned.
		unsafe {
			(*self.base).svr.write(svr.0);
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
			// SAFETY: The mode bits are always valid.
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
