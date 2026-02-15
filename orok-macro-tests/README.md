# Oro Kernel Macro Tests

This crate houses the unit tests for the `orok-macro` and `orok-macro-proc` crates.
It is a member of the Oro kernel workspace, but is not a dependency of any other crate.

It's broken out into its own crate since the procedural macros in `orok-macro-proc`
might refer to support items in `orok-macro` by name (e.g. `::orok_macro::SomeSupportItem`),
but references to `::orok_macro` inside tests within `orok-macro` would result in a compilation error.

Thus, we use a third crate to test both `orok-macro` and `orok-macro-proc` to properly test
these macros as though they're being used in a downstream crate.