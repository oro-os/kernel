# Rust `std` interim implementation for Oro modules

This crate aims to shim, **in part**, the Rust `std` crate for Oro modules.

> [!WARNING]
> There are **many, many** types, constants, and methods missing from this crate.
> This is **not** intended to be a full replacement for the Rust `std` crate, and
> is **purely an interim solution** until the time when such functionality is added
> to the upstream `std` crate (if ever).
>
> **Missing functionality is not considered a bug.** However, _incorrect_ functionality
> or additional methods (not including extension traits in the more Oro-specific `oro`
> crate, if any) **are** considered bugs and should be reported as such.

## Usage
For updated usage information, please refer to the [crate documentation](https://docs.rs/oro-std).

Add the following to your `Cargo.toml`:

```toml
[dependencies]
std = { git = "https://github.com/oro-os/kernel.git", package = "oro-std" }
```

**Note that the use of `std = { package = "oro-std" }` is _NOT_ a guarantee that normal `std`-based
Rust code will compile.**

If you would also like to use the packaged `oro` crate, you can add it as a feature:

```toml
[dependencies]
std = { version = "...", package = "oro-std", features = ["oro"] }
```

```rust
use std::os::oro; // requires the "oro" feature
```

> [!WARNING]
> The `std::os::oro` module is **highly unstable**; there is **no guarantee**
> that its API will remain the same indefinitely, and will almost definitely
> require recompilation of any code that uses it in the future. **Use at your own risk.**

## Why not just patch `std`?
It'd require a change to the Rust compiler's target definitions themselves,
which is more work than providing a stripped down interim subset of `std` features.
The way the `oro` crate is able to build modules without much additional work for
module writers has limitations regarding build flags and target settings (namely
that a `--target path/to.json` cannot be passed), which would be required to _correctly_
patch `std` as it contains a lot of `#[cfg(target_os = "...")]` directives.

Instead, the goal of this crate is to provide a future-proof way to write Oro modules _now_
with the intention of fully deprecating its use in the future.

Further, not all Oro kernel functionality has been developed yet nor is set in stone,
so doing it this way allows parts of `std` to be introduced piecemeal as the project
progresses in its early stages.

## License
This crate is Copyright &copy; 2024 by Joshua Lee Junon and is released under either
the [MIT](LICENSE.mit) **or** the [Apache 2.0](LICENSE.apache-2.0) license, at the
user's discretion.

Part of the [Oro Operating System](https://github.com/oro-os) project.
