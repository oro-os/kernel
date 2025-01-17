#![expect(dead_code, clippy::missing_docs_in_private_items)]

use volatile_register::{RO, RW, WO};

const FR_BUSY: u32 = 1 << 3;
const FR_TXFF: u32 = 1 << 5;
const CR_TXEN: u32 = 1 << 8;
const CR_UARTEN: u32 = 1 << 0;
const LCR_FEN: u32 = 1 << 4;

/// A PL011 UART driver.
///
/// Note that this is a very basic implementation and does not support
/// interrupts or DMA; it's used primarily for debugging support.
pub struct PL011 {
	registers:  *const RegisterBlock,
	base_clock: u32,
	baud_rate:  u32,
	data_bits:  DataBits,
	stop_bits:  StopBits,
	parity:     Parity,
}

// SAFETY(qix-): We know that the register block is safe to share.
unsafe impl Send for PL011 {}

/// Specifies the parity bit settings for the UART.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Parity {
	/// No parity is checked (parity checking disabled).
	None = 0,
	/// Parity is enabled and odd parity is checked.
	Odd  = 0b10,
	/// Parity is enabled and even parity is checked.
	Even = 0b110,
}

/// Specifies the number of data bits for the UART.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DataBits {
	/// 5 data bits.
	Five  = 0b00_00000,
	/// 6 data bits.
	Six   = 0b01_00000,
	/// 7 data bits.
	Seven = 0b10_00000,
	/// 8 data bits.
	Eight = 0b11_00000,
}

/// Specifies the number of stop bits for the UART.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StopBits {
	/// 1 stop bit.
	One = 0,
	/// 2 stop bits.
	Two = (1 << 3),
}

// https://developer.arm.com/documentation/ddi0183/g/programmers-model/summary-of-registers?lang=en
#[repr(C)]
struct RegisterBlock {
	dr:    RW<u32>,
	rsr:   RW<u32>,
	_r:    [u32; 4],
	fr:    RO<u32>,
	_r0:   u32,
	ilpr:  RW<u32>,
	// The UARTLCR_H, UARTIBRD, and UARTFBRD registers form the single
	// 30-bit wide UARTLCR Register that is updated on a single write
	// strobe generated by a UARTLCR_H write. So, to internally update
	// the contents of UARTIBRD or UARTFBRD, a UARTLCR_H write must
	// always be performed at the end.
	//
	// https://developer.arm.com/documentation/ddi0183/g/programmers-model/register-descriptions/line-control-register--uartlcr-h?lang=en
	//
	// Note also that these three registers MUST NOT be modified when
	// the PL011 is enabled (CR_UARTEN is set).
	ibrd:  RW<u32>,
	fbrd:  RW<u32>,
	lcr_h: RW<u32>,
	cr:    RW<u32>,
	ifls:  RW<u32>,
	imsc:  RW<u32>,
	ris:   RO<u32>,
	mis:   RO<u32>,
	icr:   WO<u32>,
	dmacr: RW<u32>,
}

impl PL011 {
	/// Create a new PL011 UART driver at the given base address.
	///
	/// To find which base address, run `info qtree` in a QEMU
	/// monitor and look for the `pl011` device's `mmio` property.
	///
	/// # Safety
	/// Caller must ensure that `base` is aligned to a 4-byte boundary
	/// and points to the base register block of the PL011 UART.
	#[must_use]
	pub unsafe fn new(
		base: usize,
		base_clock: u32,
		baud_rate: u32,
		data_bits: DataBits,
		stop_bits: StopBits,
		parity: Parity,
	) -> Self {
		let s = Self {
			registers: base as *const RegisterBlock,
			base_clock,
			baud_rate,
			data_bits,
			stop_bits,
			parity,
		};

		s.reset();
		s
	}

	/// Resets the UART
	pub fn reset(&self) {
		unsafe {
			// Disable the UART
			(*self.registers)
				.cr
				.write((*self.registers).cr.read() & CR_UARTEN);

			// Flush any transmissions
			self.flush();

			// Flush FIFOs
			(*self.registers)
				.lcr_h
				.write((*self.registers).lcr_h.read() & !LCR_FEN);

			// Set frequency settings
			let (integer, fractional) = Self::calculate_divisors(self.base_clock, self.baud_rate);
			(*self.registers).ibrd.write(integer.into());
			(*self.registers).fbrd.write(fractional.into());

			// **MUST** be written to after either IBRD or FBRD
			// are updated, every time, at the END of the update.
			// This is because IBRD, FBRD and LCR make up a singular
			// 30 bit register that is only written to the UART device
			// when LCR is written to.
			(*self.registers).lcr_h.write(
				u32::from(self.data_bits as u8 | self.stop_bits as u8 | self.parity as u8)
					| LCR_FEN,
			);

			// Mask all interrupts
			(*self.registers).imsc.write(0x7FF);

			// Disable DMA
			(*self.registers).dmacr.write(0);

			// Enable transmissions and the UART
			(*self.registers).cr.write(CR_TXEN | CR_UARTEN);
		}
	}

	/// Calculate the baud rate divisor pair
	/// Returns the `(integer, fractional)` parts as a tuple.
	#[expect(clippy::cast_sign_loss)]
	fn calculate_divisors(base_clock: u32, baud_rate: u32) -> (u16, u8) {
		let baud_div = f64::from(base_clock) / (16.0 * f64::from(baud_rate));
		#[expect(clippy::cast_possible_truncation)]
		let integer = baud_div as u16;
		#[expect(clippy::cast_possible_truncation)]
		let fractional = (((baud_div - f64::from(integer)) * 64.0) + 0.5) as u8;
		(integer, fractional)
	}

	/// Waits for any pending transmissions to be cleared
	pub fn flush(&self) {
		unsafe {
			while (*self.registers).fr.read() & FR_BUSY != 0 {
				core::hint::spin_loop();
			}
		}
	}

	// Waits for a write to be possible and then
	// writes a byte
	#[inline]
	pub fn block_write_data_byte(&self, byte: u8) {
		unsafe {
			while (*self.registers).fr.read() & FR_TXFF != 0 {
				core::hint::spin_loop();
			}
			(*self.registers).dr.write(u32::from(byte));
		}
	}

	/// Writes a byte slice to the UART using [`Self::block_write_data_byte`].
	pub fn block_write_all(&self, data: &[u8]) {
		for byte in data {
			self.block_write_data_byte(*byte);
		}
	}
}

impl core::fmt::Write for PL011 {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		for byte in s.bytes() {
			self.block_write_data_byte(byte);
		}
		self.flush();
		Ok(())
	}
}
