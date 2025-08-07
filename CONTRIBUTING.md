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

# Build the project
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets --all-features
```

## Code Standards

### Formatting

Always run `cargo fmt` before committing:

```bash
cargo fmt --all
```

### Linting

Ensure your code passes clippy without warnings:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing

- Write tests for new functionality
- Ensure all tests pass: `cargo test --all-features`
- Add doc tests for public APIs

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

### 2. Use CommandBuilder for Command Construction

All VISCA commands should use the `CommandBuilder` to ensure proper protocol handling:

✅ **Do:**
```rust
let mut builder = CommandBuilder::<8>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
builder.push_mut(speed);
builder.push_mut(value);
let bytes = builder.finalize(); // Automatically adds terminator
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
- `camera/` - High-level camera API
- `transport/` - Network transport implementations
- `macros/` - Code generation macros

### Adding New Commands

1. Add command implementation in appropriate `command/*.rs` file
2. Add high-level method in `camera/methods/*.rs`
3. Update capability detection if needed
4. Add tests for both command encoding and camera method
5. Update documentation

### Error Handling

- Use `Result<T, Error>` for fallible operations
- Provide context in error messages
- Use `thiserror` for error definitions
- Map transport errors appropriately

## Testing Guidelines

### Unit Tests

Place unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_encoding() {
        let cmd = ZoomIn;
        let encoded = cmd.encode_visca().unwrap();
        assert_eq!(encoded.bytes, vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]);
    }
}
```

### Integration Tests

Place integration tests in `tests/` directory:

```rust
// tests/zoom_tests.rs
#[test]
fn test_zoom_with_mock_transport() {
    let transport = MockTransport::new();
    let camera = Camera::new(transport);
    // Test implementation
}
```

## Pull Request Process

1. **Before submitting:**
   - Run `cargo fmt --all`
   - Run `cargo clippy --all-targets --all-features`
   - Run `cargo test --all-features`
   - Update documentation if needed

2. **PR Description should include:**
   - What changes were made
   - Why the changes are needed
   - Any breaking changes
   - Testing performed

3. **Review process:**
   - All PRs require review before merging
   - CI must pass (tests, clippy, fmt)
   - Address review feedback promptly

## Breaking Changes

- Clearly mark breaking changes in PR description
- Update CHANGELOG.md
- Consider migration path for users
- Update examples if affected

## Documentation

- Add doc comments for all public APIs
- Include examples in doc comments
- Update README if adding major features
- Keep CLAUDE.md updated for AI assistance

## Questions?

- Open an issue for bugs or feature requests
- Join discussions in GitHub Discussions
- Contact maintainers for guidance

## License

By contributing, you agree that your contributions will be licensed under the Apache 2.0 License.