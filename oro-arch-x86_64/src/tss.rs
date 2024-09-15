//! Structures and functionality for managing the task state segment (TSS).

/// A task state segment (TSS) entry for x86_64.
#[repr(C)]
#[expect(missing_docs)]
pub struct Tss {
	pub res0:        u32,
	pub rsp0:        TssPtr,
	pub rsp1:        TssPtr,
	pub rsp2:        TssPtr,
	pub res1:        u32,
	pub res2:        u32,
	pub ist1:        TssPtr,
	pub ist2:        TssPtr,
	pub ist3:        TssPtr,
	pub ist4:        TssPtr,
	pub ist5:        TssPtr,
	pub ist6:        TssPtr,
	pub ist7:        TssPtr,
	pub res3:        u32,
	pub res4:        u32,
	pub res5:        u16,
	pub iopb_offset: u16,
}

/// A TSS pointer entry for x86_64.
#[repr(C, align(4))]
#[expect(missing_docs)]
pub struct TssPtr {
	pub low:  u32,
	pub high: u32,
}

impl From<u64> for TssPtr {
	fn from(value: u64) -> Self {
		Self {
			low:  value as u32,
			high: (value >> 32) as u32,
		}
	}
}
