# Oro Kernel Test Harness

This crate provides the testing harness for the Oro kernel.

The test harness takes in the event stream from a [modified QEMU](https://github.com/oro-os/oro-qemu)
and interprets the stream to perform effects testing, constraint evaluation, etc. in order to
emit warnings, errors and other diagnostics about the kernel's behavior.

The event stream is a _very dense_ stream of events emitted by the kernel, containing events related
to register changes, kernel stages, memory accesses, and more. It's meant to be run in a "cleanroom"
environment, typically under emulation. There are currently no supported means by which it can
be run on real hardware.

There are two places where the harness is used:

- The test TUI, in `orok-test-tui`, which provides a rich interactive interface for running QEMU/kernel
  sessions and inspecting the event stream in real time.
- A command line application, provided in this crate, which can be used to run the harness in scripts, CI,
  etc. and by which emits diagnostics via stdout and stderr to run tests within automated environments.
