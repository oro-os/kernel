# `saturating_cast`
[![Build Status](https://github.com/okaneco/saturating_cast/workflows/Rust%20CI/badge.svg)](https://github.com/okaneco/saturating_cast)
[![Crates.io](https://img.shields.io/crates/v/saturating_cast.svg)](https://crates.io/crates/saturating_cast)
[![Docs.rs](https://docs.rs/saturating_cast/badge.svg)](https://docs.rs/saturating_cast)

Library for saturating casts between integer primitives.

## Features

- `no_std` by default
- Saturating casts implemented between all of the following:
  - `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
  - `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
- Saturating traits can be implemented for user types

## Description

A saturating cast is a combined [`clamp`][clamp] and cast from one type to
another, conceptually `value.clamp(T::MIN, T::MAX) as T`.

[clamp]: https://doc.rust-lang.org/stable/std/cmp/trait.Ord.html#method.clamp

## Example

In the following code, the `i32` value is clamped to `255` since `u8` has a
range of `0..=255` and `1024` is larger than the maximum value that `u8` can
represent.

In the second snippet, an `i8` with negative sign is saturated to `0` when cast
to `u16` because unsigned integers cannot be negative.

```rust
use saturating_cast::SaturatingCast;

let x: i32 = 1024;
let y: u8 = 10;
let z = x.saturating_cast::<u8>() - y;
assert_eq!(245, z);

let a = -128_i8;
let b: u16 = a.saturating_cast();
assert_eq!(0, b);
```

Saturation **does not** preserve the original value if the source type value
is out of range for the target type: the resulting value will be the minimum or
maximum of the target type.

This crate only casts and returns primitive integers. Saturating arithmetic is
still required to avoid wraparound in `release` and panicking in `debug` mode
(or with overflow checks on in `release`).

## License
This crate is licensed under either
- the [MIT License](LICENSE-MIT), or
- the [Apache License (Version 2.0)](LICENSE-APACHE)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
