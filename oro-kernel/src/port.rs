//! Implements Oro ports in the kernel.

use oro_id::{Id, IdType};

/// A singular port.
///
/// Ports are unidirectional communication channels between
/// [`crate::instance::Instance`]s. They are implemented
/// as ring buffers of slotted messages of fixed size, the size
/// being determined by metadata associated with the port's type.
///
/// Ports are the primary means of communication not only between
/// module instances, but also between the built-in kernel modules
/// provided to the system on the root [`crate::ring::Ring`].
///
/// Ports are exposed to userspace applications at specific addresses
/// in the process address space, and are governed by a system of
/// page swaps, faults, and other mechanisms to ensure ordered delivery.
///
/// Ports are wired up prior to the module instance being spawned, such
/// that instance code can assume that the ports are already available
/// and guaranteed to function. It's the kernel's responsibility to
/// wire up the appropriate ports to the at the appropriate addresses
/// in the address space.
///
/// # Synchronous Ports
/// By default, ports are synchronous. A write to a full buffer on the
/// producer side will page fault and schedule the thread for execution
/// once the slot being written to has been consumed. Likewise, an empty
/// buffer on the consumer side will page fault and schedule the thread
/// for execution once the slot being read from has been produced.
///
/// This is the default behavior, and under certain circumstances,
/// the only behavior (e.g. the user has prohibited "asynchronous upgrades"
/// on a port).
///
/// # Asynchronous Ports
/// Ports are able to read and write to the buffer cursors in order to
/// manage the buffer themselves. If allowed, a sequence of reads and
/// writes will automatically upgrade the port to an asynchronous port
/// for **both the producer and consumer** (meaning that both sides
/// must behave in the same way in order for the kernel to recognize
/// the upgrade).
///
/// In this mode, page faulting is disabled, and the pages are mapped
/// with the appropriate permissions to allow the producer and consumer
/// to read and write to the buffer directly, including ahead of time.
///
/// This mode is inherently unsafe, as it introduces the possibility
/// of race conditions and potential data corruption. Thus, it is
/// possible for the user to prohibit asynchronous upgrades on a port
/// in the event one is behaving incorrectly.
///
/// Asynchronous port operation is useful for high-performance, low-latency
/// communication in cases where pre-empting the thread is not desirable,
/// or whereby the overhead introduced by the page faulting system is
/// too high.
///
/// # Port Types
/// Ports are typed, and the type of the port determines if a producer
/// and consumer can be connected. The kernel will enforce this type
/// checking at the time of wiring up the ports, and will refuse to
/// begin execution of a module instance if not.
///
/// Port types also define their message size, along with other metadata
/// about the port. There are no "standard" port types; they are governed
/// entirely by the ecosystem. For example, the kernel provides several
/// built-in module instances and describes the ports they export and
/// import at the port level.
///
/// It is only imperative that the module loading system provides the
/// kernel with the size of messages for each port type; nothing else
/// is inspected beyond that.
///
/// # Thread Ownership
/// Modules are allowed to spawn threads, and those threads are allowed
/// to read and write to ports. However, ports are owned by a single
/// thread, and only that thread is allowed to read and write to the
/// port at a given time. Attempts to read/write to a port from a thread
/// that does not own it will result in a page fault.
///
/// Ownership of a port may be transferred to another thread, however
/// this is a somewhat expensive operation and should be done sparingly.
pub struct Port {
	/// The resource ID.
	id:        u64,
	/// The type ID of the port.
	type_id:   Id<{ IdType::PortType }>,
	/// Gets the length of the port's message.
	slot_size: usize,
}

impl Port {
	/// Returns the port's ID.
	#[must_use]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// Returns the port's type ID.
	#[must_use]
	pub fn type_id(&self) -> &Id<{ IdType::PortType }> {
		&self.type_id
	}

	/// Returns the port's slot size.
	#[must_use]
	pub fn slot_size(&self) -> usize {
		self.slot_size
	}
}
