# Contributing to the Oro Operating System Kernel

<table>
	<tr>
		<th align="left"><strong>Last Review Date</strong></th>
		<td>10 Feb 2024</td>
	</tr>
</table>

First off, thank you for considering contributing to the
Oro Operating System kernel project!

The following is a set of guidelines for contributing to the
kernel codebase.

## Dependencies and Abstractions
The kernel project attempts to minimize dependencies, but
recognizes that some libraries simply do a better job than
reinventing the wheel - especially considering the time and
effort required to maintain a library.

A few guidelines when introducing or interacting with dependencies:

- All libraries must be `no_std` compatible.
- All libraries must be licensed under an OSI-approved license,
  compatible with the [LICENSE](LICENSE) of the Oro Operating System
  kernel project.
- All libraries must be well-maintained and have a clear path for
  security updates and bug fixes.
- All libraries must be listed in the workspace (root) `Cargo.toml`
  file and must be pinned to a specific version. Workspace crates that
  then use the dependency must have them listed as `dependency.workspace = true`.
- No library shall be exposed directly outside of the crate within which
  it is used. Corollary: all third-party libraries must have Oro-specific
  abstractions, and must be written in a way that they may be replaced
  with a different library (or homegrown implementation) in the future.
- Library versions must be pinned to the _most specific_ version possible.
  This means that `^1.0.0` is not acceptable, but `1.0.0` is. This is to
  ensure that the kernel is reproducible and that we can guarantee that
  the kernel will build in the future.

## Use of `unsafe`
The kernel project uses `unsafe` judiciously. In some cases, especially
in a codebase of this nature, `unsafe` is quite obviously unavoidable.
However, we strive to minimize the use of `unsafe` as much as possible,
especially in commons code (such as the `*-common` crates and the `oro-kernel`
crate).

Thus, use of `unsafe` will be scrutinized heavily. Please prepare for this.

### Use in Traits vs Functions
The kernel project has a clear standarization on where `unsafe` is used
with regards to traits:

- When an **implementation** must adhere to a safety invariant, the trait
  must be marked as an `unsafe trait` and a `# Safety` doc comment must
  be included. This is true even if a safety requirement pertains to a subset
  of methods.
- When the **caller** must adhere to a safety invariant, the method
  must be marked `unsafe`. This is true regardless of if the trait itself
  is marked as `unsafe trait`.

### Locality of Invariant Checks
There are a few types of invariant testing that the kernel project performs;
some cause runtime panics, others are compile-time checks, and some, such as
`unsafe_precondition!()`, are a mixture of both.

When a certain item has a safety constraint, any checks for these constraints
that can be performed either in **debug builds** or at **compile-time** should be
performed as close to their **usage** as possible.

In contrast, invariant checks that are performed at **runtime** in **release**
code should be performed as close to the **implementation** as well as
**mutation** as possible.

This is a bit of a broad piece of guidance that is quite context-dependent,
so please use best judgement prior to submitting a pull request. If there are
any issues with the placement of invariant checks, the maintainers will
do their best to provide feedback.

If there are any questions about invariant checks, please open an issue
to discuss prior to opening a pull request.

## Use of `usize`
For now, the kernel project standardizes that page frames are `u64` and
virtual addresses are `usize`. This may change in the future.

## Generics vs `impl` in Function Arguments
Rust supports both `impl Trait` and generics in function arguments, most working
nearly identically. The kernel project has a preference for generic arguments
over `impl Trait` in all cases, even when the generic is only used once.

## Porting from Other Operating Systems
The kernel project is not a dumping ground for code from other operating
systems. However, some code may be ported from other operating systems
if it is necessary, and **only if the code is licensed under an OSI-approved
license compatible with the kernel project's** (see [LICENSE](LICENSE)).

In such a case, the code must adhere to **all** guidelines in this document,
must have proper attribution to the original authors in the form of a module-
or item-level comment (depending on the breadth of the port), and must be
accompanied by a clear explanation of why the code was ported and how it
fits into the Oro kernel project.

Code from the Linux kernel or any corporate-owned
operating system (Windows, MacOS/Darwin, etc.) will not be accepted.

**There are no exceptions.**

## Documentation
All new functions, traits, structs, and type aliases, as well as new modules,
must be properly documented. CI will fail if this is not the case.
Non-publicly-available items can have documentation omitted, but it is
encouraged to document everything anyway (we might ask you to do this
if the item is complex or non-obvious).

Inline comments are encouraged, but not required. However, if you are
writing a particularly complex algorithm or function, please consider
adding comments to explain the code.

Any use of `unsafe` must be accompanied by a `# Safety` section in the
documentation, explaining why the `unsafe` is necessary and what invariants
must be upheld for the `unsafe` code to be safe. This is also enforced
by CI.

## Code Style
The kernel project uses `rustfmt` to enforce a consistent code style.
Please run `cargo fmt --all` prior to committing and pushing your changes.

If you are making a change to the code style configuration, please do the
following when submitting a pull request:

1. Create a single commit with the configuration changes.
2. Create a secondary commit (after the first) with the `cargo fmt --all`
   changes.

Any code changes that are not purely code style changes should be in a
separate pull request, and will not otherwise be accepted.

Manual style changes will probably not be accepted. Open an issue first
to discuss the changes you would like to make prior to submitting a
pull request.

## Warnings, Lints, etc.
The kernel project uses `clippy` to enforce a consistent code style.
Please run `cargo oro-clippy-x86_64` and `cargo oro-clippy-aarch64`
prior to committing and pushing your changes.

## Continuous Integration (CI)
The kernel project uses GitHub Actions for continuous integration.
All pull requests must pass the CI checks before they will be merged, with
no exceptions. This includes all tests, lints, and other checks.

Further, the kernel project uses a custom CI pipeline that runs tests
on real machines. Those machines might uncover issues that are not
(easily) reproducible in an "ideal" (virtualized) environment, such as
QEMU or other virtualization solutions. These checks must also pass
prior to merging, with no exceptions. We understand this may present
a challenge for some contributors, but we believe it is important to
ensure the highest quality of code. **Please prepare for this when
contributing.**

## IDEs and Editors
The kernel project does not enforce the use of any particular IDE or
editor. That being said, IDE-/editor-specific files or other configuration
artifacts should not be committed to the repository. This includes
`.vscode`, `.idea`, `.vim`, and other similar directories or files.

To be useful, there are a few in-repo things that will help with
local development:

### Rust-Analyzer

`.cargo/config.toml` contains a few aliases for `rust-analyzer`'s
usual `check` command. They should be provided as the
`rust-analyzer.check.overrideCommand` setting in your editor, IDE
or other tooling.

They are:

- `cargo oro-ra-aarch64` for checking the aarch64 target
- `cargo oro-ra-x86_64` for checking the x86_64 target

Typically running them in sequence works fine with rust-analyzer
as long as both commands' output is coalesced into a single stream.

For example:

```sh
bash -c "cargo oro-ra-aarch64 && cargo oro-ra-x86_64"
```

#### Usage in VSCode
If you have the [`rust-analyzer` extension](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
installed in VSCode, you can get much better code completion and
support by doing the following:

1. Add `.vscode/` to `.git/info/exclude` (please do not add it
   to `.gitignore` if you intend to submit a pull request).
2. Create a `.vscode/settings.json` file with the following content:

```json
{
	"rust-analyzer.check.overrideCommand": [
		"/usr/bin/env",
		"bash",
		"-c",
		"cargo oro-ra-aarch64 && cargo oro-ra-x86_64"
	]
}
```

### GDB / QEMU Debugging Utilities
The Oro kernel project ships with a debugging environment called
"dbgutil" for use within GDB using QEMU. Among other things, it
boots the kernel for you via QEMU, wires up the GDB session to
the QEMU instance, and sets up a few useful GDB commands.

Among other things, it has several automatic behaviors that aid in
navigating the kernel (such as switching from the preboot environment
image to the kernel image) as well as ISO generation, instrospection
APIs that wouldn't otherwise be possible with only GDB or QEMU, and
a few other niceties.

Dbgutil is not required for development, but it is highly recommended
for booting and debugging the kernel during development. It's embedded
into the kernel (_not_ bootloader) image and bootstraps itself automatically
when opened with `gdb`.

Full documentation can be found in the [dbgutil/](dbgutil) directory.

## Languages other than Rust
The kernel project is written in Rust, and we typically do not accept
contributions in other languages. If you have a compelling reason to
introduce a new language, please open an issue to discuss it first.
However, be prepared for a "no".

## Reporting a Vulnerability
If you have found a vulnerability within the Oro kernel or any of the associated
crates included in this repository, **please do not open an issue** and instead
consult [SECURITY.md](SECURITY.md) for instructions on how to responsibly disclose
your findings.

## Code of Conduct
Hopefully this doesn't need to be over-explained. Oro maintainers reserve
the right to deny interaction with the project for any reason, at their
discretion.

These terms will certainly be more pointed in the future, but for now,
here are a few things that will get you banned from the project:

- Confrontation in the form of bad-faith arguments, personal attacks, or
  other forms of harassment. "Ad hominem" attacks - whereby the individual
  is criticized rather than the argument - are not acceptable.
- Any form of discrimination.
- Abusing or misusing the issue tracker, pull requests, CI/CD pipeline or
  any other communication medium.
- Making demands of the maintainers or contributors. This is a volunteer
  project, and we are under no obligation to do anything for you. We understand
  that certain features or bug fixes are important to you, and you are of course
  free to voice those urgencies, but becoming hostile in the process is
  not only unhelpful, but it is also countproductive and will discourage
  the maintainers and contributors from working with you.

Further, any violation of the legalities of the project (any legal agreements,
including licenses or contribution agreements, if any) will result in a
permanent ban from the project in perpetuity.
