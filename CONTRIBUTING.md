# Contributing to the Oro Operating System Kernel

<table>
	<tr>
		<th align="left"><strong>Status</strong></th>
		<td>Current</td>
	</tr>
	<tr>
		<th align="left"><strong>Last Review Date</strong></th>
		<td>21 June 2026</td>
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

- All libraries used in the kernel itself must be `no_std` compatible.
- All libraries must be licensed under an OSI-approved license,
  compatible with the [LICENSE](license/oro-kernel.txt) of the
  Oro Operating System kernel project.
- All libraries must be well-maintained and have a clear path for
  security updates and bug fixes.
- No library shall be exposed directly outside of the crate within which
  it is used. Corollary: all third-party libraries must have Oro-specific
  abstractions, and must be written in a way that they may be replaced
  with a different library (or homegrown implementation) in the future.
- Library versions must be pinned to the _most specific_ version possible.
  This means that `^1.0.0` is not acceptable, but `=1.0.0` is. This is to
  ensure that the kernel is reproducible and that we can guarantee that
  the kernel will build in the future. CI currently enforces this.

## Use of `unsafe`
The kernel project uses `unsafe` judiciously. In some cases, especially
in a codebase of this nature, `unsafe` is quite obviously unavoidable.
However, we strive to minimize the use of `unsafe` as much as possible.

Thus, use of `unsafe` will be scrutinized heavily. Please prepare for this.

### Use in Traits vs Functions
The kernel project has a clear standarization on where `unsafe` is used
with regards to traits (i.e. `unsafe trait` itself vs an `unsafe fn` within a trait):

- When an **implementation** must adhere to a safety invariant, the trait
  must be marked as an `unsafe trait` and a `# Safety` doc comment must
  be included. This is true even if a safety requirement pertains to a subset
  of methods.
- When the **caller** must adhere to a safety invariant, the method
  must be marked as `unsafe fn`. This is true regardless of if the trait itself
  is marked as `unsafe trait`.

### Usage of `unsafe` in non-soundness-critical code
All cases of potential memory unsoundness must employ `unsafe` and a `# Safety` doc
comment.

However, there are some cases where `unsafe` is not _necessary_ but is used to indicate
potentially dangerous code if used incorrectly - i.e. forcing the developer to acknowledge
some pre- or post-condition. These cases are not _necessarily_ about memory safety or
soundness, but since all uses of `unsafe {}` must be acknowledged with a `// SAFETY: ...`
comment, this forces developers to explain how the specific call site has done its
due-diligence to ensure the safety of the call.

## Porting from Other Operating Systems
The kernel project is not a dumping ground for code from other operating
systems. However, some code may be ported from other operating systems
if it is necessary, and **only if the code is licensed under an OSI-approved
license compatible with the kernel project's** (see
[the Oro Operating System kernel license](license/oro-kernel.txt)).

In such a case, the code must adhere to **all** guidelines in this document,
must have proper attribution to the original authors in the form of a module-
or item-level comment (depending on the breadth of the port), and must be
accompanied by a clear explanation of why the code was ported and how it
fits into the Oro kernel project.

Code from any corporate-owned operating system (Windows, MacOS/Darwin, etc.)
will not be accepted.

Code from the Linux kernel or other open-source operating systems similarly
licensed under a compatible OSI-approved license _may_ be accepted with proper
attribution, but will be scrutinized heavily to ensure it is used fairly
and appropriately. Prepare to defend your reasoning for porting such code.
Not properly attributing code from other operating systems will result in a
permanent ban from the project in perpetuity at the discretion of the maintainers.

Further, code written in languages other than Rust will not be accepted.

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

Conversely, over-documented code (comments that explain the obvious) is
discouraged and will likely be removed during review. Use your best judgement;
if something isn't right, the reviewers will let you know.

Any use of `unsafe` must be accompanied by a `# Safety` section in the
documentation (or for `unsafe{}` blocks, a `// SAFETY: ...` comment),
explaining why the `unsafe` is necessary and what invariants
must be upheld for the `unsafe` code to be safe. This is also enforced
by CI.

## Code Style
The kernel project uses `rustfmt` to enforce a consistent code style.
Please run `cargo fmt` prior to committing and pushing your changes.

If you are making a change to the code style configuration, please do the
following when submitting a pull request:

1. Create a single commit with the configuration changes.
2. Create a secondary commit (after the first) with the `cargo fmt` changes.

Any code changes that are not purely code style changes should be in a
separate pull request, and will not otherwise be accepted.

Manual style changes will probably not be accepted. Open an issue first
to discuss the changes you would like to make prior to submitting a
pull request. In certain cases, the use of `#[rustfmt::skip]` may be acceptable,
but this should be used sparingly and only when necessary. Be prepared to
justify the reasoning in your pull request.

## Warnings, Lints, etc.
The kernel project uses `clippy` to enforce a consistent code style.
Please run `cargo oro clippy` prior to committing and pushing your changes.

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
ensure the highest quality of code.
**Please prepare for this when contributing.**

## IDEs and Editors
The kernel project does not enforce the use of any particular IDE or
editor. That being said, IDE-/editor-specific files or other configuration
artifacts should not be committed to the repository. This includes
`.vscode`, `.idea`, `.vim`, and other similar directories or files.

If your editor or IDE emits files that are not already listed in
`.gitignore`, please add them to `.git/info/exclude` instead of
`.gitignore` if you intend to submit a pull request.

To be useful, there are a few in-repo things that will help with
local development:

### Rust-Analyzer

`rust-analyzer` will probably not work out-of-the-box for your IDE, as
`rust-analyzer`'s usual `check` command will not pick up on all of the
target configuration for each of the artifacts.

Instead, the `cargo oro` utility provides a check:

```shell
cargo oro check --json
cargo oro clippy --json    # Slower, but checks for more things.
```

One of these should be provided as the `rust-analyzer.check.overrideCommand`
setting in your editor, IDE, or other tooling.

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
		"cargo oro check --json"
	]
}
```

## Reporting a Vulnerability
If you have found a vulnerability within the Oro kernel or any of the associated
crates included in this repository, **please do not open an issue** and instead
consult [SECURITY.md](SECURITY.md) for instructions on how to responsibly disclose
your findings.

## Code of Conduct
Hopefully this doesn't need to be over-explained. Oro maintainers reserve
the right to deny interaction with the project for any reason, at their
discretion, for any duration or by any means they see fit. This includes,
but is not limited to, banning from the issue tracker, pull requests,
CI/CD pipeline, and any other communication medium, either temporarily
or indefinitely.

These terms will certainly be more pointed in the future, but for now,
here are a few things that will almost definitely result in a ban from the project:

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

Further, any violation of the legalities of the project (blatant theft
or plagiarism from other projects when contributing, submitting code that
is reverse engineered from proprietary systems, etc.) will result in a
permanent ban from the project in perpetuity at the discretion of the maintainers.
