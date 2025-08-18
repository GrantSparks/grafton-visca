# Contributing to Grafton VISCA

Thank you for your interest in contributing to the Grafton VISCA library! This guide will help you understand our development practices and standards.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/your-username/grafton-visca.git`
3. Create a feature branch: `git checkout -b feature/your-feature`
4. Make your changes and commit
5. Push to your fork and submit a pull request

## Development Setup

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone the repository
git clone https://github.com/GrantSparks/grafton-visca.git
cd grafton-visca

# Build the project with all features
cargo build --all-features

# Run all tests
cargo test --all-features

# Run tests for specific feature combinations
cargo test --no-default-features
cargo test --no-default-features --features async
cargo test --no-default-features --features rt-tokio
cargo test --no-default-features --features test-utils
```

## Code Standards

### Required Checks Before Committing

Run the following commands to ensure code quality:

```bash
# Format code
cargo fmt --all

# Check with clippy (must pass without warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Test all feature combinations
cargo test --no-default-features
cargo test --no-default-features --features async
cargo test --no-default-features --features rt-tokio
cargo test --all-features

# Build examples
cargo build --examples --all-features

# Generate documentation
cargo doc --no-deps --all-features
```

### Feature Flags

The library supports multiple feature configurations:

- **No default features**: Blocking-only API
- **`async`**: Runtime-agnostic async support (requires executor)
- **`rt-tokio`**: Tokio runtime integration (includes async)
- **`rt-async-std`**: async-std runtime integration (includes async)
- **`rt-smol`**: smol runtime integration (includes async)
- **`serial`**: Serial port support for RS-232/RS-422 connections (partial implementation)
- **`test-utils`**: Testing utilities and deterministic executor (not for production)

Always test your changes with different feature combinations.

## VISCA Protocol Safety Best Practices

### 1. Always Use VISCA_TERMINATOR Constant

✅ **Do:**
```rust
use crate::command::const_encoding::VISCA_TERMINATOR;
buffer.push_mut(VISCA_TERMINATOR);
```

❌ **Don't:**
```rust
buffer.push_mut(0xFF); // Never hardcode the terminator
```

### 2. Use ConstCommandBuilder for Command Construction

All VISCA commands should use the `ConstCommandBuilder` to ensure proper protocol handling:

✅ **Do:**
```rust
use crate::command::const_encoding::builder::ConstCommandBuilder;

let mut builder = ConstCommandBuilder::<8>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
builder.push_mut(speed);
builder.push_mut(value);
let terminated = builder.terminate(); // Moves to terminated state
let bytes = terminated.as_bytes();    // Ready to send
```

❌ **Don't:**
```rust
let mut buffer = [0x81, 0x01, 0x04, 0x47, speed, value, 0xFF, 0];
// Manual buffer construction is error-prone
```

### 3. Use Internal Macros for Command Implementation

When implementing new commands, use the provided macros:

```rust
// For simple commands
visca_command! {
    ZoomStop = [0x81, 0x01, 0x04, 0x07, 0x00]
}

// For boolean on/off commands
visca_bool_command! {
    BacklightCompensation = [0x81, 0x01, 0x04, 0x33]
}

// For parameterized commands
visca_param_command! {
    ZoomSpeed(speed: u8) = [0x81, 0x01, 0x04, 0x07] => |builder, speed| {
        builder.push_mut(0x20 | (speed & 0x07));
    }
}

// For commands with complex logic
visca_builder! {
    PanTiltAbsolute(pan: u16, tilt: u16) = [0x81, 0x01, 0x06, 0x02] => {
        builder.push_nibbles_mut((pan >> 12) as u8, (pan >> 8) as u8);
        builder.push_nibbles_mut((pan >> 4) as u8, pan as u8);
        builder.push_nibbles_mut((tilt >> 12) as u8, (tilt >> 8) as u8);
        builder.push_nibbles_mut((tilt >> 4) as u8, tilt as u8);
    }
}
```

### 4. Validate Commands in Debug Mode

Use debug assertions to catch protocol errors during development:

```rust
fn send_command(&self, cmd: &[u8]) -> Result<()> {
    debug_assert!(
        cmd.ends_with(&[VISCA_TERMINATOR]),
        "Command missing VISCA terminator"
    );
    // Send implementation
}
```

### 5. Use Type-Safe Parameters

Define bounded parameter types for compile-time validation:

```rust
visca_bounded_param! {
    /// Zoom speed from 0 (slow) to 7 (fast)
    ZoomSpeed: u8 {
        min: 0,
        max: 7
    }
}
```

### 6. Document Protocol Details

When implementing commands, include:
- VISCA command byte sequence
- Expected response types
- Camera compatibility notes

```rust
/// Zoom to a specific position
/// 
/// VISCA Command: `81 01 04 47 0p 0p 0p 0p FF`
/// where pppp = zoom position (0x0000 to 0x4000)
/// 
/// Response: ACK followed by Completion
/// 
/// Supported: PTZOptics G2, Sony EVI series
pub struct ZoomDirect {
    position: u16,
}
```

## Architecture Guidelines

### Module Organization

- `command/` - Low-level VISCA command implementations
  - Each command module implements `EncodeVisca` trait
  - Commands use internal macros for consistent encoding
  - Response parsing handled by `ResponseParser`
- `camera/` - High-level camera API
  - `methods/` - Trait-based camera control methods organized by function
    - Separate blocking and async traits (e.g., `ZoomControl` and `ZoomControlBlocking`)
  - `profiles/` - Camera model profiles (PtzOpticsG2, SonyFR7, etc.)
  - `builder.rs` - CameraBuilder for constructing cameras
- `transport/` - Network transport implementations
  - `blocking/` - BlockingTcp, BlockingUdp
  - `tokio/` - TokioTcp, TokioUdp (requires rt-tokio feature)
  - Mock transport for testing
  - Scripted transport for deterministic testing
- `runtime/` - Protocol-compliant command execution
  - Handles ACK/Completion sequences
  - Manages retries and timeouts
  - Priority-based command scheduling
- `testing/` - Test utilities and deterministic executor
- `executor.rs` - Runtime executor abstraction
- `grafton-visca-macros/` - Code generation macros (separate crate)

### Adding New Commands

1. **Define the command struct** in appropriate `command/*.rs` file:
   ```rust
   pub struct MyCommand {
       parameter: u8,
   }
   ```

2. **Implement encoding** using macros:
   ```rust
   visca_param_command! {
       MyCommand(param: u8) = [0x81, 0x01, 0x04, 0x50] => |builder, param| {
           builder.push_mut(param);
       }
   }
   ```

3. **Add high-level method** in `camera/methods/*.rs`:
   ```rust
   // For async API
   pub trait MyFeatureControl {
       async fn my_operation(&self, param: u8) -> Result<()>;
   }
   
   // For blocking API
   pub trait MyFeatureControlBlocking {
       fn my_operation(&self, param: u8) -> Result<()>;
   }
   ```

4. **Update capability detection** if needed in `camera/capabilities.rs`

5. **Add comprehensive tests**:
   - Command encoding tests
   - Camera method tests
   - Integration tests with mock transport

6. **Update documentation** with examples and camera compatibility

### Error Handling

- Use `Result<T, Error>` for all fallible operations
- Errors are categorized: Network, Protocol, Timeout, InvalidParameter
- Provide context in error messages
- Use `thiserror` for error definitions
- Transport errors are properly mapped and propagated

## Testing Guidelines

### Unit Tests

Place unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::test_utils::visca_test;

    #[test]
    fn test_command_encoding() {
        let cmd = ZoomIn;
        let encoded = cmd.encode_visca().unwrap();
        assert_eq!(encoded.bytes, vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]);
    }

    // Use visca_test! macro for command verification
    visca_test!(test_zoom_stop, ZoomStop, [0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]);
}
```

### Integration Tests

Place integration tests in `tests/` directory. Use feature flags appropriately:

```rust
// tests/camera_command_tests.rs
#[cfg(feature = "test-utils")]
#[test]
fn test_zoom_with_scripted_transport() {
    use grafton_visca::testing::testkit::{ScriptedBlockingTransport, ScriptEntry};
    use grafton_visca::{Camera, camera::BlockingMode};
    
    let script = vec![
        ScriptEntry::exchange(
            &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF],
            &[0x90, 0x41, 0xFF, 0x90, 0x51, 0xFF], // ACK + Completion
        ),
    ];
    
    let transport = ScriptedBlockingTransport::new(script);
    let camera = Camera::<BlockingMode, _, _, _>::new(transport);
    // Test camera operations
}
```

### Test Utilities

For deterministic testing, use the test-utils feature:

```rust
#[cfg(feature = "test-utils")]
use grafton_visca::testing::testkit::{ScriptedBlockingTransport, ScriptEntry, DeterministicExecutor};

#[test]
fn test_protocol_compliance() {
    let script = vec![
        ScriptEntry::exchange(
            &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF], 
            &[0x90, 0x41, 0xFF]
        ),
        ScriptEntry::delay(100),
        ScriptEntry::send(&[0x90, 0x51, 0xFF]),
    ];
    
    let transport = ScriptedBlockingTransport::new(script);
    // Test implementation
}
```

## Pull Request Process

1. **Before submitting:**
   ```bash
   # Format code
   cargo fmt --all
   
   # Run clippy for all feature combinations
   cargo clippy --all-targets --no-default-features -- -D warnings
   cargo clippy --all-targets --no-default-features --features async -- -D warnings
   cargo clippy --all-targets --no-default-features --features rt-tokio -- -D warnings
   cargo clippy --all-targets --all-features -- -D warnings
   
   # Test all feature combinations
   cargo test --no-default-features
   cargo test --no-default-features --features async
   cargo test --no-default-features --features rt-tokio
   cargo test --all-features
   
   # Check documentation
   cargo doc --no-deps --all-features
   ```

2. **PR Description should include:**
   - Clear description of changes
   - Issue number if applicable (fixes #123)
   - Breaking changes clearly marked
   - Testing performed with different feature flags
   - Camera models tested (if applicable)

3. **Review process:**
   - All PRs require review before merging
   - CI must pass all checks
   - Address review feedback promptly
   - Maintainers may request additional tests or documentation

## Breaking Changes

- Clearly mark breaking changes in PR description with **BREAKING:** prefix
- Update CHANGELOG.md with migration guide
- Consider providing compatibility shims or migration tools
- Update all affected examples
- Document the migration path in the PR

## Documentation

### Required Documentation

- Add doc comments for all public APIs
- Include at least one example in doc comments
- Document panic conditions with `# Panics` section
- Document error conditions with `# Errors` section
- Add `# Examples` section for non-trivial APIs

### Documentation Standards

```rust
/// Performs a zoom operation on the camera.
///
/// This method sends a zoom command to the camera and waits for completion.
///
/// # Arguments
///
/// * `speed` - Zoom speed from 0 (slowest) to 7 (fastest)
/// * `direction` - Zoom direction (In or Out)
///
/// # Errors
///
/// Returns an error if:
/// - The camera is not connected
/// - The zoom command is not supported
/// - A timeout occurs waiting for completion
///
/// # Examples
///
/// ```no_run
/// # use grafton_visca::{Camera, ZoomSpeed};
/// # async fn example(camera: Camera) -> Result<(), Box<dyn std::error::Error>> {
/// camera.zoom_variable(ZoomSpeed::new(5)?, ZoomDirection::In).await?;
/// # Ok(())
/// # }
/// ```
pub async fn zoom_variable(&self, speed: ZoomSpeed, direction: ZoomDirection) -> Result<()> {
    // Implementation
}
```

## Commit Message Guidelines

Follow conventional commits format:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style changes (formatting, etc.)
- `refactor:` Code refactoring
- `test:` Test additions or changes
- `chore:` Maintenance tasks
- `perf:` Performance improvements

Examples:
```
feat: add support for PTZOptics G3 cameras
fix: handle timeout correctly in zoom operations
docs: update examples for async API
test: add integration tests for preset commands
```

## Performance Considerations

- Avoid unnecessary allocations in hot paths
- Use `const` functions where possible
- Prefer stack allocation for small buffers
- Profile performance-critical code
- Document performance characteristics in comments

## Security

- Never log sensitive information
- Validate all input parameters
- Use bounded types for protocol parameters
- Handle untrusted network data safely
- Report security issues privately to maintainers

## Questions?

- Open an issue for bugs or feature requests
- Use GitHub Discussions for general questions
- Tag maintainers for urgent issues
- Check existing issues before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the Apache 2.0 License.