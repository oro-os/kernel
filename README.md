<div align="center">
	<img src="https://raw.githubusercontent.com/oro-os/kernel/master/asset/oro-banner.svg" />
	<br>
	<h1 align="center"><b>Oro Operating System</b></h1>
	<br>
	Kernel and associated bootloaders for the <strong>Oro Operating System</strong>,<br>
	a general-purpose, minimal, and novel microkernel operating system written in Rust.
	<br>
	&laquo;&nbsp;<a href="https://oro.sh">oro.sh</a>&nbsp;&raquo;
	<h1></h1>
	<br>
	<br>
</div>

This is the home of the Oro Operating System kernel and bootloader crates.
All code necessary to build and run the kernel is provided in this repository.

## Building
The kernel is built standalone and used as a module for a bootloader
entry point. The kernel does not support being booted to directly.

To build the kernel itself:

```shell
cargo kernel-x86_64
cargo kernel-aarch64
```

To build a bootloader:

```shell
cargo limine-x86_64
cargo limine-aarch64
```

## Documentation
The Oro kernel is thoroughly documented. You may generate a local copy of
the documentation with:

```shell
cargo oro-doc-x86_64 --open
cargo oro-doc-aarch64 --open
```

## Security
If you have found a vulnerability within the Oro kernel or any of the associated
crates included in this repository, **please do not open an issue** and instead
consult [SECURITY.md](SECURITY.md) for instructions on how to responsibly disclose
your findings.

# License
The Oro Operating System kernel is &copy; 2016-2024 by Joshua Lee Junon,
and licensed under the [Mozilla Public License 2.0](LICENSE).
