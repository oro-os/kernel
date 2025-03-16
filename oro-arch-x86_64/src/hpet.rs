//! High Precision Event Timer (HPET) initialization routines.
#![expect(
	dead_code,
	reason = "Some functionality isn't used but included to help document some of the spec"
)]

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

use oro_acpi::AcpiTable;
use oro_macro::bitstruct;
use oro_mem::{
	alloc::sync::Arc,
	phys::{Phys, PhysAddr},
};
use oro_sync::{Lock, Mutex};
use oro_type::Volatile;

use crate::time::GetInstant;

/// Initializes the HPET.
///
/// # Panics
/// Panics if no HPET is found.
///
/// # Safety
/// Must **only** be called by the primary core, exactly once at system boot.
#[must_use]
#[cold]
pub unsafe fn initialize() -> Arc<dyn GetInstant> {
	// Find the HPET table in the ACPI tables.
	// TODO(qix-): Once more timekeepng mechanisms are supported, don't
	// TODO(qix-): make this panic.
	let Some(hpet) = crate::boot::protocol::find_acpi_table::<oro_acpi::Hpet>() else {
		panic!("no HPET found");
	};

	assert!(
		hpet.inner_ref().Address.SpaceId.read() == 0,
		"HPET address space is not system memory"
	);

	let phys_addr = hpet.inner_ref().Address.Address.read();
	let registers = Phys::from_address_unchecked(phys_addr)
		.as_ref::<HpetRegisters>()
		.expect("virtual address of HPET base registers is misaligned");

	// Start the timer.
	registers
		.gen_cfg
		.set(registers.gen_cfg.get().with_enable_cfg(false));
	registers.main_counter.store(0, Relaxed);
	crate::asm::strong_memory_fence();
	registers
		.gen_cfg
		.set(registers.gen_cfg.get().with_enable_cfg(true));
	crate::asm::strong_memory_fence();

	let fs_per_tick = registers.caps_and_id.get().counter_clk_period();

	Arc::new(Mutex::new(HpetHandle {
		base_registers:    registers,
		counter:           0,
		femtosecond_magic: crate::time::calculate_fsns_magic(fs_per_tick),
	}))
}

/// A handle to the HPET from which instants can be fetched.
pub struct HpetHandle {
	/// A reference to the base registers of the HPET.
	base_registers:    &'static HpetRegisters,
	/// The current counter, in "fembiseconds" (see [`Self::femtosecond_magic`]).
	counter:           u128,
	/// The constant multiplier to apply to new counter
	/// values to arrive at "fembiseconds" - a power-of-two
	/// femtoseconds approximation. This value can be right-shifted
	/// by 20 to arrive at nanoseconds.
	///
	/// This is a cached value of the HPET's configuration
	/// field.
	femtosecond_magic: u128,
}

// SAFETY: We can guarantee this is `Send` as it's available system-wide.
unsafe impl Send for HpetHandle {}

impl GetInstant for Mutex<HpetHandle> {
	fn now(&self) -> crate::time::Instant {
		let mut this = self.lock();
		// Disable, read, reset, re-enable.
		let old_cfg = this.base_registers.gen_cfg.get().with_enable_cfg(true);
		this.base_registers
			.gen_cfg
			.set(old_cfg.with_enable_cfg(false));
		let new_counts = this.base_registers.main_counter.swap(0, Relaxed);
		this.base_registers.gen_cfg.set(old_cfg);
		this.counter += u128::from(new_counts);
		let ts = this.counter * this.femtosecond_magic;
		drop(this);
		crate::time::Instant::new(ts)
	}
}

/// The memory mapped register block for the HPET.
#[expect(clippy::missing_docs_in_private_items)]
#[repr(C, align(8))]
struct HpetRegisters {
	/// General capabilities and ID.
	///
	/// **Read only**
	caps_and_id:  Volatile<HpetCapsAndId>,
	_reserved0:   u64,
	/// General Configuration Register
	///
	/// **Writes to this field _must be read-modify-write_** as there are
	/// OEM-specific bits that must be preserved (even though they are
	/// deprecated).
	///
	/// **Read write**
	gen_cfg:      Volatile<HpetGeneralConfig>,
	_reserved1:   u64,
	/// General Interrupt Status Register
	///
	/// **Read write**
	gen_status:   Volatile<HpetGeneralStatus>,
	_reserved2:   [u64; 25],
	/// Main counter value register
	///
	/// **Read write**
	///
	/// - Writes to this register should only be done while the counter is halted.
	/// - 32-bit counters will always return 0 for the upper 32-bits of this register.
	/// - Reads and writes to this counter must be atomic.
	main_counter: AtomicU64,
	_reserved3:   u64,
	/// Timer 0.
	timer0:       HpetTimer,
	_reserved4:   u64,
	/// Timer 1.
	timer1:       HpetTimer,
	_reserved5:   u64,
	/// Timer 2.
	timer2:       HpetTimer,
	_reserved6:   u64,
	/// Timers 3 - 31.
	///
	/// **Note that not all counters are available.**
	/// The [`Self::caps_and_id`] field must be checked
	/// for the number of timers that are present.
	///
	/// As of the 1.0a specification, all timers here are **reserved**.
	///
	/// You should access these timers only via the [`Self::get_timer()`]
	/// method.
	///
	/// Accessing a timer that isn't indicated as available
	/// is **undefined behavior**.
	timers:       [HpetTimer; 28],
}

const _: () = {
	oro_macro::assert::size_of::<HpetRegisters, 0x400>();
};

impl HpetRegisters {
	/// Gets the given [`HpetTimer`] by ID.
	///
	/// Returns `None` if the index is out of range.
	///
	/// # Performance
	/// The performance of this function is rather poor;
	/// it performs a volatile read and several shift operations
	/// plus a branch to check the index.
	///
	/// If the timer is used in a high performance scenario,
	/// it's better to store a reference to the timer.
	#[inline]
	pub fn get_timer(&self, idx: usize) -> Option<&HpetTimer> {
		if idx < self.caps_and_id.get().count_timers() {
			match idx {
				0 => Some(&self.timer0),
				1 => Some(&self.timer1),
				2 => Some(&self.timer2),
				i => Some(&self.timers[i - 3]),
			}
		} else {
			None
		}
	}
}

/// An HPET timer register block.
#[repr(C, align(8))]
struct HpetTimer {
	/// Configuration and capability register.
	///
	/// **Read write**
	caps_and_config:  Volatile<HpetTimerCapabilities>,
	/// Comparator value register
	///
	/// **Read write**
	comparator_value: Volatile<u64>,
	/// FSB interrupt route register
	fsb_int_route:    Volatile<u64>,
}

bitstruct! {
	/// The [`HpetRegisters::caps_and_id`] field.
	struct HpetCapsAndId(u64) {
		/// This indicates which revision of the function is implemented.
		///
		/// The value must NOT be `00h`.
		rev_id[7:0] => as u8,
		/// Number of timers.
		///
		/// This indicates the number of timers in this block. The number in this field indicates the
		/// **last timer** (i.e. if there are three timers, the value will be `02h`, four timers will be `03h`,
		/// five timers will be `04h`, etc.).
		num_tim_cap[12:8] => as usize,
		/// Counter size
		counter_size[13] => enum HpetCounterSize(u8) {
			/// Indicates that the main counter is 32-bits wide (and cannot operate in
			/// 64-bit mode).
			Bits32 = 0,
			/// Indicates that the main counter is 64-bits wide (although this does not
			/// preclude it from being operated in a 32-bit mode).
			Bits64 = 1,
		}
		/// LegacyReplacement Route Capable
		///
		/// If this is `true`, it indicates that the hardware supports the LegacyReplacement Interrupt Route option.
		leg_rt_cap[15] => as bool,
		/// This read-only field will be the same as what would be assigned if this logic was a PCI function.
		vendor_id[31:16] => as u16,
		/// Main Counter Tick Period
		///
		/// This read-only field indicates the period at which the counter increments in femptoseconds (`10^-15` seconds).
		/// A value of `0` in this field is not permitted. The value in this field must be less than or equal to `05F5E100h`
		/// (`10^8` femptoseconds = 100 nanoseconds). The resolution must be in femptoseconds (rather than picoseconds)
		/// in order to achieve a resolution of 50 ppm.
		counter_clk_period[63:32] => as u32,
	}
}

impl HpetCapsAndId {
	/// Returns the total number of HPET timers available in the system.
	///
	/// This is one plus [`Self::num_tim_cap`].
	#[inline]
	pub fn count_timers(self) -> usize {
		self.num_tim_cap() + 1
	}
}

bitstruct! {
	/// The general configuration register ([`HpetRegisters::gen_cfg`] field).
	struct HpetGeneralConfig(u64) {
		/// When `true` the main counter will begin running, and interrupts
		/// will be delivered (if configured).
		enable_cfg[0] => as bool,
		/// LegacyReplacement Route:
		///
		/// - `false` – Doesn’t support LegacyReplacement Route
		/// – `true` - Supports LegacyReplacement Route
		///
		/// If the [`Self::enable_cfg`] bit and the `leg_rt_cfg` bit are both set,
		/// then the interrupts will be routed as follows:
		///
		/// - Timer 0 will be routed to IRQ0 in Non-APIC or IRQ2 in the I/O APIC
		/// - Timer 1 will be routed to IRQ8 in Non-APIC or IRQ8 in the I/O APIC
		/// - Timer 2-n will be routed as per the routing in the timer n config registers.
		///
		/// If the LegacyReplacement Route bit is set, the individual routing bits for timers 0 and 1
		/// (APIC or FSB) will have no impact.
		///
		/// If the LegacyReplacement Route bit is not set, the individual routing bits for
		/// each of the timers are used.
		leg_rt_cfg[1] => as bool,
	}
}

bitstruct! {
	/// The general interrupt status register ([`HpetRegisters::gen_status`] field).
	struct HpetGeneralStatus(u64) {
		/// Timer 0 interrupt active.
		///
		/// The functionality of this bit depends on whether the edge or
		/// level-triggered mode is used for this timer:
		///
		/// - **If set to level-triggered mode:** This bit defaults to `false`. This bit will be set by hardware
		///   if the corresponding timer interrupt is active. Once the bit is set, it can be cleared by software
		///   writing a `true` to the same bit position. Writes of `false` to this bit will have no effect.
		///   For example, if the bit is already set a write of `false` will not clear the bit.
		/// - **If set to edge-triggered mode:** This bit should be ignored by software. Software should
		///   always write `false` to this bit.
		t0_int_sts[0] => as bool,
		/// Timer 1 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t1_int_sts[1] => as bool,
		/// Timer 2 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t2_int_sts[2] => as bool,
		/// Timer 3 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t3_int_sts[3] => as bool,
		/// Timer 4 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t4_int_sts[4] => as bool,
		/// Timer 5 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t5_int_sts[5] => as bool,
		/// Timer 6 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t6_int_sts[6] => as bool,
		/// Timer 7 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t7_int_sts[7] => as bool,
		/// Timer 8 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t8_int_sts[8] => as bool,
		/// Timer 9 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t9_int_sts[9] => as bool,
		/// Timer 10 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t10_int_sts[10] => as bool,
		/// Timer 11 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t11_int_sts[11] => as bool,
		/// Timer 12 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t12_int_sts[12] => as bool,
		/// Timer 13 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t13_int_sts[13] => as bool,
		/// Timer 14 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t14_int_sts[14] => as bool,
		/// Timer 15 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t15_int_sts[15] => as bool,
		/// Timer 16 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t16_int_sts[16] => as bool,
		/// Timer 17 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t17_int_sts[17] => as bool,
		/// Timer 18 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t18_int_sts[18] => as bool,
		/// Timer 19 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t19_int_sts[19] => as bool,
		/// Timer 20 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t20_int_sts[20] => as bool,
		/// Timer 21 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t21_int_sts[21] => as bool,
		/// Timer 22 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t22_int_sts[22] => as bool,
		/// Timer 23 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t23_int_sts[23] => as bool,
		/// Timer 24 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t24_int_sts[24] => as bool,
		/// Timer 25 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t25_int_sts[25] => as bool,
		/// Timer 26 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t26_int_sts[26] => as bool,
		/// Timer 27 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t27_int_sts[27] => as bool,
		/// Timer 28 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t28_int_sts[28] => as bool,
		/// Timer 29 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t29_int_sts[29] => as bool,
		/// Timer 30 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t30_int_sts[30] => as bool,
		/// Timer 31 interrupt active.
		///
		/// See [`Self::t0_int_sts`] for behavior information.
		t31_int_sts[31] => as bool,
	}
}

bitstruct! {
	/// HPET Timer `n` Configuration Register ([`HpetTimer::caps_and_config`] field).
	struct HpetTimerCapabilities(u64) {
		/// Interrupt Type
		int_type[1] => enum HpetTimerInterruptType(u8) {
			/// The timer interrupt is edge triggered.
			///
			/// This means that an edge-type interrupt is generated.
			/// If another interrupt occurs, another edge will be generated.
			Edge = 0,
			/// The timer interrupt is level triggered.
			///
			/// This means that a level-triggered interrupt is generated.
			/// The interrupt will be held active until it is cleared by
			/// writing to the bit in the General Interrupt Status Register.
			/// If another interrupt occurs before the interrupt is cleared,
			/// the interrupt will remain active.
			Level = 1,
		},
		/// Interrupt enable.
		///
		/// This read/write bit must be set to enable the timer to cause an
		/// interrupt when the timer event fires
		///
		/// **Note:** If this bit is `false`, the timer will still operate
		/// and generate appropriate status bits, but will not cause an interrupt.
		int_enb[2] => as bool,
		/// Timer Type.
		///
		/// If the [`HpetTimerCapabilities::per_int()`] bit is [`HpetTimerCapability::PeriodicCapable`]
		/// and this bit is [`HpetTimerType::Periodic`] then the timer will generate
		/// periodic interrupts rather than one-shot interrupts.
		///
		/// If [`HpetTimerCapabilities::per_int()`] is instead [`HpetTimerCapability::OneShotOnly`], this
		/// bit is ignored. Users _should not_ set this to `Periodic`.
		///
		/// **Note that the timer counter comparator operates in an unexpected manner
		/// under periodic mode that must be coded differently than one-shot timers.**
		t_type[3] => enum HpetTimerType(u8) {
			/// The timer operates in one-shot mode.
			OneShot = 0,
			/// The timer operates in period mode, generating interrupts at
			/// regular intervals.
			Periodic = 1,
		}
		/// Whether or not periodic mode is supported by this timer.
		per_int[4] => enum HpetTimerCapability(u8) {
			/// The timer only supports one-shot mode.
			OneShotOnly = 0,
			/// The timer supports periodic mode, which can be enabled
			/// by setting [`HpetTimerCapabilities::t_type`] to [`HpetTimerType::Periodic`].
			PeriodicCapable = 1,
		}
		/// The size of the timer's counter.
		size[5] => enum HpetTimerSize(u8) {
			/// The counter width is 32-bits.
			Bits32 = 0,
			/// The counter width is 64-bits.
			Bits64 = 1,
		},
		/// Timer value set.
		///
		/// Software uses this read/write bit only for timers that have been set to periodic mode.
		/// By writing this bit to `true`, the software is then allowed to directly set a periodic
		/// timer’s accumulator.
		///
		/// Software does NOT have to write this bit back to `false` (it automatically clears).
		/// Software **should not** write `true` to this bit position if the timer is set to non-periodic mode.
		val_set[6] => as bool,
		/// 32-bit Mode.
		///
		/// Software can set this read/write bit to force a 64-bit timer to behave as a 32-bit timer.
		/// This is typically needed if the software is not willing to halt the main counter to read
		/// or write a particular timer, and the software is not capable of doing an atomic 64-bit read to the timer.
		/// If the timer is not 64 bits wide, then this bit will always be read as `false` and writes will have no effect.
		mode_32[8] => as bool,
		/// Interrupt route.
		///
		/// This 5-bit read/write field indicates the routing for the interrupt to the I/O APIC.
		/// A maximum value of 32 interrupts are supported. Default is `00h`.
		///
		/// Software writes to this field to select which interrupt in the I/O (x) will be used for this timer’s interrupt.
		/// If the value is not supported by this particular timer, then the value read back will not match what is written.
		/// The software must only write valid values.
		///
		/// **Note:** If the LegacyReplacement Route bit is set, then Timers 0 and 1 will have a different routing,
		/// and this bit field has no effect for those two timers. Note: If the `Tn_FSB_INT_DEL_CNF` bit is set,
		/// then the interrupt will be delivered directly to the FSB, and this bit field has no effect.
		///
		/// **Note from `qix-`:** There's no indication of what `Tn_FSB_INT_DEL_CNF` is. There's no other mention of it
		/// in the Intel HPET Specification 1.0a (2004). It's probably referring (erroneously) to [`Self::fsb_en`]. I've
		/// not translated the spec to refer to the Rust field in the above documentation just to avoid any potential
		/// errors of my own, as I could be incorrect.
		int_route[13:9] => as u8,
		/// FSB Interrupt Delivery Enable
		///
		/// If the [`Self::fsb_int_del_cap()`] bit is `true` for this timer, then the software can set the this bit to
		/// force the interrupts to be delivered directly as FSB messages, rather than using the I/O (x) APIC.
		///
		/// In this case, the [`Self::int_route()`] field in this register will be ignored.
		/// The [`HpetTimer::fsb_int_route`] register will be used instead.
		fsb_en[14] => as bool,
		/// FBS Interrupt Delivery Capability
		///
		/// If this read-only bit is `true`, then the hardware supports a direct
		/// front-side bus delivery of this timer’s interrupt.
		fsb_int_del_cap[15] => as bool,
		/// Interrupt routing capability.
		///
		/// This 32-bit read-only field indicates to which interrupts in the I/O (x) APIC this timer’s interrupt can be routed.
		/// This is used in conjunction with the [`Self::int_route()`] field.
		///
		/// Each bit in this field corresponds to a particular interrupt.
		/// For example, if this timer’s interrupt can be mapped to interrupts 16, 18, 20, 22, or 24,
		/// then bits 16, 18, 20, 22, and 24 in this field will be set to 1. All other bits will be 0.
		// NOTE(qix-): Fun fact: the specification has an error marking the high bit here as `64` instead of `63`.
		int_rout_cap[63:32] => as u32,
	}
}
