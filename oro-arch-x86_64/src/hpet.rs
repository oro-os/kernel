//! High Precision Event Timer (HPET) initialization routines.

use oro_acpi::AcpiTable;
use oro_debug::dbg;
use oro_macro::bitstruct;
use oro_mem::phys::{Phys, PhysAddr};
use oro_type::Volatile;

/// Initializes the HPET.
///
/// # Panics
/// Panics if no HPET is found.
///
/// # Safety
/// Must **only** be called by the primary core, exactly once at system boot.
pub unsafe fn initialize() {
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
	assert!(
		hpet.inner_ref().Address.BitWidth.read() == 0,
		"HPET address bit width is not 0"
	);
	assert!(
		hpet.inner_ref().Address.AccessWidth.read() == 0,
		"HPET address access width is not 0"
	);
	assert!(
		hpet.inner_ref().Address.BitOffset.read() == 0,
		"HPET address bit offset is not 0"
	);

	let phys_addr = hpet.inner_ref().Address.Address.read();
	let registers = Phys::from_address_unchecked(phys_addr)
		.as_ref::<HpetRegisters>()
		.expect("virtual address of HPET base registers is misaligned");

	dbg!("HPET caps and ID: {:#X?}", registers.caps_and_id.get());
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
	/// **Read write**
	gen_cfg:      Volatile<u64>,
	_reserved1:   u64,
	/// General Interrupt Status Register
	///
	/// **Read write**
	gen_status:   Volatile<u64>,
	_reserved2:   u64,
	/// Main counter value register
	///
	/// **Read write**
	main_counter: Volatile<u64>,
	_reserved3:   u64,
	/// Timers.
	///
	/// **Note that not all counters are available.**
	/// The [`Self::caps_and_id`] field must be checked
	/// for the number of timers that are present.
	///
	/// As of the 1.0a specification, timers 3 and above
	/// (index 2 and greater) are **reserved**.
	///
	/// Accessing a timer that isn't indicated as available
	/// is **undefined behavior**.
	timers:       [HpetTimer; 1 << 5],
}

/// An HPET timer register block.
#[expect(clippy::missing_docs_in_private_items)]
#[repr(C, align(8))]
struct HpetTimer {
	/// Configuration and capability register.
	///
	/// **Read write**
	caps_and_config:  Volatile<u64>,
	/// Comparator value register
	///
	/// **Read write**
	comparator_value: Volatile<u64>,
	/// FSB interrupt route register
	fsp_int_route:    Volatile<u64>,
	_reserved:        u64,
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
