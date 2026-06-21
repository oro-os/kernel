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
The Oro kernel uses its own build utility called `cargo oro`.

To build all possible versions of the kernel and all booloaders, simply run:

```shell
cargo oro build
```

To see all available build options, run:

```shell
cargo oro build --help
```

For all other operations, such as documenting, linting, vendoring, etc.:

```shell
cargo oro --help
```

## Documentation
The Oro kernel is thoroughly documented. You may generate a local copy of
the documentation with:

```shell
cargo oro doc --open
```

## Security
If you have found a vulnerability within the Oro kernel or any of the associated
crates included in this repository, **please do not open an issue** and instead
consult [SECURITY.md](SECURITY.md) for instructions on how to responsibly disclose
your findings.

# License
The Oro Operating System kernel is &copy; 2016-2026 by Joshua Lee Junon and all
other contributors, and licensed under the [GPL-2.0 License](licenses/oro-kernel.txt) with a
syscall exception (see [oro-kernel-syscall-exception.txt](licenses/oro-kernel-syscall-exception.txt))
and a clarification on the license version (see [oro-kernel-gpl2-only.txt](licenses/oro-kernel-gpl2-only.txt)).

Certain crates within this repository are dual-licensed under the
MIT and Apache-2.0 licenses, typically when published to the [crates.io](https://crates.io)
registry. Those crates will contain a `LICENSE.mit` and `LICENSE.apache2` file
and are not subject to the GPL-2.0 license under which the rest of this repository
is licensed.

All folders directly under `vendor/` are third-party crates that are included in this
repository for reproducibility and ease of use. The chosen licenses applicable to the
Oro kernel have been symlinked into `licenses/vendor/<crate-name>`.
