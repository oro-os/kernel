<div align="center">
	<img src="https://raw.githubusercontent.com/oro-os/kernel/master/asset/oro-banner.svg" />
	<br>
	<h1 align="center"><b>Oro Operating System</b></h1>
	<br>
	Kernel and associated bootloaders for the <strong>Oro Operating System</strong>,<br>
	a general-purpose, minimal, and novel microkernel operating system written in Rust.
	<br>
	&laquo;&nbsp;<a href="https://oro.sh">oro.sh</a>&nbsp;|&nbsp;<a href="https://discord.gg/WXavRNqcDS">discord</a>&nbsp;|&nbsp;<a href="https://x.com/oro_sys">x</a>&nbsp;&raquo;
	<h1></h1>
	<br>
	<br>
</div>

This is the home of the Oro Operating System kernel and bootloader crates.
All code necessary to build and run the kernel is provided in this repository.

> [!CAUTION]
> The Oro Operating System is currently in the early stages of development.
> It is not yet suitable for use in a production environment.

## Building
The kernel is built standalone and used as a module for a bootloader
entry point. The kernel does not support being booted to directly.

ACPI support uses a submodule; be sure it's checked out (one-time setup):

```shell
git submodule update --init --recursive --depth=1
```

Then, to build the kernel itself:

```shell
cargo kernel-x86_64
cargo kernel-aarch64
```

To build a bootloader:

```shell
cargo limine-x86_64
cargo limine-aarch64
```

To build the example modules:

```shell
cargo oro-examples
```

## Documentation
The Oro kernel is thoroughly documented. You may generate a local copy of
the documentation with:

```shell
cargo oro-doc --open
```

## Security
If you have found a vulnerability within the Oro kernel or any of the associated
crates included in this repository, **please do not open an issue** and instead
consult [SECURITY.md](SECURITY.md) for instructions on how to responsibly disclose
your findings.

# License
The Oro Operating System kernel is &copy; 2016-2025 by Joshua Lee Junon,<br>
and licensed under the [Mozilla Public License 2.0](LICENSE).

Certain crates within this repository are dual-licensed under the
MIT and Apache-2.0 licenses, typically when published to the [crates.io](https://crates.io)
registry. Those crates will contain a `LICENSE.mit` and `LICENSE.apache-2.0` file
and are not subject to the MPL-2.0 license to which the rest of this repository
is subject.
