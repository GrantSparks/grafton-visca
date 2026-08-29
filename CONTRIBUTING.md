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

# Run the declared 2.0 release-candidate support matrix
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

# Optional safety checks
cargo +nightly miri setup
cargo +nightly miri test --lib --no-default-features

# Format code
cargo fmt
```

## Code Style

We maintain high code quality standards:

- **Format**: Use `cargo fmt` before committing
- **Linting**: Fix all `cargo clippy` warnings (including pedantic)
- **Documentation**: All public APIs must have doc comments with examples
- **Tests**: Add tests for new functionality

## Safety Guidelines

### Protocol Safety with VISCA_TERMINATOR

All VISCA commands MUST be properly terminated with the `VISCA_TERMINATOR` byte (0xFF). This is critical for protocol compliance and camera communication safety.

**Important**: Always use the `ConstCommandBuilder` or similar safe abstractions that automatically handle termination:

```rust
use crate::command::bytes::{ConstCommandBuilder, VISCA_TERMINATOR};

// GOOD: Using CommandBuilder ensures proper termination
let builder = ConstCommandBuilder::<7>::new()
    .append(command_bytes)
    .with_camera_id(camera_id)
    .terminate();  // Automatically adds VISCA_TERMINATOR

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

### Running Tests

```bash
# Default test suite
cargo test

# Declared 2.0 runtime and transport feature combinations
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

# Optional interpreter-based UB checks for library tests
cargo +nightly miri test --lib --no-default-features

# With output for debugging
cargo test -- --nocapture
```

## Public API Snapshots

`api/2.0.0-rc.1/` holds the approved public-surface baseline for the 2.0
release candidate: one `cargo public-api` listing per tracked feature surface.
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

For 2.0 release-candidate work, update the Unreleased section of `CHANGELOG.md` in the
same change as the implementation. If behavior, setup, examples, or contributor
workflow changes, update the matching README, example, or contributor docs
before closing the task. `submit` examples must distinguish lifecycle management
from profile-aware input validation and applied completion from physical settling.

### Code Documentation

- All public items must have doc comments
- Include examples in doc comments
- Document safety considerations for movement commands
- Use `#[must_use]` where appropriate

### Example Documentation

```rust
/// Return a camera to its home position and wait for physical settling.
///
/// # Examples
///
/// ```no_run
/// # use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2};
/// let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
/// let camera = session.camera::<PtzOpticsG2>()?;
/// camera.pan_tilt().home()?.settled()?;
/// session.close()?;
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if submission, protocol completion, fallback settling, or
/// shutdown fails.
```

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
   - [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
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
[RELEASING.md](RELEASING.md) for `2.0.0-rc.1` versioning, changelog
finalization, validation, tagging, crates.io index verification, and recovery if
the macro package publishes but the main package does not. Never create a
release tag from a commit that has not passed the complete 2.0 matrix — public
API snapshots included — on a pull request.

`cargo-semver-checks` is not part of that matrix while 2.0 is a release
candidate: 2.0.0-rc.1 is unpublished, so there is no crates.io baseline to
compare against, and comparing with 1.x would only re-report the intended major
break. It returns once 2.0.0 is published.

## Feature Flags

When adding features that require new dependencies:

1. Make them optional via feature flags
2. Document the feature in Cargo.toml
3. Update README.md with feature documentation
4. Ensure tests work with and without the feature

## Performance Considerations

- Prefer const functions where possible
- Use zero-allocation patterns (see `ConstCommandBuilder`)
- Avoid unnecessary heap allocations
- Profile performance-critical code paths

## Questions?

If you have questions about contributing:

1. Check existing issues and discussions
2. Review the API documentation
3. Open an issue for clarification
4. Join discussions on implementation approaches

Thank you for helping make grafton-visca better and safer for everyone!
