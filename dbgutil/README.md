# Oro kernel GDB debug utilities
This is a suite of GDB debugging utilities specifically for the Oro kernel.
They are automatically loaded when opening an Oro kernel variant in
`gdb`, a linux-based debugger.

The utilities help with debugging the Oro kernel via QEMU and performing
high-level operations such as printing the kernel's memory layout, performing
translations, and otherwise introspecting the kernel's state in a way that would
be manually tedious to do, or impossible to do with GDB alone.

Please note that these utilities are NOT intended to be used in production, and
are **SOLELY** for development-time debugging. Further, they are completely
Oro-specific and are not guaranteed to have any usefulness outside of the Oro
ecosystem.

## Usage
To use these utilities, simply open a debug profile variant of the Oro
kernel in GDB.

```shell
rust-gdb -q /path/to/oro-os/kernel/repo/target/x86_64-unknown-oro/debug/oro-kernel-x86_64
rust-gdb -q /path/to/oro-os/kernel/repo/target/aarch64-unknown-oro/debug/oro-kernel-aarch64
```

> [!IMPORTANT]
> GDB's auto-load functionality is whitelist-based, so by default the
> debug utilities will not load. You will need to add the following line to your
> `~/.gdbinit` file to enable auto-loading of the debug utilities:
>
> ```gdb
> add-auto-load-safe-path /path/to/oro-os/kernel/repo
> ```

> [!TIP]
> `gdb` typically only ships with the host architecture supported. It's recommended
> to build GDB from source with full support for all architectures.
>
> You can do this by running the following commands in the gdb source directory:
> ```shell
> ./configure --enable-targets=all --enable-tui --with-expat --with-python
> make -j$(nproc)
> sudo make install
> ```
