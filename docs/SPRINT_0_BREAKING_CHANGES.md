# Sprint 0: API Changes and Breaking Changes Analysis

## Overview
This document identifies potential API changes and breaking changes that will be introduced during the enhancement of the `grafton-visca` library for production readiness.

## Breaking Changes Expected

### 1. **Async API Introduction (Sprint 3)**
- **Change**: Introduction of `AsyncViscaClient` with async/await support
- **Impact**: While we'll maintain backward compatibility with sync wrappers, users wanting async functionality will need to migrate
- **Migration Path**: 
  - Existing `ViscaTransport` trait and `send_command_and_wait` will continue to work
  - New async users should use `AsyncViscaClient::connect()` and `.await` on commands
  - Feature flag: `async` feature will be optional to avoid forcing tokio dependency

### 2. **Response Type Handling (Sprint 2)**
- **Change**: `ViscaCommand::response_type()` will be repurposed for inquiry-only responses
- **Impact**: Custom command implementations may need adjustment
- **Migration Path**: 
  - Most users won't be affected as they use provided commands
  - Custom command implementers will need to review their `response_type()` implementations
  - ACK/Completion handling will be automatic and internal

### 3. **Error Type Expansion (Sprint 2)**
- **Change**: New `ViscaError` variants for specific camera errors
- **Impact**: Match statements on `ViscaError` will need updating if not using wildcard
- **Migration Path**:
  - Use `_` wildcard pattern in error matching to handle new variants
  - New variants: `CommandBufferFull`, `CommandNotExecutable`, `SyntaxError`, etc.

## Non-Breaking Additions

### 1. **New Commands (Sprint 1)**
All new commands will be additive:
- Iris control commands
- Shutter control commands  
- Gain control commands
- Brightness commands
- Color adjustment commands (Saturation, Hue, Red/Blue Gain)
- Advanced PTZ commands (Absolute/Relative positioning)
- Additional inquiry commands

### 2. **Concurrency Features (Sprint 3)**
- Two-command queue management (internal, transparent to users)
- Thread-safe operations
- Optional timeout support for commands

### 3. **Quality Improvements**
- Better logging and debugging support
- Comprehensive test coverage
- CI/CD pipeline
- Documentation improvements

## Version Strategy

Given the scope of changes:
- Current version: 0.2.1
- After Sprint 0-1 (new commands): 0.3.0
- After Sprint 2-3 (async support): 0.4.0 or 1.0.0-rc
- Production ready: 1.0.0

## Deprecation Policy

No immediate deprecations planned, but future considerations:
- `send_command_and_wait` may be marked deprecated in favor of async API (but will remain functional)
- Some internal parsing functions may become private

## Feature Flags

Planned feature flags to manage compatibility:
- `async`: Enable async API (requires tokio)
- `sync`: Enable synchronous wrappers (might be default initially)
- `full`: Enable all features

## Recommendations for Users

1. **For existing users**: No immediate action required. Your code will continue to work.
2. **For new projects**: Consider waiting for async API if you need concurrent camera control.
3. **For production use**: Wait for 1.0.0 release after all sprints complete.

## Testing Compatibility

All changes will be tested against:
- Existing example code (`examples/hello_visca.rs`)
- Current public API surface
- Documentation examples

This ensures backward compatibility throughout the enhancement process.