# flickzeug

![Flickzeug Banner](https://github.com/user-attachments/assets/f9f869e6-b3d1-46b7-86ce-1756a9fee85f)

[![flickzeug on crates.io](https://img.shields.io/crates/v/flickzeug)](https://crates.io/crates/flickzeug)
[![Documentation (latest release)](https://docs.rs/flickzeug/badge.svg)](https://docs.rs/flickzeug/)
[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE-MIT)

A Rust library for computing diffs, parsing and applying patches, and performing three-way merges.

> **Note**: This is a fork of [diffy](https://github.com/bmwill/diffy) maintained by [prefix.dev](https://prefix.dev).

## Highlights

- **Fuzzy patch application**: Apply patches even when line numbers have drifted or context has slightly changed — essential for real-world patching scenarios
- **Battle-tested**: Used in production with thousands of real-world patches from [conda-forge](https://conda-forge.org/), the community-driven collection of conda packages

## Features

- **Diff creation**: Compute differences between texts using Myers' diff algorithm, producing minimal edit sequences
- **Patch parsing & formatting**: Read and write unified diff format (compatible with `git diff`, `diff -u`, etc.)
- **Fuzzy patch application**: Apply patches with configurable fuzzy matching when line numbers don't align exactly, using similarity-based line matching
- **Three-way merge**: Merge changes from two sources against a common ancestor, with conflict detection and multiple conflict marker styles
- **Binary support**: All major APIs have `*_bytes` variants for working with non-UTF-8 content

## Usage

Add `flickzeug` to your `Cargo.toml`:

```toml
[dependencies]
flickzeug = "0.4"
```

### Creating a diff

```rust
use flickzeug::create_patch;

let original = "The quick brown fox\njumps over\nthe lazy dog.\n";
let modified = "The quick brown cat\njumps over\nthe sleepy dog.\n";

let patch = create_patch(original, modified);
println!("{}", patch);
```

### Applying a patch

```rust
use flickzeug::{apply, Patch};

let original = "The quick brown fox\njumps over\nthe lazy dog.\n";
let patch_text = "..."; // unified diff format

let patch = Patch::from_str(patch_text).unwrap();
let result = apply(original, &patch).unwrap();
```

### Three-way merge

```rust
use flickzeug::merge;

let base = "line1\nline2\nline3\n";
let ours = "line1\nmodified by us\nline3\n";
let theirs = "line1\nline2\nline3 changed\n";

let merged = merge(base, ours, theirs).unwrap();
```

## License

This project is available under the terms of either the [Apache 2.0 license](LICENSE-APACHE) or the [MIT license](LICENSE-MIT).

## Acknowledgments

This project is a fork of [diffy](https://github.com/bmwill/diffy) by Brandon Williams. We thank the original author for their excellent work.
