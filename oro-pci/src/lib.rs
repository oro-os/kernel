//! PCI support for Oro.
//!
//! **Note:** Requires the global allocator to be set up,
//! as well as the linear mapper to be configured.
#![no_std]

use core::fmt;

use oro_macro::{assert, bitstruct};
use oro_type::{Endian, LittleEndian};

#[doc(hidden)]
type Le<T> = Endian<T, LittleEndian>;

/// Common PCI configuration header.
#[repr(C)]
pub struct ConfigHeader {
	/// The Vendor ID.
	vendor_id: Le<u16>,
	/// The Device ID.
	device_id: Le<u16>,
	/// The Command register.
	command: Le<u16>,
	/// The Status register.
	status: Le<u16>,
	/// The Revision ID.
	revision_id: u8,
	/// The class code.
	class_code: [u8; 3],
	/// The cache line size.
	cache_line_size: u8,
	/// The latency timer.
	latency_timer: u8,
	/// The header type.
	header_type: u8,
	/// The BIST.
	bist: u8,
}

const _: () = const {
	assert::size_of::<ConfigHeader, 16>();
};

impl ConfigHeader {
	/// Returns the vendor ID.
	#[inline]
	#[must_use]
	pub fn vendor_id(&self) -> u16 {
		self.vendor_id.read()
	}

	/// Returns the device ID.
	#[inline]
	#[must_use]
	pub fn device_id(&self) -> u16 {
		self.device_id.read()
	}

	/// Returns the raw command register value.
	#[inline]
	#[must_use]
	pub fn command_raw(&self) -> u16 {
		self.command.read()
	}

	/// Returns the contents of the command register
	/// as a [`CommandRegister`] structure.
	#[inline]
	#[must_use]
	pub fn command(&self) -> CommandRegister {
		CommandRegister(self.command_raw())
	}

	/// Sets the command register.
	#[inline]
	pub fn set_command(&mut self, command: impl Into<u16>) {
		self.command.write(command.into());
	}

	/// Returns the raw status register value.
	#[inline]
	#[must_use]
	pub fn status_raw(&self) -> u16 {
		self.status.read()
	}

	/// Returns the contents of the status register
	/// as a [`StatusRegister`] value.
	#[inline]
	#[must_use]
	pub fn status(&self) -> StatusRegister {
		StatusRegister(self.status_raw())
	}

	/// Sets the status register.
	///
	/// # Safety
	/// In most cases, the status register is read-only. "Setting"
	/// the status is probably not something you need to do, and can
	/// result in undefined behavior if the device is not expecting it.
	///
	/// This method is still provided for niche edge cases (such as
	/// device emulation, etc.), but should be used with caution.
	#[inline]
	pub unsafe fn set_status(&mut self, status: impl Into<u16>) {
		self.status.write(status.into());
	}

	/// Returns the header type code as a `u8`.
	#[inline]
	#[must_use]
	pub fn header_type(&self) -> u8 {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.header_type).read_volatile() & 0x7F }
	}

	/// Returns whether or not the header is multi-function.
	#[inline]
	#[must_use]
	pub fn is_multi_function(&self) -> bool {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { ((&raw const self.header_type).read_volatile() & 0x80) != 0 }
	}

	/// Returns the raw class code bytes
	#[inline]
	#[must_use]
	pub fn class_code_raw(&self) -> [u8; 3] {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.class_code).read_volatile() }
	}

	/// Returns the revision ID.
	#[inline]
	#[must_use]
	pub fn revision_id(&self) -> u8 {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.revision_id).read_volatile() }
	}

	/// Returns the cache line size.
	#[inline]
	#[must_use]
	pub fn cache_line_size(&self) -> u8 {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.cache_line_size).read_volatile() }
	}

	/// Returns the latency timer.
	#[inline]
	#[must_use]
	pub fn latency_timer(&self) -> u8 {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.latency_timer).read_volatile() }
	}

	/// Returns the raw BIST (Built-In Self Test) value.
	#[inline]
	#[must_use]
	pub fn bist_raw(&self) -> u8 {
		// SAFETY: We assume that `self` is valid. This read is always aligned.
		unsafe { (&raw const self.bist).read_volatile() }
	}

	/// Returns the BIST (Built-In Self Test) value
	/// as a [`BistRegister`] structure.
	#[inline]
	#[must_use]
	pub fn bist(&self) -> BistRegister {
		BistRegister(self.bist_raw())
	}

	/// Sets the BIST value.
	///
	/// # Safety
	/// Setting [`BistRegister::start_or_running`] to `1` when
	/// [`BistRegister::supported`] is `false` is undefined.
	#[inline]
	pub unsafe fn set_bist(&mut self, bist: impl Into<u8>) {
		// SAFETY: We assume that `self` is valid. This write is always aligned.
		unsafe {
			(&raw mut self.bist).write_volatile(bist.into());
		}
	}
}

impl fmt::Debug for ConfigHeader {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ConfigHeader")
			.field("vendor_id", &format_args!("{:#04X}", self.vendor_id()))
			.field("device_id", &format_args!("{:#04X}", self.device_id()))
			.field("command", &self.command())
			.field("status", &self.status())
			.field("revision_id", &self.revision_id())
			.field("class_code", &self.class_code_raw())
			.field("cache_line_size", &self.cache_line_size())
			.field("latency_timer", &self.latency_timer())
			.field("header_type", &self.header_type())
			.field("is_multi_function", &self.is_multi_function())
			.field("bist", &self.bist())
			.finish()
	}
}

/// A generic PCI device (`header_type=0`) configuration structure.
#[derive(Debug)]
#[repr(C)]
pub struct PciConfigType0 {
	/// The common header.
	pub header: ConfigHeader,
	/// Base address registers.
	///
	/// **Note:** This is just a bag-of-bytes; its contents
	/// must be interpreted based on each of the base address
	/// contents.
	pub base_registers: [Le<u32>; 6],
	/// The cardbus CIS pointer.
	pub cardbus_cis_pointer: Le<u32>,
	/// The subsystem vendor ID.
	pub subsystem_id: Le<u16>,
	/// The subsystem ID.
	pub subsystem_vendor_id: Le<u16>,
	/// The expansion ROM base address.
	pub expansion_rom_base_address: Le<u32>,
	/// The capabilities pointer.
	pub capabilities_pointer: u8,
	/// Reserved
	pub reserved: [u8; 7],
	/// The interrupt line.
	pub interrupt_line: u8,
	/// The interrupt pin.
	pub interrupt_pin: u8,
	/// The min grant.
	pub min_grant: u8,
	/// The max latency.
	pub max_latency: u8,
}

const _: () = const {
	assert::size_of::<PciConfigType0, 64>();
};

impl PciConfigType0 {
	/// Returns an iterator over the base address registers.
	#[must_use]
	pub fn base_registers_iter(&self) -> BarIter {
		BarIter::new([
			self.base_registers[0].read(),
			self.base_registers[1].read(),
			self.base_registers[2].read(),
			self.base_registers[3].read(),
			self.base_registers[4].read(),
			self.base_registers[5].read(),
		])
	}

	/// Writes the base address registers.
	///
	/// The given slice must occupy _at maximum_ 6 entries.
	///
	/// Always writes starting at the first register; if random-access
	/// is required, use [`Self::write_base_registers_at`].
	///
	/// If the registers to be written are invalid, no registers are
	/// written at all. See [`BarWriteError`] for errors that may arise.
	///
	/// # Safety
	/// Modifying base registers will change the device's
	/// memory map and may cause volatile writes to system
	/// memory that could be unsafe.
	#[inline]
	pub unsafe fn write_base_registers(
		&mut self,
		registers: &[BaseAddressRegister],
	) -> Result<(), BarWriteError> {
		self.write_base_registers_at(0, registers)
	}

	/// Writes the base address registers starting at the given index.
	///
	/// The given slice must occupy _at maximum_ `6 - index` entries.
	///
	/// If the registers to be written are invalid, no registers are
	/// written at all. See [`BarWriteError`] for errors that may arise.
	///
	/// # Safety
	/// Modifying base registers will change the device's
	/// memory map and may cause volatile writes to system
	/// memory that could be unsafe.
	#[cold]
	pub unsafe fn write_base_registers_at(
		&mut self,
		start: usize,
		registers: &[BaseAddressRegister],
	) -> Result<(), BarWriteError> {
		let max_entries = 6 - start;
		let mut vals = [0; 6];
		let mut entry_count = 0;
		for register in registers {
			let (v0, v1) = (*register).into();
			vals[entry_count] = v0;
			if let Some(v1) = v1 {
				vals[entry_count + 1] = v1;
				entry_count += 2;
			} else {
				entry_count += 1;
			}
		}

		if entry_count > max_entries {
			return Err(BarWriteError::TooLong);
		}

		for (i, val) in vals.iter().enumerate().take(entry_count) {
			self.base_registers[start + i].write(*val);
		}

		Ok(())
	}

	/// Convenience function for [`Self::write_base_registers_at(at, &[reg])`].
	///
	/// # Safety
	/// Modifying base registers will change the device's
	/// memory map and may cause volatile writes to system
	/// memory that could be unsafe.
	#[inline]
	pub unsafe fn write_base_register_at(
		&mut self,
		at: usize,
		reg: BaseAddressRegister,
	) -> Result<(), BarWriteError> {
		self.write_base_registers_at(at, &[reg])
	}
}

/// A type of PCI configuration structure.
#[derive(Debug)]
pub enum PciConfig {
	/// A type 0 configuration structure.
	Type0(*mut PciConfigType0),
}

/// A PCI device structure.
#[derive(Debug)]
pub struct PciDevice {
	/// The bus number.
	pub bus:      u8,
	/// The device number.
	pub device:   u8,
	/// The function number.
	pub function: u8,
	/// The base address of the configuration space.
	pub config:   PciConfig,
}

/// Iterates over a memory mapped IO region to probe for PCI devices.
pub struct MmioIterator {
	/// The base address.
	base: *const u8,
	/// The end bus number (inclusive).
	end_bus: u16,
	/// The current bus number.
	current_bus: u16,
	/// The current device number.
	current_device: u8,
	/// The current function number.
	current_function: u8,
}

impl MmioIterator {
	/// Creates a new MMIO iterator.
	///
	/// Returns `None` if the base pointer is not page aligned.
	#[must_use]
	pub fn new(base: *const u8, start_bus: u8, end_bus: u8) -> Option<Self> {
		if base.align_offset(4096) != 0 {
			return None;
		}

		Some(Self {
			base,
			end_bus: u16::from(end_bus),
			current_bus: u16::from(start_bus),
			current_device: 0,
			current_function: 0,
		})
	}
}

impl Iterator for MmioIterator {
	type Item = PciDevice;

	// SAFETY(qix-): The alignment is always valid here, since we have checked the
	// SAFETY(qix-): base offset.
	#[allow(clippy::cast_ptr_alignment)]
	fn next(&mut self) -> Option<Self::Item> {
		for bus in self.current_bus..=self.end_bus {
			for dev in self.current_device..=31 {
				for func in self.current_function..=7 {
					let offset =
						((bus as usize) << 20) | ((dev as usize) << 15) | ((func as usize) << 12);
					let base = unsafe { self.base.add(offset) };
					let header = unsafe { &*base.cast::<ConfigHeader>() };

					if header.vendor_id.read() == 0xFFFF {
						// No device here.
						continue;
					}

					let device = PciDevice {
						bus:      bus as u8,
						device:   dev,
						function: func,
						config:   match header.header_type() {
							0 => PciConfig::Type0(base.cast::<PciConfigType0>().cast_mut()),
							_ => {
								continue;
							}
						},
					};

					self.current_bus = bus;
					self.current_device = dev;
					self.current_function = func + 1;

					return Some(device);
				}

				self.current_function = 0;
			}

			self.current_device = 0;
		}

		self.current_bus = self.end_bus + 1;

		None
	}
}

bitstruct! {
	/// A PCI command register.
	pub struct CommandRegister(u16) {
		/// The I/O space bit.
		///
		/// Controls a device's response to I/O Space accesses. A value of 0
		/// disables the device response. A value of 1 allows the device to
		/// respond to I/O Space accesses. State after RST# is 0.
		pub io_space[0] => as bool,
		/// The memory space bit.
		///
		/// Controls a device's response to Memory Space accesses. A value of
		/// 0 disables the device response. A value of 1 allows the device to
		/// respond to Memory Space accesses. State after RST# is 0.
		pub memory_space[1] => as bool,
		/// The bus master bit.
		///
		/// Controls a device's ability to act as a master on the PCI bus. A value
		/// of 0 disables the device from generating PCI accesses. A value of 1
		/// allows the device to behave as a bus master. State after RST# is 0.
		pub bus_master[2] => as bool,
		/// The special cycles bit.
		///
		/// Controls a device's action on Special Cycle operations. A value of 0
		/// causes the device to ignore all Special Cycle operations. A value of 1
		/// allows the device to monitor Special Cycle operations. State after
		/// RST# is 0.
		pub special_cycles[3] => as bool,
		/// The memory write and invalidate enable bit.
		///
		/// This is an enable bit for using the Memory Write and Invalidate
		/// command. When this bit is 1, masters may generate the command.
		/// When it is 0, Memory Write must be used instead. State after RST#
		/// is 0. This bit must be implemented by master devices that can
		/// generate the Memory Write and Invalidate command.
		pub memory_write_and_invalidate[4] => as bool,
		/// The VGA palette snoop bit.
		///
		/// This bit controls how VGA compatible and graphics devices handle
		/// accesses to VGA palette registers. When this bit is 1, palette
		/// snooping is enabled (i.e., the device does not respond to palette
		/// register writes and snoops the data). When the bit is 0, the device
		/// should treat palette write accesses like all other accesses. VGA
		/// compatible devices should implement this bit.
		pub vga_palette_snoop[5] => as bool,
		/// The parity error response bit.
		///
		/// This bit controls the device's response to parity errors. When the bit
		/// is set, the device must take its normal action when a parity error is
		/// detected. When the bit is 0, the device sets its Detected Parity Error
		/// status bit (bit 15 in the Status register) when an error is detected, but
		/// does not assert PERR# and continues normal operation. This bit's
		/// state after RST# is 0. Devices that check parity must implement this
		/// bit. Devices are still required to generate parity even if parity checking
		/// is disabled.
		pub parity_error_response[6] => as bool,
		/// The stepping bit.
		///
		/// Hardwire this bit to 0.
		///
		/// > This bit cannot be assigned any new meaning in new designs.
		/// > In an earlier version of the PCI specification,
		/// > bit 7 (this bit)  was used and devices may have hardwired it to 0, 1,
		/// > or implemented a read/write bit.
		pub stepping[7] => as bool,
		/// The SERR# enable bit.
		///
		/// This bit is an enable bit for the SERR# driver. A value of 0 disables
		/// the SERR# driver. A value of 1 enables the SERR# driver. This bit's
		/// state after RST# is 0. All devices that have an SERR# pin must
		/// implement this bit. Address parity errors are reported only if this bit
		/// and bit 6 are 1
		pub serr[8] => as bool,
		/// The fast back-to-back enable bit.
		///
		/// This optional read/write bit controls whether or not a master can do
		/// fast back-to-back transactions to different devices. Initialization
		/// software will set the bit if all targets are fast back-to-back capable. A
		/// value of 1 means the master is allowed to generate fast back-to-back
		/// transactions to different agents as described in Section 3.4.2. A value
		/// of 0 means fast back-to-back transactions are only allowed to the
		/// same agent. This bit's state after RST# is 0.
		pub fast_back_to_back[9] => as bool,
		/// The interrupt disable bit.
		///
		/// This bit disables the device/function from asserting INTx#. A value of
		/// 0 enables the assertion of its INTx# signal. A value of 1 disables the
		/// assertion of its INTx# signal. This bit’s state after RST# is 0.
		pub interrupt_disable[10] => as bool,
	}
}

bitstruct! {
	/// Device status register.
	pub struct StatusRegister(u16) {
		/// Interrupt status.
		///
		/// This read-only bit reflects the state of the interrupt in the
		/// device/function. Only when the Interrupt Disable bit in the command
		/// register is a 0 and this Interrupt Status bit is a 1, will the
		/// device’s/function’s INTx# signal be asserted. Setting the Interrupt
		/// Disable bit to a 1 has no effect on the state of this bit.
		pub interrupt_status[3] => as bool,
		/// Capabilities list.
		///
		/// This optional read-only bit indicates whether or not this device
		/// implements the pointer for a New Capabilities linked list at offset 34h.
		/// A value of zero indicates that no New Capabilities linked list is
		/// available. A value of one indicates that the value read at offset 34h is
		/// a pointer in Configuration Space to a linked list of new capabilities.
		pub capabilities_list[4] => as bool,
		/// 66 MHz capable.
		///
		/// This optional read-only bit indicates whether or not this device is
		/// capable of running at 66 MHz as defined in Chapter 7. A value of zero
		/// indicates 33 MHz. A value of 1 indicates that the device is 66 MHz
		/// capable.
		pub mhz66_capable[5] => as bool,
		/// Fast back-to-back capable.
		///
		/// This optional read-only bit indicates whether or not the target is
		/// capable of accepting fast back-to-back transactions when the
		/// transactions are not to the same agent. This bit can be set to 1 if the
		/// device can accept these transactions and must be set to 0 otherwise.
		/// Refer to Section 3.4.2. for a complete description of requirements for
		/// setting this bit.
		pub fast_back_to_back_capable[7] => as bool,
		/// Master data parity error detected.
		///
		/// This bit is only implemented by bus masters. It is set when three
		/// conditions are met: 1) the bus agent asserted PERR# itself (on a
		/// read) or observed PERR# asserted (on a write); 2) the agent setting
		/// the bit acted as the bus master for the operation in which the error
		/// occurred; and 3) the Parity Error Response bit (Command register) is
		/// set.
		pub master_data_parity_error[8] => as bool,
		/// DEVSEL timing.
		///
		/// These bits encode the timing of DEVSEL#. Section 3.6.1 specifies
		/// three allowable timings for assertion of DEVSEL#.
		///
		/// These bits are read-only and must indicate the slowest time
		/// that a device asserts DEVSEL# for any bus command except
		/// Configuration Read and Configuration Write.
		pub devsel_timing[10:9] => enum DevselTiming(u8) {
			/// Fast
			Fast = 0b00,
			/// Medium
			Medium = 0b01,
			/// Slow
			Slow = 0b10,
			/// Reserved value (should not be used)
			Reserved = 0b11,
		}
		/// Signaled target abort.
		///
		/// This bit must be set by a target device whenever it terminates a
		/// transaction with Target-Abort. Devices that will never signal Target-
		/// Abort do not need to implement this bit.
		pub signaled_target_abort[11] => as bool,
		/// Received target abort.
		///
		/// This bit must be set by a master device whenever its transaction is
		/// terminated with Target-Abort. All master devices must implement this
		/// bit.
		pub received_target_abort[12] => as bool,
		/// Received master abort.
		///
		/// This bit must be set by a master device whenever its transaction
		/// (except for Special Cycle) is terminated with Master-Abort. All master
		/// devices must implement this bit
		pub received_master_abort[13] => as bool,
		/// Signaled system error.
		///
		/// This bit must be set whenever the device asserts SERR#. Devices
		/// who will never assert SERR# do not need to implement this bit.
		pub signaled_system_error[14] => as bool,
		/// Detected parity error.
		///
		/// This bit must be set by the device whenever it detects a parity error,
		/// even if parity error handling is disabled (as controlled by bit 6 in the
		/// Command register).
		pub detected_parity_error[15] => as bool,
	}
}

bitstruct! {
	/// The BIST (Built-In Self Test) control and status register.
	pub struct BistRegister(u8) {
		/// Completion code.
		///
		/// A completion code of `0` indicates that the BIST completed successfully.
		/// All other values indicate an error, and are device-specific.
		///
		/// 4-bit value; values `0b0001`-`0b1111` indicate error. Guaranteed
		/// never to return values `>= 16`.
		pub completion_code[3:0] => as u8,
		/// Invokes the BIST (built-in self test).
		///
		/// Setting this to `1` and writing this register back to the
		/// BIST register of the device will cause the device to perform
		/// a self-test.
		///
		/// The self-test may take some time to complete. The device
		/// will set this field back to `0` when the test is complete,
		/// and the completion code can be read from [`Self::completion_code`].
		///
		/// Setting this to `1` when [`Self::supported`] is `false` is undefined.
		pub start_or_running[6] => as bool,
		/// BIST supported.
		///
		/// If this is `false`, the device does not support BIST and
		/// setting [`Self::start_or_running`] to `1` is undefined.
		pub supported[7] => as bool,
	}
}

/// Defines a base address register.
///
/// All encoding bits are zeroed when returned, including
/// reserved bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseAddressRegister {
	/// A 32-bit memory region, with prefetch.
	Memory32Prefetch(u32),
	/// A 32-bit memory region, no prefetch.
	Memory32(u32),
	/// A 64-bit memory region, with prefetch.
	Memory64Prefetch(u64),
	/// A 64-bit memory region, no prefetch.
	Memory64(u64),
	/// A legacy ISA PCI < 1MiB region, with prefetch.
	LegacyMemory1MiBPrefetch(u32),
	/// A legacy ISA PCI < 1MiB region, no prefetch.
	LegacyMemory1MiB(u32),
	/// 32-bit I/O space region.
	Io32(u32),
}

impl BaseAddressRegister {
	/// Returns the number of BAR entries required for this BAR type.
	#[inline]
	#[must_use]
	pub const fn entry_count(self) -> usize {
		match self {
			Self::Memory64Prefetch(_) | Self::Memory64(_) => 2,
			Self::Memory32Prefetch(_)
			| Self::Memory32(_)
			| Self::LegacyMemory1MiBPrefetch(_)
			| Self::LegacyMemory1MiB(_)
			| Self::Io32(_) => 1,
		}
	}

	/// Returns whether or not this BAR is prefetchable.
	#[inline]
	#[must_use]
	pub const fn is_prefetchable(self) -> bool {
		matches!(
			self,
			Self::Memory32Prefetch(_)
				| Self::Memory64Prefetch(_)
				| Self::LegacyMemory1MiBPrefetch(_)
		)
	}

	/// Returns whether or not the BAR is 64-bits wide.
	#[inline]
	#[must_use]
	pub const fn is_u64(self) -> bool {
		matches!(self, Self::Memory64Prefetch(_) | Self::Memory64(_))
	}
}

impl TryFrom<&[u32]> for BaseAddressRegister {
	type Error = BarParseError;

	fn try_from(v: &[u32]) -> Result<Self, Self::Error> {
		let Some(&u0) = v.first() else {
			return Err(BarParseError::TooShort);
		};
		let is_mem = (u0 & 1) == 0;

		if is_mem {
			let ty = (u0 >> 1) & 0b11;

			if oro_macro::unlikely!(ty == 0b11) {
				// Reserved.
				return Err(BarParseError::Invalid);
			}

			let is_prefetch = u0 & (1 << 3) != 0;
			let u0 = u0 & !0b1111;

			match ty {
				0b00 => {
					if is_prefetch {
						Ok(BaseAddressRegister::Memory32Prefetch(u0))
					} else {
						Ok(BaseAddressRegister::Memory32(u0))
					}
				}
				0b01 => {
					if is_prefetch {
						Ok(BaseAddressRegister::LegacyMemory1MiBPrefetch(u0))
					} else {
						Ok(BaseAddressRegister::LegacyMemory1MiB(u0))
					}
				}
				0b10 => {
					let Some(&u1) = v.get(1) else {
						return Err(BarParseError::TooShort);
					};

					let u1 = u64::from(u1);
					let u0 = u64::from(u0);

					if is_prefetch {
						Ok(BaseAddressRegister::Memory64Prefetch((u1 << 32) | u0))
					} else {
						Ok(BaseAddressRegister::Memory64((u1 << 32) | u0))
					}
				}
				_ => unreachable!(),
			}
		} else {
			// I/O space.
			Ok(BaseAddressRegister::Io32(u0 & !0b11))
		}
	}
}

impl From<BaseAddressRegister> for u64 {
	fn from(v: BaseAddressRegister) -> Self {
		#[cfg(debug_assertions)]
		{
			match v {
				BaseAddressRegister::Io32(v) => {
					debug_assert!((v & 0b11) == 0, "I/O BAR has bits[1:0] != 0");
				}
				BaseAddressRegister::Memory64(v) | BaseAddressRegister::Memory64Prefetch(v) => {
					debug_assert!((v & 0b1111) == 0, "64-bit memory BAR has bits[3:0] != 0");
				}
				BaseAddressRegister::LegacyMemory1MiB(v)
				| BaseAddressRegister::LegacyMemory1MiBPrefetch(v)
				| BaseAddressRegister::Memory32(v)
				| BaseAddressRegister::Memory32Prefetch(v) => {
					debug_assert!((v & 0b1111) == 0, "32-bit memory BAR has bits[3:0] != 0");
				}
			}
		}

		#[allow(clippy::identity_op)]
		match v {
			BaseAddressRegister::Memory64(v) => v | 0b0100,
			BaseAddressRegister::Memory64Prefetch(v) => v | 0b1100,
			BaseAddressRegister::Memory32(v) => u64::from(v) | 0b0000,
			BaseAddressRegister::Memory32Prefetch(v) => u64::from(v) | 0b1000,
			BaseAddressRegister::LegacyMemory1MiB(v) => u64::from(v) | 0b0010,
			BaseAddressRegister::LegacyMemory1MiBPrefetch(v) => u64::from(v) | 0b1010,
			BaseAddressRegister::Io32(v) => u64::from(v) | 0b0001,
		}
	}
}

impl From<BaseAddressRegister> for (u32, Option<u32>) {
	fn from(v: BaseAddressRegister) -> Self {
		let entry_count = v.entry_count();
		debug_assert!(
			matches!(entry_count, 1 | 2),
			"BAR entry count is not 1 or 2"
		);
		let is_64 = entry_count == 2;
		let b64: u64 = v.into();
		(
			b64 as u32,
			if is_64 {
				Some((b64 >> 32) as u32)
			} else {
				None
			},
		)
	}
}

/// An error type returned by [`BaseAddressRegister::try_from`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarParseError {
	/// The BAR is invalid (typically memory BAR with `bits[2:1] = 0b11 (reserved)`).
	Invalid,
	/// The given slice of `u32`s is too short (must be at least 2 entries if the BAR
	/// is 64-bit). The first `u32` value is returned.
	TooShort,
}

/// An error type returned by [`PciConfigType0::write_base_registers`].
pub enum BarWriteError {
	/// The given BAR slice is too long (consumed more than 6 entries).
	TooLong,
}

/// An iterator over the BARs of a PCI device.
pub struct BarIter {
	/// The array of `u32`s in the BAR section.
	inner: [u32; 6],
	/// The current index.
	index: usize,
}

impl BarIter {
	/// Creates a new `BarIter`.
	#[inline]
	fn new(inner: [u32; 6]) -> Self {
		Self { inner, index: 0 }
	}
}

impl Iterator for BarIter {
	type Item = Result<BaseAddressRegister, BarParseError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= 6 {
			return None;
		}

		let res = BaseAddressRegister::try_from(&self.inner[self.index..]);
		// If the BAR is invalid, we stop iterating (signaled by setting the
		// the index to >= 6).
		self.index += res.map_or(6, BaseAddressRegister::entry_count);
		Some(res)
	}
}
