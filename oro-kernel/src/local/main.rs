//! Main kernel thread and associated logic.

use crate::local::state::core_state;
use oro_arch::Target;
use oro_common::{arch::Arch, dbg};

/// Runs the main kernel thread.
pub fn run() -> ! {
	loop {
		Target::halt_once_and_wait();

		// SAFETY(qix-): We're in this function, which means
		// SAFETY(qix-): the kernel has already initialized this memory.
		let core_state = unsafe { core_state() };

		dbg!("TICK", "woken up"); // XXX DEBUG

		if core_state.ticked.read() == 1 {
			core_state.ticked.write(0);
			dbg!("TICK", "TICK!"); // XXX DEBUG
		}
	}
}
