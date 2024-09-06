# Bindgen bindings for ACPICA
Oro-specific bindgen-based `-sys` dependency for ACPICA.

> [!WARNING]
> You probably don't want this crate; it's specific to the Oro kernel.
> Check out [`acpica-sys`](https://docs.rs/acpica-sys/latest/acpica_sys/) instead.

Along with the sys bindings, this crate also provides a macro,
`acpi_tables!`, which can be used to generate code based on
all of the detected ACPI tables that are supported.

For example:

```rust
use oro_acpica_sys::acpi_tables;

macro_rules! impl_tables {
	($($slug:tt => ($strukt:ident, $sig:ident, ( $sigty:ty ), $(#[doc = $doc:literal]),*)),* $(,)?) => {
		$(println!(
			"slug={} struct={} sig={}",
			stringify!($slug),
			stringify!($strukt),
			stringify!($sig),
 		);)*
	};
}

fn main() {
	acpi_tables!(impl_tables);
}
```

would print:

```
slug=Rsdp struct=acpi_table_rsdp sig=ACPI_SIG_RSDP
slug=Madt struct=acpi_table_madt sig=ACPI_SIG_MADT
slug=Facp struct=acpi_table_facp sig=ACPI_SIG_FACP
...
```

> [!IMPORTANT]
> Modifications or higher-level abstractions, or any additional Rust
> code, are not to be placed in this repository. Please use the `oro-acpi` crate
> in the [kernel](https://github.com/oro-os/kernel) for such functionality.

> [!NOTE]
> This crate does not actually link to any functional C code. All
> exports are data structures or constants only. No PRs to change this will
> be accepted.

## License
This codebase is licensed under Intel's [modified BSD license](LICENSE).

It is part of the [Oro Operating System project](https://github.com/oro-os).
Note that Oro-specific modifications do _not_ live here.
