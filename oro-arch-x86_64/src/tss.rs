//! Structures and functionality for managing the task state segment (TSS).

/// A task state segment (TSS) entry for x86_64.
#[derive(Debug)]
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

impl Default for Tss {
	fn default() -> Self {
		Self {
			res0:        0,
			rsp0:        TssPtr::default(),
			rsp1:        TssPtr::default(),
			rsp2:        TssPtr::default(),
			res1:        0,
			res2:        0,
			ist1:        TssPtr::default(),
			ist2:        TssPtr::default(),
			ist3:        TssPtr::default(),
			ist4:        TssPtr::default(),
			ist5:        TssPtr::default(),
			ist6:        TssPtr::default(),
			ist7:        TssPtr::default(),
			res3:        0,
			res4:        0,
			res5:        0,
			iopb_offset: core::mem::size_of::<Tss>() as u16,
		}
	}
}

/// A TSS pointer entry for x86_64.
#[derive(Debug, Default, Clone, Copy)]
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
