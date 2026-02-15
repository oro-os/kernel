# RISC-V 64-bit (GC) Architecture Support for the Oro Kernel
This crate implements support for the RISC-V 64-bit architecture in the Oro Kernel.

As with all architecture support crates, this crate is split into two parts:

- `arch`: This module contains architecture-specific, Oro-agnostic types and functions. These are
  general-purpose architecture facilities that could be used by any kernel or OS targeting RISC-V 64-bit,
  and are not tied to the Oro Kernel in any way.
- `oro`: This module contains Oro-specific types and functions that implement the `Arch` trait
  defined in `orok-arch-base`. These types and functions are specific to the Oro Kernel and implement
  the architecture-specific behavior required by the Oro Kernel.
