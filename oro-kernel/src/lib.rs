//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate is a library with the core kernel functionality, datatypes,
//! etc. and provides a common interface for architectures to implement
//! the Oro kernel on their respective platforms.
#![no_std]
// NOTE(qix-): `adt_const_params` isn't strictly necessary but is on track for acceptance,
// NOTE(qix-): and the open questions (e.g. mangling) are not of concern here.
// NOTE(qix-): https://github.com/rust-lang/rust/issues/95174
#![allow(incomplete_features)]
#![feature(adt_const_params)]

use oro_mem::pfa::alloc::{PageFrameAllocate, PageFrameFree};
use oro_sync::spinlock::unfair_critical::UnfairCriticalSpinlock;

pub mod id;
pub mod module;
pub mod port;
pub mod ring;

/// Core-local instance of the Oro kernel.
///
/// Intended to live on the core's respective stack,
/// living for the lifetime of the core (and destroyed
/// and re-created on core powerdown/subsequent bringup).
pub struct Kernel<Pfa: 'static> {
	/// Global reference to the shared kernel state.
	state: &'static KernelState<Pfa>,
}

impl<Pfa: 'static> Kernel<Pfa> {
	/// Creates a new core-local instance of the Kernel.
	///
	/// # Safety
	/// Must only be called once per CPU session (i.e.
	/// boot or bringup after a powerdown case, where the
	/// previous core-local [`Kernel`] was migrated or otherwise
	/// destroyed).
	///
	/// The `state` given to the kernel must be shared for all
	/// instances of the kernel that wish to partake in the same
	/// Oro kernel universe.
	pub unsafe fn new(state: &'static KernelState<Pfa>) -> Self {
		Self { state }
	}

	/// Returns the underlying [`KernelState`] for this kernel instance.
	#[must_use]
	pub fn state(&self) -> &'static KernelState<Pfa> {
		self.state
	}
}

/// Global state shared by all [`Kernel`] instances across
/// core boot/powerdown/bringup cycles.
pub struct KernelState<Pfa: 'static> {
	/// The shared, spinlocked page frame allocator (PFA) for the
	/// entire system.
	pfa: UnfairCriticalSpinlock<Pfa>,
}

impl<Pfa> KernelState<Pfa>
where
	Pfa: PageFrameAllocate + PageFrameFree + 'static,
{
	/// Creates a new instance of the kernel state. Meant to be called
	/// once for all cores at boot time.
	pub fn new(pfa: UnfairCriticalSpinlock<Pfa>) -> Self {
		Self { pfa }
	}

	/// Returns the underlying PFA belonging to the kernel state.
	pub fn pfa(&self) -> &UnfairCriticalSpinlock<Pfa> {
		&self.pfa
	}
}
