const DR_OFFSET: usize = 0x00;
const FR_OFFSET: usize = 0x18;
const IBRD_OFFSET: usize = 0x24;
const FBRD_OFFSET: usize = 0x28;
const LCR_OFFSET: usize = 0x2C;
const CR_OFFSET: usize = 0x30;
const ISMC_OFFSET: usize = 0x38;
const DMACR_OFFSET: usize = 0x48;

const FR_BUSY: u32 = 1 << 3;
const CR_TXEN: u32 = 1 << 8;
const CR_UARTEN: u32 = 1 << 0;
const LCR_FEN: u32 = 1 << 4;
const LCR_STP2: u32 = 1 << 3;

/// A PL011 UART driver.
///
/// Note that this is a very basic implementation and does not support
/// interrupts or DMA; it's used primarily for debugging support.
pub struct PL011 {
	base: usize,
	base_clock: u32,
	baud_rate: u32,
	data_bits: u32,
	stop_bits: u32,
}

impl PL011 {
	/// Create a new PL011 UART driver at the given base address.
	///
	/// To find which base address, run `info qtree` in a QEMU
	/// monitor and look for the `pl011` device's `mmio` property.
	pub fn new(
		base: usize,
		base_clock: u32,
		baud_rate: u32,
		data_bits: u32,
		stop_bits: u32,
	) -> Self {
		let s = Self {
			base,
			base_clock,
			baud_rate,
			data_bits,
			stop_bits,
		};

		s.reset();
		s
	}

	/// Resets the UART
	fn reset(&self) {
		// Disable the UART
		self.write(CR_OFFSET, self.read(CR_OFFSET) & CR_UARTEN);

		// Flush any transmissions
		self.flush();

		// Flush FIFOs
		self.write(LCR_OFFSET, self.read(LCR_OFFSET) & !LCR_FEN);

		// Set frequency settings
		let (integer, fractional) = Self::calculate_divisors(self.base_clock, self.baud_rate);
		self.write(IBRD_OFFSET, integer);
		self.write(FBRD_OFFSET, fractional);

		// Configure data frame format
		let mut lcr = ((self.data_bits - 1) & 3) << 5;
		if self.stop_bits == 2 {
			lcr |= LCR_STP2;
		}

		self.write(LCR_OFFSET, lcr);

		// Mask all interrupts
		self.write(ISMC_OFFSET, 0x7FF);

		// Disable DMA
		self.write(DMACR_OFFSET, 0);

		// Enable transmissions and the UART
		self.write(CR_OFFSET, CR_TXEN | CR_UARTEN);
	}

	/// Read a register value
	fn read(&self, offset: usize) -> u32 {
		unsafe { core::ptr::read_volatile((self.base + offset) as *const u32) }
	}

	/// Write a register value
	fn write(&self, offset: usize, value: u32) {
		unsafe { core::ptr::write_volatile((self.base + offset) as *mut u32, value) }
	}

	/// Calculate the baud rate divisor pair
	/// Returns the `(integer, fractional)` parts as a tuple.
	fn calculate_divisors(base_clock: u32, baud_rate: u32) -> (u32, u32) {
		let div = 4 * base_clock / baud_rate;
		((div >> 6) & 0xFFFF, (div & 0x3F))
	}

	/// Waits for any pending transmissions to be cleared
	fn flush(&self) {
		while self.read(FR_OFFSET) & FR_BUSY != 0 {
			::core::hint::spin_loop();
		}
	}
}

impl ::core::fmt::Write for PL011 {
	fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
		for byte in s.bytes() {
			self.flush();
			self.write(DR_OFFSET, byte as u32);
		}
		Ok(())
	}
}
