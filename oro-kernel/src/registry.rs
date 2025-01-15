//! Oro kernel object registry implementation.
#![expect(unused_imports)]

use oro_mem::alloc::sync::Arc;
use oro_sync::{Lock, ReentrantMutex};
use oro_sysabi::{
	key,
	syscall::{Error, Opcode, Result},
};
use stash::Stash;

use crate::{
	arch::Arch,
	scheduler::{SystemCallAction, SystemCallRequest, SystemCallResponse},
	thread::Thread,
};

// TODO(qix-): Implement the registry (this is just here for a moment, bear with)
