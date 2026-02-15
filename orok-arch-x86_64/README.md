# x86_64 Architecture Support for the Oro Kernel
This crate implements support for the x86_64 architecture in the Oro Kernel.

As with all architecture support crates, this crate is split into two parts:

- `arch`: This module contains architecture-specific, Oro-agnostic types and functions. These are
  general-purpose architecture facilities that could be used by any kernel or OS targeting x86_64,
  and are not tied to the Oro Kernel in any way.
- `oro`: This module contains Oro-specific types and functions that implement the `Arch` trait
  defined in `orok-arch-base`. These types and functions are specific to the Oro Kernel and implement
  the architecture-specific behavior required by the Oro Kernel.
