use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64};

use crate::{
	Packet,
	atomic::{RelaxedAtomic, RelaxedNumericAtomic},
};

const QEMU_INIT: u64 = 0x0000_FFFF_FFFF_FFFF;
const QEMU_ORO_KDBEVT_X86_EXCEPTION: u64 = 0x1000;
const QEMU_ORO_KDBEVT_X86_REG_DUMP0: u64 = 0x1001;
const QEMU_ORO_KDBEVT_X86_REG_DUMP1: u64 = 0x1002;
const QEMU_ORO_KDBEVT_X86_REG_DUMP2: u64 = 0x1003;
const QEMU_ORO_KDBEVT_X86_REG_DUMP3: u64 = 0x1004;
const QEMU_ORO_KDBEVT_X86_REG_DUMP4: u64 = 0x1005;
const QEMU_ORO_KDBEVT_X86_CR0_UPDATE: u64 = 0x1006;
const QEMU_ORO_KDBEVT_X86_CR3_UPDATE: u64 = 0x1007;
const QEMU_ORO_KDBEVT_X86_CR4_UPDATE: u64 = 0x1008;

/// Tracked state of an event stream.
///
/// State is provided to the event stream
/// handler in order to track events, counts, etc.
/// and to perform individual checks against events
/// taking the state into account (e.g. to check
/// a series of events).
///
/// State is typically initialized via [`State::for_arch`] and then
/// provided to the event processor in order to allow
/// whatever consumer of the library to introspect
/// the state.
#[derive(Debug)]
pub struct State {
	/// The stream has been initialized. Sanity check; this should always be `true` after
	/// the first frame.
	pub initialized: AtomicBool,
	/// Number of packets received.
	pub packet_count: AtomicU64,
	/// Number of events emitted in response to incoming packets.
	pub event_count: AtomicU64,
	/// The number of skipped constraint checks.
	pub skipped_constraints: AtomicU64,
	/// The last offset of the raw debug location C-string.
	///
	/// This is initialized to 0, and the test harness will always
	/// link a null byte at the first location.
	pub last_debug_loc_offset: AtomicU64,
	/// Whether or not we've hit the kernel execution.
	///
	/// This is to filter out CPU state that changes prior to
	/// the kernel actually being executed (e.g. control register
	/// writes that are reported by QEMU prior to the effects system
	/// coming online).
	pub in_kernel: AtomicBool,
	/// Whether or not the environment reports register writes (e.g. QEMU)
	pub reports_register_writes: AtomicBool,
	/// The last core that was seen from a packet. Set to `255` to indicate a "global",
	/// core-less event. Upon handling events, the core that emitted the packet came
	/// from this core.
	pub last_core: AtomicU8,
	/// NOTE: Certian arch state is **not** updated very frequently; typically only
	/// around exception events and the like.
	pub arch: Arch,
}

impl State {
	/// Creates a new `State` for the given architecture.
	pub fn for_arch<A: Default + Into<Arch>>() -> Self {
		Self {
			reports_register_writes: Default::default(),
			initialized: AtomicBool::new(false),
			packet_count: Default::default(),
			event_count: Default::default(),
			skipped_constraints: Default::default(),
			in_kernel: Default::default(),
			last_debug_loc_offset: Default::default(),
			last_core: AtomicU8::new(255),
			arch: A::default().into(),
		}
	}

	/// Creates a new `State` for the given [`ArchType`].
	pub fn for_arch_type(ty: ArchType) -> Self {
		match ty {
			ArchType::X8664 => Self::for_arch::<X8664State>(),
			ArchType::Aarch64 => Self::for_arch::<Aarch64State>(),
			ArchType::Riscv64 => Self::for_arch::<Riscv64State>(),
		}
	}

	/// Indicates that this state will receive CPU state reports
	/// (e.g. from QEMU).
	///
	/// Do NOT call this if the test harness will not receive CPU
	/// state reports. CPU state reports are only available in
	/// emulated environments, typically (e.g. QEMU).
	///
	/// There may be a future where certain baremetal test environments
	/// will be able to receive certain CPU state reports, but as of
	/// writing (14 Feb, 2026) that is not the case.
	pub fn will_receive_cpu_state(self) -> Self {
		self.reports_register_writes.set(true);
		self
	}
}

/// State for one of the supported architectures.
///
/// The creator of a [`State`] (i.e. the consumer of this library)
/// must know which architecture is being executed beforehand.
#[derive(Debug)]
#[expect(
	variant_size_differences,
	reason = "State is big, variants are big. These are allocated on the heap via Arc."
)]
pub enum Arch {
	X8664(X8664State),
	Aarch64(Aarch64State),
	Riscv64(Riscv64State),
}

/// One of the supported architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchType {
	X8664,
	Aarch64,
	Riscv64,
}

impl core::fmt::Display for ArchType {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::X8664 => "x86_64".fmt(f),
			Self::Aarch64 => "AArch64".fmt(f),
			Self::Riscv64 => "RISC-V64".fmt(f),
		}
	}
}

impl Arch {
	/// Returns the [`ArchType`] for this architecture
	pub fn ty(&self) -> ArchType {
		match self {
			Self::X8664(_) => ArchType::X8664,
			Self::Aarch64(_) => ArchType::Aarch64,
			Self::Riscv64(_) => ArchType::Riscv64,
		}
	}
}

/// x86_64 state.
#[derive(Default, Debug)]
pub struct X8664State {
	/// Per-core state
	pub core: X8664MultiCoreState,
}

/// All core states for x86_64
///
/// Wrapped due to needing special default initialization logic
#[derive(Debug)]
pub struct X8664MultiCoreState(pub [X8664CoreState; 255]);

impl Default for X8664MultiCoreState {
	fn default() -> Self {
		let mut arr = [const { core::mem::MaybeUninit::<X8664CoreState>::uninit() }; 255];
		for elem in &mut arr {
			elem.write(X8664CoreState::default());
		}

		// SAFETY: We can assert all the data is now valid.
		unsafe { Self(core::mem::transmute::<_, [X8664CoreState; 255]>(arr)) }
	}
}

impl core::ops::Deref for X8664MultiCoreState {
	type Target = [X8664CoreState; 255];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// x86_64 per-core state
#[derive(Debug, Default)]
pub struct X8664CoreState {
	pub rax:        AtomicU64,
	pub rbx:        AtomicU64,
	pub rcx:        AtomicU64,
	pub rdx:        AtomicU64,
	pub rip:        AtomicU64,
	pub cr0:        AtomicU64,
	pub cr2:        AtomicU64,
	pub cr3:        AtomicU64,
	pub cr4:        AtomicU64,
	pub rflags:     AtomicU64,
	pub efer:       AtomicU64,
	pub cpl:        AtomicU8,
	pub rsi:        AtomicU64,
	pub rdi:        AtomicU64,
	pub rbp:        AtomicU64,
	pub rsp:        AtomicU64,
	pub r8:         AtomicU64,
	pub r9:         AtomicU64,
	pub r10:        AtomicU64,
	pub r11:        AtomicU64,
	pub r12:        AtomicU64,
	pub r13:        AtomicU64,
	pub r14:        AtomicU64,
	pub r15:        AtomicU64,
	pub cs:         AtomicU16,
	pub es:         AtomicU16,
	pub ds:         AtomicU16,
	pub fs:         AtomicU16,
	pub gs:         AtomicU16,
	pub ss:         AtomicU16,
	pub dr0:        AtomicU64,
	pub dr1:        AtomicU64,
	pub dr2:        AtomicU64,
	pub dr3:        AtomicU64,
	pub dr4:        AtomicU64,
	pub dr5:        AtomicU64,
	pub dr6:        AtomicU64,
	pub dr7:        AtomicU64,
	pub exception:  AtomicU8,
	pub error_code: AtomicU32,

	// Last Seen Events
	pub last_reg_write_cr0:          AtomicU64,
	pub last_fx_reg_write_cr0_start: AtomicU64,
	pub last_fx_reg_write_cr0_end:   AtomicU64,

	// Effects
	pub fx_reg_write_cr0: AtomicBool,
}

impl From<X8664State> for Arch {
	#[inline]
	fn from(value: X8664State) -> Self {
		Self::X8664(value)
	}
}

/// AArch64 state.
#[derive(Default, Debug)]
pub struct Aarch64State {
	pub esr_el: AtomicU64,
	pub far_el: AtomicU64,
	pub pc: AtomicU64,
	pub pstate: AtomicU64,
	pub exception_level: AtomicU8,
	pub sp: AtomicU64,
	/// x31 is [`Aarch64State::sp`], hence only 31 registers here.
	///
	/// Note that `x[29]` is the frame pointer (FP) and
	/// `x[30]` is the link register (LR).
	pub x: [AtomicU64; 31],
	pub ttbr0_el1: AtomicU64,
	pub ttbr1_el1: AtomicU64,
	pub ttbr0_el2: AtomicU64,
	pub ttbr1_el2: AtomicU64,
	pub ttbr0_el3: AtomicU64,
	pub ttbr1_el3: AtomicU64,
}

impl From<Aarch64State> for Arch {
	#[inline]
	fn from(value: Aarch64State) -> Self {
		Self::Aarch64(value)
	}
}

/// RISC-V64 state.
#[derive(Default, Debug)]
pub struct Riscv64State {
	/// Exception cause
	pub e_cause: AtomicU64,
	/// Trap value; typically faulting address or illegal value
	pub tval: AtomicU64,
	pub pc: AtomicU64,
	pub mstatus: AtomicU64,
	/// M=3, S=1, U=0 - Virtual level not implemented
	pub privilege_level: AtomicU8,
	/// Whether or not virtualization is active
	pub virtualization_active: AtomicBool,
	/// Transformed instruction for two-stage faults; either 64-bits
	/// or 32-bits, depending on encoding
	pub tinst: AtomicU64,
	pub old_satp: AtomicU64,
	pub new_satp: AtomicU64,
	pub x: [AtomicU64; 32],
}

impl From<Riscv64State> for Arch {
	#[inline]
	fn from(value: Riscv64State) -> Self {
		Arch::Riscv64(value)
	}
}

/// Handles events (faults, exceptions, etc.) caused by checker violations.
///
/// If events are not interesting, pass `()` to [`State::handle_packet`].
#[expect(unused_variables)]
pub trait EventHandler {
	/// Handles an event.
	fn handle_event(&self, event: Event) {}
}

impl EventHandler for () {}

/// An event coming from a state update.
#[derive(Debug, thiserror::Error, Clone)]
pub enum Event {
	/// A packet was received prior to the initialization frame from QEMU.
	#[error("received a packet from QEMU before the initialization frame (packet type: {ty:X})")]
	NotInitialized { ty: u64 },
	/// An initialization packet was received multiple times from QEMU.
	#[error("received multiple initialization packets from QEMU")]
	AlreadyInitialized,

	/// A QEMU event was received with an unknown/unsupported message type
	#[error("QEMU event received with unsupported message type: {ty:X}")]
	UnknownQemuEvent { ty: u64 },
	/// A kernel event was received with an unknown/unsupported message type
	#[error("kernel event received with unsupported message type: {ty:X}")]
	UnknownKernelEvent { ty: u64 },
	/// A QEMU event was received for an architecture that didn't match the current [`State::arch`] variant.
	#[error(
		"QEMU event received for architecture {received:?}, but current state uses architecture \
		 {current:?}"
	)]
	QemuEventArchMismatch {
		current:  ArchType,
		received: ArchType,
	},
	/// A kernel event was received for an architecture that didn't match the current [`State::arch`] variant.
	#[error(
		"kernel event received for architecture {received:?}, but current state uses architecture \
		 {current:?}"
	)]
	KernelEventArchMismatch {
		current:  ArchType,
		received: ArchType,
	},
	/// A kernel effect start event was received with an unknown effect ID
	#[error("kernel effect start event received with unknown effect ID: {effect_id:X}")]
	UnknownKernelEffectStart { effect_id: u64 },
	/// A kernel effect end event was received with an unknown effect ID
	#[error("kernel effect end event received with unknown effect ID: {effect_id:X}")]
	UnknownKernelEffectEnd { effect_id: u64 },
	/// An "oro has started execution" event was passed more than once
	#[error("in_kernel event received more than once (the kernel has already started)")]
	KernelAlreadyStarted,
	/// A QEMU packet that is meant for per-core state updates had no core.
	#[error(
		"a QEMU packet that is meant for per-core state updates had no core specified in its r0: \
		 {ty:X}"
	)]
	QemuMissingCore { ty: u64 },
	/// A kernel packet was missing a core ID.
	///
	/// This is slightly different than the [`Event::QemuMissingCore`] in that _every_ kernel packet
	/// must have a core ID.
	#[error("kernel packet is missing a core id: {ty:X}")]
	KernelMissingCore { ty: u64 },

	/// An [`Exception`] occurred.
	#[error("an exception occurred: {0:?}")]
	Exception(Exception),

	/// On x86_64, the exception code was >= 32 (invalid).
	#[error("(x86_64) the exception code was >=32 (out of range): {exception}")]
	X8664ExceptionOutOfRange { exception: u64 },
	/// On x86_64, the error code was out of range (larger than 32-bits, invalid).
	#[error("(x86_64) the error code was outside a 32-bit range (out of range): {error_code}")]
	X8664ErrorCodeOutOfRange { error_code: u64 },
	/// On x86_64, a selector was out of range (larger than 16-bits, invalid).
	#[error(
		"(x86_64) a selector value was outside a 16-bit range (out of range): \
		 {selector:?}({value})"
	)]
	X8664SelectorOutOfRange {
		selector: X8664Selector,
		value:    u64,
	},
	/// On x86_64, a CPL was received (e.g. via an exception event) that was >3 (out of range, invalid).
	#[error("(x86_64) a CPU protection level (CPL) was received that was >3 (out of range): {cpl}")]
	X8664CplOutOfRange { cpl: u64 },

	/// A constraint check failed.
	#[error("a constraint check failed: {0}")]
	Constraint(ConstraintError),
}

/// An x86_64 selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X8664Selector {
	Cs,
	Ds,
	Fs,
	Gs,
	Ss,
	Es,
}

/// An exception (error) that has occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exception {
	X8664(X8664Exception),
	Aarch64(Aarch64Exception),
	Riscv64(Riscv64Exception),
}

/// An x86_64 exception (error) that has occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X8664Exception {
	/// Other (unknown or unspecific)
	Other(u64),
}

impl From<X8664Exception> for Exception {
	#[inline]
	fn from(value: X8664Exception) -> Self {
		Exception::X8664(value)
	}
}

/// An AArch64 exception (error) that has occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64Exception {
	/// Other (unknown or unspecific)
	Other(u64),
}

impl From<Aarch64Exception> for Exception {
	#[inline]
	fn from(value: Aarch64Exception) -> Self {
		Exception::Aarch64(value)
	}
}

/// A RISC-V64 exception (error) that has occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Riscv64Exception {
	/// Other (unknown or unspecific)
	Other(u64),
}

impl From<Riscv64Exception> for Exception {
	#[inline]
	fn from(value: Riscv64Exception) -> Self {
		Exception::Riscv64(value)
	}
}

impl State {
	/// Handles a raw packet, updating the state and potentially
	/// emitting one or more events.
	pub fn handle_packet(&self, packet: &Packet, handler: &impl EventHandler) {
		self.packet_count.increment();

		self.last_core.set(packet.thread().unwrap_or(255));

		if packet.is_from_qemu() && packet.ty() == QEMU_INIT {
			if self.initialized.set(true) {
				self.emit(handler, Event::AlreadyInitialized);
			}
			return;
		} else if !self.initialized.get() {
			self.emit(handler, Event::NotInitialized { ty: packet.ty() });
			// Set this to true to avoid emitting NotInitialized for every single packet until the init packet arrives.
			self.initialized.set(true);
		}

		// Handle state updates.
		if packet.is_from_qemu() {
			// Sanity check
			debug_assert!(!packet.is_from_kernel());

			match packet.ty() {
				QEMU_ORO_KDBEVT_X86_EXCEPTION => self.handle_q1000(packet, handler),
				QEMU_ORO_KDBEVT_X86_REG_DUMP0 => self.handle_q1001(packet, handler),
				QEMU_ORO_KDBEVT_X86_REG_DUMP1 => self.handle_q1002(packet, handler),
				QEMU_ORO_KDBEVT_X86_REG_DUMP2 => self.handle_q1003(packet, handler),
				QEMU_ORO_KDBEVT_X86_REG_DUMP3 => self.handle_q1004(packet, handler),
				QEMU_ORO_KDBEVT_X86_REG_DUMP4 => self.handle_q1005(packet, handler),
				QEMU_ORO_KDBEVT_X86_CR0_UPDATE => self.handle_q1006(packet, handler),
				QEMU_ORO_KDBEVT_X86_CR3_UPDATE => self.handle_q1007(packet, handler),
				QEMU_ORO_KDBEVT_X86_CR4_UPDATE => self.handle_q1008(packet, handler),
				ty => self.emit(handler, Event::UnknownQemuEvent { ty }),
			}
		} else {
			use orok_test_consts as C;

			// Sanity check
			debug_assert!(packet.is_from_kernel());

			match packet.ty() {
				C::EFFECT_START => self.handle_k0001(packet, handler),
				C::EFFECT_END => self.handle_k0002(packet, handler),
				C::IN_KERNEL => self.handle_k0003(packet, handler),
				ty => self.emit(handler, Event::UnknownKernelEvent { ty }),
			}
		}

		// Now validate all constraints.
		//
		// We only do this in the context of a specific core; the few cases
		// where no core ID is provided are mostly informational and won't
		// result in any constraint changes that would need to be checked.
		//
		// All events coming from the kernel have a core associated with them
		if let Some(core_id) = packet.thread() {
			self.check_constraints(core_id, handler);
		}
	}

	/// Emits an event, updating counters.
	fn emit(&self, handler: &impl EventHandler, event: Event) {
		self.event_count.increment();
		handler.handle_event(event);
	}

	/// Requires a core; otherwise, emits an event and returns `None`.
	fn require_core(&self, packet: &Packet, handler: &impl EventHandler) -> Option<usize> {
		let Some(core_id) = packet.thread() else {
			if packet.is_from_qemu() {
				self.emit(handler, Event::QemuMissingCore { ty: packet.ty() });
			} else {
				self.emit(handler, Event::KernelMissingCore { ty: packet.ty() });
			}
			return None;
		};

		Some(usize::from(core_id))
	}

	/// `ORO_KDBEVT_X86_EXCEPTION` - x86/x86-64 Exception event
	///
	/// - reg[1] = exception number (0-31)
	/// - reg[2] = error code (if applicable)
	/// - reg[3] = CR2 (page fault linear address)
	/// - reg[4] = RIP/EIP
	/// - reg[5] = CS
	/// - reg[6] = RFLAGS/EFLAGS
	/// - reg[7] = CPL (current privilege level)
	fn handle_q1000(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			if packet.reg(1) >= 32 {
				self.emit(
					handler,
					Event::X8664ExceptionOutOfRange {
						exception: packet.reg(1),
					},
				);
			}

			core.exception.set((packet.reg(1) & (32 - 1)) as u8);
			let error_code = packet.reg(2) & u64::from(u32::MAX);
			if error_code != packet.reg(2) {
				self.emit(
					handler,
					Event::X8664ErrorCodeOutOfRange {
						error_code: packet.reg(2),
					},
				);
			}
			core.error_code.set(error_code as u32);
			core.cr2.set(packet.reg(3));
			core.rip.set(packet.reg(4));
			let cs = packet.reg(5) & u64::from(u16::MAX);
			if cs != packet.reg(5) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Cs,
						value:    packet.reg(5),
					},
				);
			}
			core.rflags.set(packet.reg(6));
			if packet.reg(7) > 3 {
				self.emit(handler, Event::X8664CplOutOfRange { cpl: packet.reg(7) });
			}
			core.cpl.set((packet.reg(7) & 3) as u8);
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 Register dump 0: General purpose registers
	/// - reg[1] = RAX/EAX
	/// - reg[2] = RBX/EBX
	/// - reg[3] = RCX/ECX
	/// - reg[4] = RDX/EDX
	/// - reg[5] = RSI/ESI
	/// - reg[6] = RDI/EDI
	/// - reg[7] = RBP/EBP
	fn handle_q1001(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.rax.set(packet.reg(1));
			core.rbx.set(packet.reg(2));
			core.rcx.set(packet.reg(3));
			core.rdx.set(packet.reg(4));
			core.rsi.set(packet.reg(5));
			core.rdi.set(packet.reg(6));
			core.rbp.set(packet.reg(7));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 Register dump 1: Stack pointer and R8-R13 (64-bit only)
	/// - reg[1] = RSP/ESP
	/// - reg[2] = R8  (0 in 32-bit mode)
	/// - reg[3] = R9  (0 in 32-bit mode)
	/// - reg[4] = R10 (0 in 32-bit mode)
	/// - reg[5] = R11 (0 in 32-bit mode)
	/// - reg[6] = R12 (0 in 32-bit mode)
	/// - reg[7] = R13 (0 in 32-bit mode)
	fn handle_q1002(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.rsp.set(packet.reg(1));
			core.r8.set(packet.reg(2));
			core.r9.set(packet.reg(3));
			core.r10.set(packet.reg(4));
			core.r11.set(packet.reg(5));
			core.r12.set(packet.reg(6));
			core.r13.set(packet.reg(7));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 Register dump 2: R14-R15 and segment selectors (64-bit only)
	/// - reg[1] = R14 (0 in 32-bit mode)
	/// - reg[2] = R15 (0 in 32-bit mode)
	/// - reg[3] = ES selector
	/// - reg[4] = DS selector
	/// - reg[5] = FS selector
	/// - reg[6] = GS selector
	/// - reg[7] = SS selector
	fn handle_q1003(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.r14.set(packet.reg(1));
			core.r15.set(packet.reg(2));
			let es = packet.reg(3) & u64::from(u16::MAX);
			if es != packet.reg(3) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Es,
						value:    packet.reg(3),
					},
				);
			}
			core.es.set(es as u16);
			let ds = packet.reg(4) & u64::from(u16::MAX);
			if ds != packet.reg(4) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Ds,
						value:    packet.reg(4),
					},
				);
			}
			core.ds.set(ds as u16);
			let fs = packet.reg(5) & u64::from(u16::MAX);
			if fs != packet.reg(5) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Fs,
						value:    packet.reg(5),
					},
				);
			}
			core.fs.set(fs as u16);
			let gs = packet.reg(6) & u64::from(u16::MAX);
			if gs != packet.reg(6) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Gs,
						value:    packet.reg(6),
					},
				);
			}
			core.gs.set(gs as u16);
			let ss = packet.reg(7) & u64::from(u16::MAX);
			if ss != packet.reg(7) {
				self.emit(
					handler,
					Event::X8664SelectorOutOfRange {
						selector: X8664Selector::Ss,
						value:    packet.reg(7),
					},
				);
			}
			core.ss.set(ss as u16);
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 Register dump 3: Control registers
	/// - reg[1] = CR0
	/// - reg[2] = CR3
	/// - reg[3] = CR4
	/// - reg[4] = unused (CR8 is APIC TPR, complex to access)
	/// - reg[5] = EFER (extended feature enable register)
	/// - reg[6] = unused
	/// - reg[7] = unused
	fn handle_q1004(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.cr0.set(packet.reg(1));
			core.cr3.set(packet.reg(2));
			core.cr4.set(packet.reg(3));
			core.efer.set(packet.reg(5));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 Register dump 4: Debug registers
	/// - reg[1] = DR0
	/// - reg[2] = DR1
	/// - reg[3] = DR2
	/// - reg[4] = DR3
	/// - reg[5] = DR6
	/// - reg[6] = DR7
	/// - reg[7] = unused
	fn handle_q1005(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.dr0.set(packet.reg(1));
			core.dr1.set(packet.reg(2));
			core.dr2.set(packet.reg(3));
			core.dr3.set(packet.reg(4));
			core.dr6.set(packet.reg(5));
			core.dr7.set(packet.reg(6));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 CR0 update event
	/// NOTE: Emitted BEFORE CPU validation - the update may be rejected
	///
	/// - reg[1] = old CR0 value
	/// - reg[2] = new (requested) CR0 value
	/// - reg[3-7] = unused
	fn handle_q1006(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.cr0.set(packet.reg(2));
			core.last_reg_write_cr0.set(self.packet_count.get());
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 CR3 update event
	/// NOTE: Emitted BEFORE CPU validation - the update may be rejected
	///
	/// - reg[1] = old CR3 value
	/// - reg[2] = new (requested) CR3 value
	/// - reg[3-7] = unused
	fn handle_q1007(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.cr3.set(packet.reg(2));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// x86/x86-64 CR4 update event
	/// NOTE: Emitted BEFORE CPU validation - the update may be rejected
	///
	/// - reg[1] = old CR4 value
	/// - reg[2] = new (requested) CR4 value
	/// - reg[3-7] = unused
	fn handle_q1008(&self, packet: &Packet, handler: &impl EventHandler) {
		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		if let Arch::X8664(arch) = &self.arch {
			let core = &arch.core[core_id];

			core.cr4.set(packet.reg(2));
		} else {
			self.emit(
				handler,
				Event::QemuEventArchMismatch {
					received: ArchType::X8664,
					current:  self.arch.ty(),
				},
			);
		}
	}

	/// Kernel Effect Start
	///
	/// - reg[1] = Debug string offset
	/// - reg[2] = Effect ID
	/// - reg[3-7] = unused
	fn handle_k0001(&self, packet: &Packet, handler: &impl EventHandler) {
		use orok_test_consts as C;

		self.last_debug_loc_offset.set(packet.reg(1));

		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		match packet.reg(2) {
			C::X8664_EFFECT_WRITE_REG_CR0 => {
				if let Arch::X8664(arch) = &self.arch {
					let core = &arch.core[core_id];

					core.fx_reg_write_cr0.set(true);
					core.last_fx_reg_write_cr0_start
						.set(self.packet_count.get());
				} else {
					self.emit(
						handler,
						Event::KernelEventArchMismatch {
							received: ArchType::X8664,
							current:  self.arch.ty(),
						},
					);
				}
			}
			effect_id => self.emit(handler, Event::UnknownKernelEffectStart { effect_id }),
		}
	}

	/// Kernel Effect End
	///
	/// - reg[1] = Debug string offset
	/// - reg[2] = Effect ID
	/// - reg[3-7] = unused
	fn handle_k0002(&self, packet: &Packet, handler: &impl EventHandler) {
		use orok_test_consts as C;

		self.last_debug_loc_offset.set(packet.reg(1));

		let Some(core_id) = self.require_core(packet, handler) else {
			return;
		};

		match packet.reg(2) {
			C::X8664_EFFECT_WRITE_REG_CR0 => {
				if let Arch::X8664(arch) = &self.arch {
					let core = &arch.core[core_id];

					core.fx_reg_write_cr0.set(false);
					core.last_fx_reg_write_cr0_end.set(self.packet_count.get());
				} else {
					self.emit(
						handler,
						Event::KernelEventArchMismatch {
							received: ArchType::X8664,
							current:  self.arch.ty(),
						},
					);
				}
			}
			effect_id => self.emit(handler, Event::UnknownKernelEffectEnd { effect_id }),
		}
	}

	/// Oro has started execution.
	///
	/// - reg[1] = Debug string offset
	/// - reg[2-7] = unused
	fn handle_k0003(&self, packet: &Packet, handler: &impl EventHandler) {
		self.last_debug_loc_offset.set(packet.reg(1));

		// We don't care about the core for this event, since any core
		// can signal that we're inside the kernel. That's all that matters,
		// as we assume that once the first core enters Oro's execution,
		// all of the other cores will do nothing until Oro does something
		// with them.
		//
		// This might prove to be a 'naive' assumption, but it works for now.
		if self.in_kernel.set(true) {
			self.emit(handler, Event::KernelAlreadyStarted);
		}
	}
}

/// A constraint error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintError {
	/// The kind of constraint error.
	pub kind:        ConstraintErrorKind,
	/// The constraints, and their outcomes.
	pub constraints: Vec<(&'static str, bool)>,
}

impl core::fmt::Display for ConstraintError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.kind)?;
		for (constraint, outcome) in &self.constraints {
			write!(
				f,
				"\n  - {}: {}",
				if *outcome { "passed" } else { "failed" },
				constraint
			)?;
		}
		Ok(())
	}
}

/// Defines constraints. These are checked after every processed packet.
macro_rules! define_constraints {
	($(
		#[doc = $doc:literal]
		$constraint_name:ident => {
			$( $arch:tt => { $($cmd:tt ($($expr:tt)+));+ ; } )+
		}
	)*) => {
		/// A constraint error kind.
		#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
		pub enum ConstraintErrorKind {
			$(
				#[doc = $doc]
				#[error($doc)]
				$constraint_name,
			)*
		}

		impl State {
			/// Checks all constraints against the current state, emitting events for any violations.
			pub fn check_constraints(&self, core: u8, handler: &impl EventHandler) {
				$(
				#[allow(unused_variables)]
				match &self.arch {
					$(
						Arch::$arch(arch) => 'skip: {
							// Happy path check
							let mut ok = true;
							$({
								let (skip, is_ok) =	define_constraints!(self, arch, &arch.core[usize::from(core)], @CONSTRAINT $cmd { $($expr)+ });
								ok = is_ok && ok;
								if skip {
									self.skipped_constraints.increment();
									break 'skip;
								}
							})+

							if !ok {
								self.emit(handler, Event::Constraint(ConstraintError {
									kind: ConstraintErrorKind::$constraint_name,
									constraints: vec![$(
										(stringify!($cmd $($expr)+), define_constraints!(self, arch, &arch.core[usize::from(core)], @CONSTRAINT $cmd { $($expr)+ }).1)
									),+]
								}));
							}
						}
					)+
					_ => {}
				}
			)*
			}
		}
	};

	($self:expr, $arch:expr, $core:expr, @CONSTRAINT GOOD { $($expr:tt)+ }) => { (false, define_constraints!($self, $arch, $core, @EXPR $($expr)+)) };
	($self:expr, $arch:expr, $core:expr, @CONSTRAINT BAD { $($expr:tt)+ }) => { (false, !(define_constraints!($self, $arch, $core, @EXPR $($expr)+))) };
	($self:expr, $arch:expr, $core:expr, @CONSTRAINT REQ { $($expr:tt)+ }) => { (!define_constraints!($self, $arch, $core, @EXPR $($expr)+), true) };
	($self:expr, $arch:expr, $core:expr, @CONSTRAINT WHEN { $($expr:tt)+ }) => { (!define_constraints!($self, $arch, $core, @EXPR $($expr)+ == #packet_count), true) };

	($self:expr, $arch:expr, $core:expr, @EXPR ! $t:tt $name:ident) => { ! define_constraints!($self, $arch, $core, @EXPRTARG $t $name) };
	($self:expr, $arch:expr, $core:expr, @EXPR $t:tt $name:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $t $name) };

	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident == $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) == define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };
	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident != $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) != define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };
	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident > $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) > define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };
	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident < $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) < define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };
	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident >= $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) >= define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };
	($self:expr, $arch:expr, $core:expr, @EXPR $lt:tt $left:ident <= $rt:tt $right:ident) => { define_constraints!($self, $arch, $core, @EXPRTARG $lt $left) <= define_constraints!($self, $arch, $core, @EXPRTARG $rt $right) };

	($self:expr, $arch:expr, $core:expr, @EXPRTARG # $name:ident) => { $self.$name.get() };
	($self:expr, $arch:expr, $core:expr, @EXPRTARG @ $name:ident) => { $arch.$name.get() };
	($self:expr, $arch:expr, $core:expr, @EXPRTARG % $name:ident) => { $core.$name.get() };
}

define_constraints! {
	/// (x86_64) CR0 written but is not currently within a reg_write effect
	Cr0NotInEffect => {
		X8664 => {
			REQ   (#reports_register_writes);
			REQ   (#in_kernel);
			WHEN  (%last_reg_write_cr0);
			BAD   (!%fx_reg_write_cr0);
		}
	}
	/// (x86_64) CR0 effect ended but no CR0 register write occurred
	Cr0NotWritten => {
		X8664 => {
			REQ   (#reports_register_writes);
			REQ   (#in_kernel);
			WHEN  (%last_fx_reg_write_cr0_end);
			BAD   (%last_fx_reg_write_cr0_start > %last_reg_write_cr0);
		}
	}
}
