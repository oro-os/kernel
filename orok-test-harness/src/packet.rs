//! Raw packets coming from the test stream, before any processing or filtering.
//! See [`Packet`] for details on the packet format.

/// A raw event coming from the test stream.
///
/// # General Layout
/// An event frame in the stream has eight "logical" `u64` values -
/// "logical" in quotes since zeroed values are not transmitted on the
/// stream to cut down on IO operations.
///
/// All values are transmitted as little-endian.
///
/// The first value indicates the source, value bitset (indicating which
/// of the 7 additional values are non-zero and thus are expected on the
/// wire), and the event type ID.
///
/// Each value is referred to as `r0`-`r7`.
///
/// # `r0` Format
/// `r0`, the first value, is always present in a frame.
///
/// - `bit[63]` indicates the general source of the event - either QEMU (`1`)
///   or the kernel, via the MMIO device that `oro-qemu` sets up (`0`)
/// - `bit[62:56]` (7 bits) are a bitmask indicating which values are sent
///   over the wire. Thus, a popcount of this field indicates how many `u64le`
///   values should be read from the stream to complete the frame, and `1` bits
///   indicate where each of those values ultimately land in the full 8-value frame.
///   `mask & (1 << (r - 1))` (e.g. for r3, `mask & (1 << (3 - 1)) = mask & 4`).
/// - `bit[55:48]` (8 bits) are the thread ID of the event. If the value is `255`,
///   there is no thread ID associated with the event and the field should be ignored.
/// - `bit[47:0]` (48 bits) are the event ID.
///
/// # Stream Characteristics
/// Upon stream start, a "reset" frame is sent. All values are transmitted (i.e. `8 * 8 = 64` bytes)
/// all bits of which are set (i.e. all `0xFF` bytes).
#[repr(transparent)]
pub struct Packet(pub(crate) [u64; 8]);

impl Packet {
	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	pub const fn is_from_qemu(&self) -> bool {
		(self.0[0] >> 63) & 1 == 1
	}

	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	pub const fn is_from_kernel(&self) -> bool {
		!self.is_from_qemu()
	}

	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	pub const fn ty(&self) -> u64 {
		self.0[0] & 0x0000_FFFF_FFFF_FFFF
	}

	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	#[expect(
		clippy::as_conversions,
		reason = "the thread ID is only 8 bits, so truncation is intentional and not lossy"
	)]
	pub const fn thread(&self) -> Option<u8> {
		let tid = ((self.0[0] >> 48) & 0xFF) as u8;
		if tid == 255 { None } else { Some(tid) }
	}

	/// Note: register 0 is the **raw** register value; use [`Packet::ty()`],
	/// [`Packet::is_from_qemu()`] or [`Packet::is_from_kernel()`] to extract
	/// relevant information from it.
	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	#[expect(
		clippy::indexing_slicing,
		reason = "it is up to the caller to ensure that idx is in bounds, as this is a hot path \
		          and we want to avoid bounds checks"
	)]
	pub const fn reg(&self, idx: usize) -> u64 {
		self.0[idx]
	}

	#[inline]
	#[must_use = "this function is side-effect free; calling it without using the result makes no \
	              sense"]
	pub const fn regs(&self) -> &[u64; 8] {
		&self.0
	}
}

impl core::fmt::Debug for Packet {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Packet")
			.field(
				"source",
				if self.is_from_qemu() {
					&"QEMU"
				} else {
					&"kernel"
				},
			)
			.field("ty", &format_args!("{:X}", self.ty()))
			.field("thread", &format_args!("{:?}", self.thread()))
			.field("r1", &format_args!("{:X}", self.reg(1)))
			.field("r2", &format_args!("{:X}", self.reg(2)))
			.field("r3", &format_args!("{:X}", self.reg(3)))
			.field("r4", &format_args!("{:X}", self.reg(4)))
			.field("r5", &format_args!("{:X}", self.reg(5)))
			.field("r6", &format_args!("{:X}", self.reg(6)))
			.field("r7", &format_args!("{:X}", self.reg(7)))
			.finish()
	}
}
