use oro_ser2mem::Ser2Mem;

/// The Oro boot protocol main configuration structure.
///
/// This structure is passed to the kernel via the bootloader,
/// where it is placed in a well-known location in memory
/// prior to jumping to _start().
///
/// For more information, see the documentation for the
/// [`oro-ser2mem`] and [`oro-bootloader-common`] crates.
#[derive(Ser2Mem)]
#[repr(C)]
pub struct BootConfig {
	/// The number of instances that are being booted.
	/// Note that this _may not_ match the number of CPUs
	/// in the system.
	pub num_instances: u32,
	/// The instance type that is being booted.
	///
	/// See the documentation for [`BootInstanceType`] for more
	/// information about invariants.
	pub instance_type: BootInstanceType,
}

/// Defines which instance of the CPU is being initialized.
/// On single-core systems, for example, this is always `Primary`.
///
/// Bootloaders must take care only to pass `Primary` to one
/// instance of whatever is running in an SMP environment.
#[derive(Ser2Mem, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum BootInstanceType {
	/// The primary CPU instance; performs initialization
	/// of all shared resources.
	Primary = 0,
	/// A secondary CPU instance; performs initialization
	/// of only its own resources.
	Secondary = 1,
}
