# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

### Added
- **Improved Error Types**: Enhanced error handling with more specific error variants
  - Added `ConnectionFailed`, `ConnectionLost`, and `CommandTimeout` for network issues
  - Added `CameraBusy`, `CameraMoving`, and `CameraNotReady` for camera state errors
  - Added `InvalidResponse` and `CommandRejected` for protocol errors
  - Added `OutOfRange` and `PresetNotFound` for value validation
  - Added `FeatureNotSupported` for camera capability detection
  - Includes helper methods `is_retryable()` and `suggested_retry_delay()`

### Changed
- **Breaking Change**: Reorganized `ViscaError` enum with new variants
  - Kept traditional exhaustive enum approach for better ergonomics
  - Users can handle all error cases with compile-time guarantees
  - Accepted that new error variants require major version bumps

## [0.3.0] - 2025-01-06

### Added
- **Production-Ready Features**: The library is now production-ready with >90% test coverage
- **Complete Documentation**: All public APIs now have comprehensive documentation with examples
- **Unit Tests**: Added extensive unit tests for core modules including:
  - Error handling (ViscaError, AppError)
  - Session management (ViscaSession)
  - Command encoding (pan/tilt commands with validation)
- **Demo Application**: Added `demo.rs` example showcasing all library features
- **Documentation Improvements**:
  - Added trait-level documentation for `ViscaTransport` and `ViscaCommand`
  - Added comprehensive function documentation for `send_command_and_wait`
  - Added struct-level documentation for transport types
  - Added enum documentation for response types

### Changed
- **README Updates**: Updated to reflect production readiness, removed WIP warnings
- **Documentation Examples**: Fixed async example code to prevent lifetime issues

### Fixed
- **Test Compilation**: Fixed duplicate test modules and missing methods
- **Clippy Warnings**: Resolved redundant closure warnings in async modules
- **Example Code**: Fixed command usage in async_concurrent example

### Sprint 4 Completion
This release completes Sprint 4 of the production readiness roadmap:
- ✅ Comprehensive unit test coverage
- ✅ Complete Rustdoc documentation  
- ✅ All code quality checks passing (fmt, clippy)
- ✅ Demo application showcasing features
- ✅ README updated for production use

## [0.2.2] - Previous Release

### Sprint 3 - Async Support
- Added full async/await support with `AsyncViscaClient`
- Implemented concurrent command execution with automatic socket management
- Added background response handling with proper state machine
- Thread-safe design allowing client to be cloned and shared

### Sprint 2 - Protocol Improvements  
- Correct ACK/Completion response handling
- Proper socket management for VISCA's two-socket limitation
- Error response classification with specific error types

### Sprint 1 - Command Coverage
- Implemented all missing VISCA commands for PTZOptics G2
- Added exposure control commands (iris, shutter, gain, etc.)
- Added color adjustment commands (saturation, hue, white balance tuning)
- Added advanced PTZ commands (absolute/relative positioning)
- Added inquiry commands for all new features

## [0.1.0] - Initial Release

- Basic VISCA over IP implementation
- Core PTZ control commands
- UDP and TCP transport support