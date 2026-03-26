# Oro Kernel Boot Protocol

This crate houses an implementation of the Oro Kernel Boot Protocol,
an in-memory data structure tagging system for bootloaders to pass information
to the Oro kernel.

The protocol is designed to be flexible and extensible, allowing bootloaders
to pass a wide variety of information to the kernel in a structured manner. It's
meant as a lower-level glue by which bootloader stages can translate bootloader
data to kernel data in a way that is backwards compatible.
