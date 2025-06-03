# API Breaking Changes Audit for grafton-visca

This document identifies potential breaking changes in the public API during the enhancement project.

## Current Public API Inventory

### Core Traits
- `ViscaTransport` - Main transport abstraction
- `ViscaCommand` - Command encoding interface

### Transport Implementations
- `UdpTransport` - UDP transport implementation
- `TcpTransport` - TCP transport implementation

### Main Functions
- `send_command_and_wait()` - Synchronous command execution

### Command Types
- `PowerCommand`
- `PanTiltCommand` 
- `ZoomCommand`
- `FocusCommand`
- `PresetCommand`
- `ExposureCommand`
- `WhiteBalanceCommand`
- `ImageCommand`
- `FlipCommand`
- `LuminanceContrastSharpnessCommand`
- `InquiryCommand`

### Response Types
- `ViscaResponse` enum
- `ViscaResponseType` enum
- `ViscaInquiryResponse` enum

### Error Types
- `ViscaError` enum
- `AppError` struct

### Value Types
- `PanSpeed`, `TiltSpeed` structs
- Various mode enums (ExposureMode, WhiteBalanceMode, etc.)

## Potential Breaking Changes

### 1. Async Trait Methods (Sprint 3)
**Impact**: HIGH  
**Change**: Making `ViscaTransport` methods async
```rust
// Current
fn send_command(&mut self, command: &[u8]) -> Result<(), ViscaError>
// Potential change
async fn send_command(&mut self, command: &[u8]) -> Result<(), ViscaError>
```
**Mitigation**: Feature-gate async support, maintain sync API

### 2. Response Type Modifications (Sprint 2)
**Impact**: MEDIUM  
**Change**: Extending `ViscaResponse` to include ACK information
```rust
// Current
pub enum ViscaResponse {
    Completion,
    InquiryResponse(ViscaInquiryResponse),
    Error(ViscaError),
}
// Potential addition
pub enum ViscaResponse {
    Acknowledgment { socket: u8 },  // NEW
    Completion,
    InquiryResponse(ViscaInquiryResponse),
    Error(ViscaError),
}
```
**Mitigation**: Use `#[non_exhaustive]` attribute

### 3. Error Type Extensions (Multiple Sprints)
**Impact**: LOW-MEDIUM  
**Change**: Adding new error variants
```rust
// New error variants needed
CommandBufferFull,
NotExecutable,
InvalidSocket,
Timeout,
```
**Mitigation**: Already using `#[non_exhaustive]` on ViscaError

### 4. Command Trait Changes (Sprint 2)
**Impact**: LOW  
**Change**: Modifying `response_type()` semantics
**Mitigation**: Keep existing behavior, add new methods if needed

## Non-Breaking Enhancements

### 1. New Command Types (Sprint 1)
- `IrisCommand`
- `ShutterCommand`
- `GainCommand`
- `BrightnessCommand`
- `ColorCommand`
- Additional inquiry commands

### 2. New Functions (Sprint 3)
- `AsyncViscaClient` (new type, not breaking)
- Async versions of existing functions

### 3. New Transport Implementations
- Async transport variants

### 4. Additional Methods
- Builder methods for complex types
- Convenience functions

## Recommended Approach

### Feature Flags Strategy
```toml
[features]
default = ["sync"]
sync = []
async = ["tokio", "futures"]
full = ["sync", "async"]
```

### Version Strategy
- Current: 0.2.1
- After Sprint 1: 0.3.0 (new features, no breaking changes)
- After Sprint 2: 0.4.0 (improved internals, minimal breaking changes)
- After Sprint 3: 0.5.0 (async support, feature-gated)
- After Sprint 4: 1.0.0-rc1 (release candidate)

### Migration Path
1. **0.2.x → 0.3.x**: No breaking changes, just new commands
2. **0.3.x → 0.4.x**: Minor updates to response handling
3. **0.4.x → 0.5.x**: Async opt-in via feature flag
4. **0.5.x → 1.0.x**: API stabilization

## Compatibility Guarantees

### What We Preserve
- All existing command types remain unchanged
- Sync API remains available (even with async feature)
- Existing error variants keep same meaning
- Transport trait for sync usage unchanged

### What May Change
- Internal implementation details
- Default timeout values (with ability to override)
- Performance characteristics (should improve)

## Testing Strategy
- Maintain compatibility tests for each version
- Test with and without feature flags
- Ensure examples continue to work
- Document all breaking changes in CHANGELOG

## Conclusion
Most enhancements can be implemented without breaking changes. The main exception is async support, which we'll handle via feature flags. The pre-1.0 status gives us flexibility, but we'll still minimize disruption for existing users.