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
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)

## Getting Started

Before contributing, please:

1. Read the [README.md](README.md) to understand the project architecture
2. Review existing issues and pull requests
3. Fork the repository and create a feature branch

## Development Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/grafton-visca.git
cd grafton-visca

# Run the declared 1.x support matrix
bash .github/scripts/test-all-features.sh

# Or run individual matrix entries while iterating
cargo test
cargo test --no-default-features
cargo test --no-default-features --features mode-async
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

# Compatibility-only checks for feature unions that may appear downstream
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

### 1. Use CommandBuilder for Safe Encoding

The `CommandBuilder` pattern ensures type-safe, zero-allocation command construction:

```rust
use crate::command::bytes::{ConstCommandBuilder, constants};

impl ViscaCommand for MyCommand {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let builder = ConstCommandBuilder::<8>::new()
            .append(constants::COMMAND_PREFIX)
            .append_u16(self.value)
            .with_camera_id(camera_id)
            .terminate();

        builder.build_into(buffer)
    }
}
```

### 2. Define Proper Buffer Sizes

Each command must define its maximum size:

```rust
impl MyCommand {
    /// Maximum size includes all bytes plus VISCA_TERMINATOR
    pub const MAX_SIZE: usize = 8;
}
```

### 3. Implement Inquiry Pattern

For inquiry commands, follow the established pattern:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyInquiry;

impl ViscaCommand for MyInquiry {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        // Use CommandBuilder for safe construction
    }

    fn behavior(&self) -> CommandBehavior {
        CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::MyInquiry))
    }
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

For camera control features, add integration tests:

```rust
#[test]
#[cfg(feature = "test-utils")]
fn test_movement_safety() {
    use crate::test_utils::MockCamera;

    let cam = MockCamera::new();
    // Test safety bounds and limits
}
```

### Running Tests

```bash
# Default test suite
cargo test

# Declared 1.x runtime and transport feature combinations
bash .github/scripts/test-all-features.sh
cargo test
cargo test --no-default-features --features runtime-tokio
cargo test --no-default-features --features runtime-smol
cargo check --no-default-features --features runtime-tokio,runtime-smol
cargo test --no-default-features --features transport-serial
cargo test --no-default-features --features runtime-tokio,transport-serial-tokio
cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils --test dyn_api_integration_test
cargo test --no-default-features --features runtime-smol,dyn-api,test-utils --test dyn_api_smol_integration_test

# Compatibility-only feature-union checks
cargo test --no-default-features --features runtime-tokio,transport-serial
cargo test --no-default-features --features runtime-smol,dyn-api

# Optional interpreter-based UB checks for library tests
cargo +nightly miri test --lib --no-default-features

# With output for debugging
cargo test -- --nocapture
```

## Documentation

For 1.x milestone work, update the Unreleased section of `CHANGELOG.md` in the same change as the implementation. If behavior, setup, examples, or contributor workflow changes, update the matching README, example, or contributor docs before closing the task.

### Code Documentation

- All public items must have doc comments
- Include examples in doc comments
- Document safety considerations for movement commands
- Use `#[must_use]` where appropriate

### Example Documentation

```rust
/// Controls camera zoom position.
///
/// # Examples
///
/// ```no_run
/// # use grafton_visca::camera::Connect;
/// # use grafton_visca::profiles::GenericVisca;
/// let mut cam = Connect::open_udp_blocking::<GenericVisca>("192.168.0.110")?;
/// cam.zoom_absolute(0x4000)?;
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # Safety
///
/// Rapid zoom changes may affect camera stability. Allow time for
/// mechanical stabilization after large zoom movements.
#[must_use]
pub fn zoom_absolute(&mut self, position: u16) -> Result<(), Error> {
    // Implementation
}
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
   - [ ] Tests pass: `cargo test --all-features`
   - [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
   - [ ] Formatted: `cargo fmt`
   - [ ] Documentation updated
   - [ ] CHANGELOG.md updated (if applicable)
   - [ ] Safety documented for movement commands

5. **Review Process**:
   - Address reviewer feedback promptly
   - Keep discussions focused and professional
   - Update PR description with any significant changes

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
