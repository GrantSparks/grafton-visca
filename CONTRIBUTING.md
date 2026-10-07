# Contributing to grafton-visca

Thank you for your interest in contributing to grafton-visca! This guide will help you understand the project structure and best practices for contributions.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Safety Guidelines](#safety-guidelines)
- [Command Development](#command-development)
- [Camera Profile Support](#camera-profile-support)
- [Testing](#testing)
- [Public API Snapshots](#public-api-snapshots)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

## Getting Started

Before contributing, please:

1. Read the [README.md](README.md) to understand the project architecture
2. Review existing issues and pull requests
3. Fork the repository and create a feature branch

## Development Setup

### Toolchains

CI pins every toolchain to an exact version in `.github/workflows/ci.yml`.
Compile-contract fixtures declare exact error codes and stable diagnostic
anchors in their source, while the `api/2.0.0-rc.1/*.txt` public API snapshots
are compared byte for byte. Reproducing a CI failure locally means using the
same versions:

```bash
rustup toolchain install 1.98.0          # stable jobs and compile contracts
# `--profile minimal` installs rustc/cargo/rust-std only, so the components the
# nightly jobs actually use must be named explicitly. `--component` takes one
# comma-separated list; a space-separated second name is parsed as another
# toolchain, not as a component.
rustup toolchain install nightly-2026-08-26 --profile minimal --component rustfmt,miri  # fmt, Miri, fuzz, API snapshots
rustup toolchain install 1.88.0          # MSRV job
```

A newer stable can change diagnostic prose. Update a fixture's in-source
message anchor only when the workflow's pinned stable is bumped in the same
change and the new diagnostic still proves the intended contract. Regenerating
the public API snapshots is documented in `api/2.0.0-rc.1/README.md`.

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/grafton-visca.git
cd grafton-visca

# Run the canonical local/release validation matrix. CI builds one job per
# feature shape from this script's `list-json` mode, so both share one list.
bash .github/scripts/test-all-features.sh

# Or run individual matrix entries while iterating
cargo test
cargo test --no-default-features
cargo test --no-default-features --features async
cargo test --no-default-features --features runtime-tokio
cargo test --no-default-features --features runtime-smol
cargo check --no-default-features --features runtime-tokio,runtime-smol
cargo test --no-default-features --features transport-serial
cargo test --no-default-features --features runtime-tokio,transport-serial-tokio
cargo test --no-default-features --features serde,schemars,ts-rs
cargo test --no-default-features --features test-utils
cargo test --no-default-features --features runtime-tokio,test-utils
cargo test --no-default-features --features runtime-smol,test-utils
cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils --test dyn_api_integration_test
cargo test --no-default-features --features runtime-smol,dyn-api,test-utils --test dyn_api_smol_integration_test

# Feature-union checks for combinations that may appear downstream
cargo test --no-default-features --features runtime-tokio,transport-serial
cargo test --no-default-features --features runtime-smol,dyn-api

# Run clippy checks
cargo clippy --all-targets --all-features -- -D warnings

# Bounded named pure-library Miri suite plus feature compile checks
bash .github/scripts/miri-tests.sh

# Format code
cargo fmt
```

## Code Style

Commit hooks check formatting with the CI-pinned nightly formatter, lint the
default feature set once, and run text hygiene checks. The default feature set
already enables `blocking`; it does not need a second blocking-only lint pass.
CI owns broad feature testing, all-feature and no-default linting, and rustdoc.

For a feature-specific lint investigation, run an opt-in hook, for example
`pre-commit run clippy-runtime-smol --hook-stage manual --all-files`.
`pre-commit run --hook-stage manual --all-files` runs all the extra lint and
rustdoc hooks. Use these when the change warrants them, rather than stacking
the entire local matrix on top of a successful CI run for the same source.

We maintain high code quality standards:

- **Format**: Use `cargo fmt` before committing
- **Linting**: Fix all `cargo clippy` warnings (including pedantic)
- **Documentation**: All public APIs must have doc comments with examples
- **Tests**: Add tests for new functionality

## Safety Guidelines

### Protocol Safety with VISCA_TERMINATOR

All VISCA commands MUST be properly terminated with the `VISCA_TERMINATOR` byte (0xFF). This is critical for protocol compliance and camera communication safety.

**Important**: Build every frame through the crate's frame writer, which adds the camera address byte and the terminator. Command and inquiry bodies are address-free byte sequences from `command::bytes::constants` (or the built-in inquiry table):

```rust
use crate::command::bytes::{constants::zoom, FrameWriter, Step};

// GOOD: the frame writer adds the address byte and the terminator
FrameWriter::new(camera_id, buffer)
    .bytes(&zoom::DRIVE)
    .byte(Step::Up.byte())
    .finish()

// BAD: Manual byte construction without terminator
let command = [0x81, 0x01, 0x04, 0x00]; // Missing 0xFF terminator!
```

### Physical Safety Considerations

PTZ cameras involve physical movement that can cause damage or injury if not handled properly:

1. **Movement Commands**: Always document safety warnings for pan/tilt/zoom operations
2. **Speed Limits**: Respect maximum speed parameters to prevent mechanical damage
3. **Position Bounds**: Implement and respect position limits
4. **Emergency Stop**: Ensure stop commands are easily accessible

Example safety documentation:

```rust
/// Moves the camera to absolute pan/tilt position.
///
/// # Safety
///
/// This command physically moves the camera hardware. Ensure:
/// - Clear operating area around the camera
/// - No obstructions in the movement path
/// - Appropriate speed settings for your environment
```

## Command Development

When implementing new VISCA commands:

The owner-facing API is built from the typed `Request`, `Inquiry`, and
`OperationCommand` contracts. Custom wire values should implement `Request`
directly; the protocol encoder is an internal implementation detail and does
not by itself select a completion class or lifecycle. Keep new semantic
classifications in the authoritative request/command ledger.

### 1. Use a typed Request for Safe Encoding

Typed `Request` implementations make semantic admission explicit while keeping
wire encoding bounded and allocation-free:

```rust
use grafton_visca::{CameraId, Request};

impl Request for MyCommand {
    type Class = grafton_visca::request::Plain;
    const MAX_SIZE: usize = 8;
    const TIMEOUT_CLASS: grafton_visca::TimeoutClass = grafton_visca::TimeoutClass::Quick;
    const RETRY_CLASS: grafton_visca::RetryClass = grafton_visca::RetryClass::Standard;
    const CONTROL_CLASS: grafton_visca::ControlClass = grafton_visca::ControlClass::Normal;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        // Encode the complete terminated VISCA frame into `buffer`.
        let _ = (camera_id, buffer);
        todo!("encode MyCommand with the bounded command-byte helpers")
    }
}
```

### 2. Define Proper Buffer Sizes

Each request declares `Request::MAX_SIZE`; it includes every byte through the
final `VISCA_TERMINATOR`.

### 3. Implement Inquiry Pattern

For inquiry commands, implement `Request<Class = request::Inquiry>` and
`Inquiry`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyInquiry;

impl Request for MyInquiry {
    type Class = request::Inquiry;
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        // Use the bounded command-byte helpers for safe construction
    }
}

impl Inquiry for MyInquiry {
    type Response = MyResponse;

    fn route(&self) -> InquiryRoute { /* select the response route */ }
    fn decoder(&self) -> ResponseDecoder<Self::Response> { /* decode response */ }
}
```

### 4. Adding a Built-in Command

A built-in command is declared in four places, and every other projection is
generated from them:

1. **Ledger row** (`src/command/semantics.rs`): one `builtin_command_ledger!`
   row gives the command its `BuiltinCommand` identity and its class
   (`plain`, `applied(<AXES>)` or `targeted(<AXES>)`), plus any write-only
   state effect.
2. **Wire encoder** (`src/command/<domain>.rs`): the request type and its
   `WireEncode` implementation, written through `FrameWriter`.
3. **Request entry** (`src/request/builtin.rs`): one `builtin_request!` entry
   names the request type's ledger row and its policy. Each entry still gives
   an explicit `size:`, the `MAX_SIZE` allocation bound for the request's
   longest frame, while the exact encoded length is measured from the encoder.
   Add the per-value checks to its `BuiltinValidation` implementation; a typed
   capability gate comes from the ledger row. A type whose values serve
   different ledger rows adds
   `rows: |value| match value[.field] { Variant => Row, ... }`, an exhaustive
   match whose patterns are variant paths (a wildcard is rejected), and its
   validator gates each value on the row it selects, `self.ledger_row()`.
4. **Noun row** (`src/noun_table.rs`): one row under the owning `@noun` header
   gives the method name, arguments, rustdoc, capability `where` marker and
   request. The blocking, async and `Dyn*` methods, the static surface entry
   and the typed-request inventory are all expanded from it.

The compiler rejects a ledger row without a noun row, a noun row whose kind
disagrees with the ledger class, and a request type without a `rows:` entry
whose noun rows disagree on their capability gate. For a type with a `rows:`
entry, every noun row that
sends it must be a `[by_value <const expr>]` row whose value selects that noun
row's ledger row. A typed built-in inquiry is added to
`define_builtin_inquiries!` in `src/command/inquiry_structs.rs` and gets its
runtime gate from its noun row in the same way.

Then:

- extend the wire golden (`src/issue_715_wire_golden.rs` and
  `tests/fixtures/issue_715_wire_golden.txt`);
- update the semantic class tripwire in `src/command/surface.rs`
  (`surface_ledger_is_exhaustive_unique_and_class_balanced`) and name the class
  the row landed in in the commit message;
- regenerate the public API snapshots (see [Public API Snapshots](#public-api-snapshots)).

## Camera Profile Support

Camera profiles are part of the public type-safety contract. Before adding or
changing a profile capability, follow the
[Camera Profile Support Guide](docs/camera_profile_support.md) and update the
[VISCA Protocol Reference](docs/visca_reference.md) when new source evidence is
needed.

The short version:

- `docs/visca_reference.md` is the checked-in source of truth for protocol,
  model, and firmware evidence.
- Model-specific capability docs take precedence over generic opcode tables.
- Runtime metadata traits feed `Capabilities::from_profile::<P>()`.
- Support marker traits such as `HasNdFilter`, `HasMotionSync`, and
  `HasVariableSpeed` expose typed APIs and must only be implemented when the
  source docs establish support.
- Raw VISCA command APIs remain available for experiments and downstream camera
  variants, but raw opcode availability does not justify marking a built-in
  profile as supported.
- Land capability and registry refactors atomically. Path-dependency consumers
  should not observe a half-applied tree where profile modules, generated macros,
  and marker impls disagree.

## Testing

### Unit Tests

Add unit tests for all new functionality:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_encoding() {
        let cmd = MyCommand::new();
        let mut buffer = [0u8; MyCommand::MAX_SIZE];
        let len = cmd.write_into(CameraId::default(), &mut buffer).unwrap();

        // Verify VISCA_TERMINATOR is present
        assert_eq!(buffer[len - 1], VISCA_TERMINATOR);
    }
}
```

### Integration Tests

For camera control features, use the public utilities under
`grafton_visca::testing::testkit` with the `test-utils` feature. Prefer
`ScriptedBlockingTransport` or `ScriptedTransport` for protocol scripts and a
real Tokio/smol executor for wall-clock timeout behavior. See
`src/testing/testkit/README.md` and the existing operation-handle integration
tests for maintained patterns.

Integration tests that need a fake camera use `tests/common/fake_camera.rs`
(`FakeCamera` with `blocking_wire()`/`async_wire()`, the shared reply frames
and wait helpers); scenarios that run on several facades or runtimes use
`facade_matrix!`/`runtime_matrix!` from `tests/common/matrix.rs`. A
hand-written transport stays local only with a `// Local fake:` comment.

### Running Tests

```bash
# Default test suite
cargo test

# Canonical local/release validation commands. CI runs the feature matrix as
# separate jobs rather than this command block verbatim.
bash .github/scripts/test-all-features.sh
cargo test
cargo test --no-default-features --features runtime-tokio
cargo test --no-default-features --features runtime-smol
cargo check --no-default-features --features runtime-tokio,runtime-smol
cargo test --no-default-features --features transport-serial
cargo test --no-default-features --features runtime-tokio,transport-serial-tokio
cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils --test dyn_api_integration_test
cargo test --no-default-features --features runtime-smol,dyn-api,test-utils --test dyn_api_smol_integration_test

# Feature-union checks
cargo test --no-default-features --features runtime-tokio,transport-serial
cargo test --no-default-features --features runtime-smol,dyn-api

# Bounded named pure-library Miri suite plus feature compile checks
bash .github/scripts/miri-tests.sh

# With output for debugging
cargo test -- --nocapture
```

### Test time budget

The whole feature matrix (`test`, `clippy` and `doc` modes) should finish in
under ten minutes of wall time on a many-core machine when run in parallel:

```bash
# N jobs at a time, each in its own target directory under target/matrix;
# per-job logs and a summary land in target/matrix/logs/<mode>/.
bash .github/scripts/test-all-features.sh --jobs 12 test

# The 15 slowest tests of one shape (pinned nightly --report-time; timings only)
bash .github/scripts/test-all-features.sh slowest 15 --all-features
```

- No single test should need more than a few seconds of wall time; the
  parallel summary lists any test libtest reports as running for over 60 s.
  The known exception is `api_stability_test::canonical_compile_contracts`,
  which compiles about 96 contract fixtures for the leg's features: about
  3-15 s on a warm nested target, and over 60 s in a cold, fully loaded
  parallel run, when it first builds the crate for its fixtures.
- Each parallel job keeps its own target directory, so a full `test`,
  `clippy` and `doc` run holds about 66 GB under `target/matrix` (91
  directories, measured on Linux with line-tables-only debug info). Delete it
  when you need the space; the next run rebuilds it.
- Wait on deadlines in virtual time where the owner allows it: write the
  scenario as `facade_matrix! { paused: ... }` so its Tokio case runs on paused
  time. The blocking and smol owners read the real clock.
- A heavy test whose behaviour cannot differ by feature configuration runs in
  one designated leg: list it in `.github/scripts/engine-model-tests.sh` only
  with evidence that no `cfg(feature = ...)` reaches its code path. The matrix
  fails if a listed test runs anywhere else, or not exactly once in its leg.

### Built-in wire golden

`tests/fixtures/issue_715_wire_golden.txt` pins the encoded bytes of every
built-in command. After a deliberate encoder change, regenerate it with:

```bash
GRAFTON_VISCA_BLESS_WIRE_GOLDEN=1 cargo test --lib wire_ledger_matches_literal_golden_inventory
```

The bless run writes the fixture and then fails on purpose ("fixture blessed;
review the diff and rerun without the variable"), and it is refused when `CI`
is set. Review the fixture diff, then rerun the test without the variable.

## Public API Snapshots

`api/2.0.0-rc.1/` holds the rolling approved public-surface baseline for the
2.0 release-candidate line: one `cargo public-api` listing per tracked feature
surface. The directory name records where the baseline was established; it is
not a per-release archive. Approved prerelease API changes regenerate these
files in place, while immutable release tags preserve their historical bytes.
The `public API RC snapshot` CI job regenerates each listing and byte-compares
it with the committed file, so any change to the crate's public API — including
one you did not intend, such as a type escaping its feature gate or a public
type quietly losing `Send` — fails the job with a readable diff instead of
reaching downstream users.

Three surfaces are tracked, and they are deliberately few: `blocking` subsumes
the bare no-default-features surface, `tokio-dyn` subsumes the async surface,
and `all-features` covers everything else. `--simplified` drops compiler-emitted
blanket impls, which are ~42% of the raw output and identical for every public
type. The baselines are CI-only; `api/` is excluded from the published crate.

**If the job fails on your PR**, decide which case you are in:

- The API change is intended. Regenerate the snapshots and commit them
  alongside the change, so the diff is reviewed with the code that caused it.
- The API change is not intended. Fix the code — the snapshot is telling you
  something leaked.

Regenerate with the pinned toolchain, never with a floating `+nightly`; the
exact commands and the pins live in
[api/2.0.0-rc.1/README.md](api/2.0.0-rc.1/README.md). Snapshot bytes depend on
the nightly rustdoc that produced them, so bumping the pinned nightly in
`.github/workflows/ci.yml` requires regenerating all three files in the same
commit.

## Documentation

For 2.0 work, update the Unreleased section of `CHANGELOG.md` in the same
change as the implementation. The changelog records what changed between
published versions, and Unreleased states the net change since the last
published version (the newest `v<version>` tag), not the history of how it was
reached: when a change revises an item that Unreleased already describes,
rewrite that entry to state the net result instead of adding another. If
behavior, setup, examples, or contributor workflow changes, update the matching
README, example, or contributor docs before closing the task. `submit` examples
must distinguish lifecycle management from profile-aware input validation and
applied completion from physical settling.

### Changelog discipline for reversals and waivers

A decision must not reuse the issue number of the finding it reverses. When a
change reverts or supersedes behavior that a published version shipped — a
prior review verdict included — it gets its own `### Changed` or `### Removed`
entry under Unreleased that names the superseded finding by its issue number.
Reversing a decision made after the last published version needs no record of
its own: rewrite or drop the Unreleased entry so that it states only the net
change from the published version. An item added and removed again before the
next publication, or an internal fix of a regression that never shipped, gets
no entry.

`docs/migration_2_0.md` records the differences between 1.2 and 2.0 only. Add
or update a row there wherever a 1.x user would feel a change; do not add rows
or notes for differences between 2.0 prereleases, and do not describe an API
that did not exist in 1.2 as a migration source.

The `release-validation` CI job enforces four parts of this record mechanically:

- Every published release section is byte-for-byte immutable relative to the
  pull request's merge base. A release is published once its `v<version>` tag
  exists; a section below a published one counts as published even without a
  tag. Only the top `## [Unreleased]` body and dated sections above the newest
  published one that have no tag (prepared but never published) may change,
  and such an untagged section may be folded back into Unreleased. A release
  cut that moves the prior Unreleased lines in order into one new strict dated
  section below an otherwise empty Unreleased heading is also accepted, but
  only once no unpublished section remains below it: fold those first. Each
  version has exactly one dated heading.
  Correct a published statement with an entry under Unreleased; do not edit the
  published entry. The validator needs the release tags, so run it from a
  full-history checkout with tags; it fails rather than guess when none are
  present.
- A change to `api/2.0.0-rc.1/*.txt` must include a `CHANGELOG.md` change in the
  same pull request.
- Every top-level Unreleased bullet carrying the exact `**BREAKING**` label must
  include an issue reference in the form `(#NNN)` or `(#NNN, #MMM)`.
- Every new commit that touches `src/` must have a non-empty explanatory body,
  not only a subject. The policy-boundary files under `.github/` bootstrap the
  repaired historical text and grandfather the already-audited PR #559 commit
  ledger; they must not be advanced to excuse later changes.

From a full-history checkout, run the validator and its deliberate-failure and
acceptance fixtures with:

```bash
python3 .github/scripts/validate-change-record.py "$(git merge-base HEAD origin/main)" HEAD
bash .github/scripts/test-validate-change-record.sh
```

When a change alters observable protocol behavior, add or update a direct
regression or wire/decode golden in the production owner, engine, or parser
path, and record the caller-visible consequence in `CHANGELOG.md`, and in
`docs/migration_2_0.md` when it differs from 1.2. Review the new behavior
against the implementation and its tests directly; do not preserve a behavior
merely because an earlier release had it if that would make correlation,
framing, cancellation, or physical safety less certain.

### Code Documentation

- All public items must have doc comments
- Include examples in doc comments
- Document safety considerations for movement commands
- Use `#[must_use]` where appropriate

### Example Documentation

````rust
/// Return a camera to its home position and wait for physical settling.
///
/// # Examples
///
/// ```no_run
/// # use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2};
/// let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
/// let camera = session.camera();
/// camera.pan_tilt().home()?.settled()?;
/// session.close()?;
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if submission, protocol completion, fallback settling, or
/// shutdown fails.
````

## Pull Request Process

1. **Branch Naming**: Use descriptive branch names (e.g., `feat/add-nd-filter`, `fix/timeout-handling`)

2. **Commit Messages**: Follow conventional commits:
   - `feat:` New features
   - `fix:` Bug fixes
   - `docs:` Documentation changes
   - `test:` Test additions/changes
   - `refactor:` Code refactoring

3. **PR Description**: Include:
   - Problem being solved
   - Implementation approach
   - Safety considerations (if applicable)
   - Testing performed

4. **Checklist**: Before submitting:
   - [ ] Declared cfg-aware matrix passes: `bash .github/scripts/test-all-features.sh`
   - [ ] No clippy warnings in any supported feature shape: `bash .github/scripts/test-all-features.sh clippy`
   - [ ] Rustdoc builds without warnings in every supported feature shape: `bash .github/scripts/test-all-features.sh doc`
   - [ ] Formatted: `cargo +nightly fmt --all -- --check`
   - [ ] Rustdoc and doctests pass for blocking, Tokio, and all-feature surfaces
   - [ ] Documentation updated
   - [ ] CHANGELOG.md updated (if applicable)
   - [ ] Safety documented for movement commands

5. **Review Process**:
   - Address reviewer feedback promptly
   - Keep discussions focused and professional
   - Update PR description with any significant changes

## Release Process

Releases use a two-crate prerelease/final publish sequence because the main crate
depends on the same-version `grafton-visca-macros` package. Follow
[RELEASING.md](RELEASING.md) for `2.0.0-rc.3` versioning, changelog
finalization, validation, tagging, crates.io index verification, and recovery if
the macro package publishes but the main package does not. Never create a
release tag from a commit that has not passed the complete 2.0 matrix — public
API snapshots included — on a pull request.

`cargo-semver-checks` is not part of that matrix while 2.0 is a release
candidate. Prerelease API drift is reviewed directly in the pinned public API
snapshots, and comparing with 1.x would only re-report the intended major
break. It returns once 2.0.0 is published.

## Feature Flags

When adding features that require new dependencies:

1. Make them optional via feature flags
2. Document the feature in Cargo.toml
3. Update README.md with feature documentation
4. Ensure tests work with and without the feature

## Performance Considerations

- Prefer const functions where possible
- Use zero-allocation patterns (see `FrameWriter`, which writes straight into the caller's buffer)
- Avoid unnecessary heap allocations
- Profile performance-critical code paths

## Questions?

If you have questions about contributing:

1. Check existing issues and discussions
2. Review the API documentation
3. Open an issue for clarification
4. Join discussions on implementation approaches

Thank you for helping make grafton-visca better and safer for everyone!
