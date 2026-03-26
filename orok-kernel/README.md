# Oro Operating System Kernel

This is the main kernel crate for the Oro operating system kernel repository.
It houses all of the main kernel code, logic, and platform-specific entry points -
however contains no platform-specific code itself, which is instead located in the
`orok-arch-*` crates and selected/exposed through the `orok-arch` crate.
