# Contributing to grafton-visca

Thank you for your interest in contributing to grafton-visca! This guide will help you get started with development and explain how to submit your contributions.

## Development Setup

### Prerequisites

- Rust 1.70 or higher
- A VISCA-compatible PTZ camera for testing (optional but recommended)
- Git for version control

### Getting Started

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/grafton-visca.git
   cd grafton-visca
   ```
3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/grafton-ai/grafton-visca.git
   ```
4. Create a new branch for your feature:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Building the Project

```bash
# Build the library
cargo build

# Build with all features (including async)
cargo build --all-features

# Run tests
cargo test

# Run clippy for linting
cargo clippy -- -D warnings

# Format code
cargo fmt
```

## Testing with Real Hardware

### Running the Example

The library includes a comprehensive example that demonstrates various camera controls:

```bash
cargo run --example hello_visca -- <protocol> <camera_ip>

# Examples:
cargo run --example hello_visca -- udp 192.168.1.100:1259
cargo run --example hello_visca -- tcp 192.168.1.100:5678
```

The example will:
1. Connect to your camera
2. Query current positions and settings
3. Demonstrate various movement commands
4. Show how to use inquiry commands

### Testing Guidelines

- Test your changes with real camera hardware when possible
- Ensure all existing tests pass
- Add tests for new functionality
- Document any camera-specific behaviors you discover

## Adding New Features

### Supporting New Camera Models

If you want to add support for a new VISCA-compatible camera:

1. **Document Differences**: Create an issue describing any protocol differences or unique commands
2. **Test Compatibility**: Verify which existing commands work with the new camera
3. **Add Camera-Specific Features**: If the camera has unique commands, add them in appropriate modules
4. **Update Documentation**: Note the new camera support in the README and relevant module docs

### Adding New VISCA Commands

To add a new VISCA command:

1. **Choose the Right Module**: Commands are organized by functionality:
   - `pan_tilt.rs` - Pan/tilt movement
   - `zoom.rs` - Zoom control
   - `focus.rs` - Focus control
   - `exposure.rs` - Exposure settings
   - etc.

2. **Implement the Command**: Add your command to the appropriate enum:
   ```rust
   #[derive(Debug)]
   pub enum YourCommand {
       NewFeature { parameter: u8 },
   }
   ```

3. **Implement ViscaCommand Trait**:
   ```rust
   impl ViscaCommand for YourCommand {
       fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
           match self {
               YourCommand::NewFeature { parameter } => {
                   Ok(vec![0x81, 0x01, 0x04, 0xXX, *parameter, 0xFF])
               }
           }
       }
       
       fn response_type(&self) -> Option<ViscaResponseType> {
           // Return None for action commands
           // Return Some(...) for inquiry commands
           None
       }
   }
   ```

4. **Add Tests**: Include unit tests for your command
5. **Update Documentation**: Add rustdoc comments explaining the command

### Adding Inquiry Commands

For inquiry commands that return data:

1. Add the inquiry variant to `InquiryCommand` enum
2. Add the response variant to `ViscaInquiryResponse` enum
3. Implement the response parser in `response.rs`
4. Add the appropriate `ViscaResponseType` variant

## Code Style

### Rust Guidelines

- Follow standard Rust naming conventions
- Use `rustfmt` for consistent formatting
- Add rustdoc comments for all public items
- Use descriptive variable names
- Prefer explicit error handling over panics

### Documentation

- Add module-level documentation for new modules
- Include examples in rustdoc comments
- Document any non-obvious byte sequences or protocol details
- Update the README if adding major features

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in present tense (e.g., "Add", "Fix", "Update")
- Keep the first line under 72 characters
- Add detailed explanation after a blank line if needed

Example:
```
Add white balance temperature control

Implements direct color temperature setting for cameras that support
it. Temperature range is 2500K-8000K, represented as byte values
according to the VISCA specification.
```

## Submitting Pull Requests

1. **Update Your Branch**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run All Checks**:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   cargo build --all-features
   ```

3. **Push Your Changes**:
   ```bash
   git push origin feature/your-feature-name
   ```

4. **Create Pull Request**:
   - Go to GitHub and create a PR from your branch
   - Fill out the PR template (if available)
   - Describe what your changes do and why
   - Reference any related issues

### PR Guidelines

- Keep PRs focused on a single feature or fix
- Ensure all CI checks pass
- Respond to code review feedback promptly
- Update documentation as needed
- Add tests for new functionality

## Getting Help

If you have questions or need help:

- Open an issue for bugs or feature requests
- Check existing issues and PRs for similar work
- Reach out to the maintainers through GitHub issues

## License

By contributing to grafton-visca, you agree that your contributions will be licensed under the Apache License 2.0.