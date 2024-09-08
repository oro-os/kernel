//! Provides the Local APIC (Advanced Programmable Interrupt Controller)
//! implementation for the Oro kernel.
//!
//! Documentation found in Section 11 of the Intel SDM Volume 3A.

/// The LAPIC (Local Advanced Programmable Interrupt Controller (APIC))
/// controller.
pub struct Lapic {
	/// The base address of the LAPIC.
	/// Virtual and pre-translated.
	base: *mut u8,
}

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
		for _ in 0..1_000_000 {
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
