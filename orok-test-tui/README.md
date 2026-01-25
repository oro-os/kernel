# Oro Testing TUI (Terminal User Interface)

This crate implements a full-featured TUI interface specifically catered for
testing the Oro kernel. It requires a modified [QEMU build](https://github.com/oro-os/oro-qemu)
that streams high-density, sequential events not only from QEMU's different
internal events (exceptions, register changes, etc.) and the graphics buffer
(rendered using sixel), but also from the Oro kernel itself via an MMIO device
used for logging/tracing as well as pre-/post-condition or effects checks.

The TUI provides a _rich_ interface to visualize and interact with the test
execution, as well as to spawn new debug sessions with all associated tooling
(GDB, serial console, monitor, etc.) already wired up.

## Prerequisites

- A full Rust toolchain, including C compiler
- GNU `make`
- `xorriso` for building ISO images (`make iso` should run successfully)
- `libchafa-dev` for rendering in the TUI (in case sixel support isn't detected)
- The modified QEMU build from the [Oro QEMU repository](https://github.com/oro-os/oro-qemu)
  on the `PATH`.

## Running

The TUI is the default executable for the entire project. It should be run as `--release`
since it's heavy and will otherwise be quite slow in debug mode.

```sh
cargo run --release
```
