# Limine Bootloader Support for the Oro Kernel

This crate adds support for the
[Limine Bootloader](https://github.com/limine-bootloader/limine)
in the Oro operating system kernel under x86_64 and AArch64 architectures.

This crate has both a library (which is common between architectures)
and individual, architecture-specific binaries.

See the `bin/` directory for architecture-specific entry points.