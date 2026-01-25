# Kernel-Agnostic x86_64 Architecture Support
This crate provides a number of wrappers, utility types, functions, etc.
for the x86_64 architecture that are not kernel-specific.

## Things that should be added here:
- Register, instruction, and general wrappers
- Chip-specific behavior (often considered "quirks") and abstractions over them
- Testing utilities for simulating different processor state for use by unit tests

## Things that should **not** be added here:
- Oro kernel-specific implementations of traits, types, adapters, etc.
- Implementations of memory management regimes, etc.
- Peripheral-specific implementations
