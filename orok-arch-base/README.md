# Base Traits for Architecture Implementations for the Oro Kernel
This crate provides the base traits that must be implemented by
an architecture implementation in order to be used by the Oro kernel.

Architecture implementations should export an implementation of each
of these given the documented export requirements in order to be
consumed by `orok-arch` during architecture selection.
