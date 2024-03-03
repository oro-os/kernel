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
