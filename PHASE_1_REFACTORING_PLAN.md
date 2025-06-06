# Phase 1 Refactoring Plan: Complete Transport Layer Consolidation

## Overview

This document outlines the complete refactoring plan for Phase 1 of issue #30, which aims to completely remove all legacy transport code and consolidate everything into a single, unified ViscaClient.

## Current State

The codebase currently has:
- **Old transport system**: `ViscaTransport` trait with `UdpTransport` and `TcpTransport` implementations in lib.rs
- **Async transport system**: `AsyncViscaTransport` trait with async implementations
- **New transport system**: `Transport` trait in transport/mod.rs with new implementations
- **Three client types**: `Client` (old), `AsyncViscaClient` (old), and `ViscaClient` (new unified)
- **11 extension traits** with ~124 methods total
- **Migration adapters** to bridge old and new systems

## Proposed Changes

### 1. Delete All Legacy Code

Remove from the codebase entirely:
- `ViscaTransport` trait (lib.rs:417-440)
- `UdpTransport` struct and impl (lib.rs:456-773)
- `TcpTransport` struct and impl (lib.rs:522-797)
- `send_command_and_wait` function (lib.rs:830-932)
- `AsyncViscaTransport` trait (async_transport.rs)
- Old `Client` struct (client.rs - entire file)
- `AsyncViscaClient` struct (async_client.rs - entire file)
- All async transport implementations that use old trait
- Migration adapters (transport/adapters.rs)

### 2. Consolidate All Methods into ViscaClient

Move all ~124 methods from extension traits directly into `ViscaClient`:

```rust
impl ViscaClient {
    // From ViscaTransportExt (20 methods)
    pub fn power_on(&self) -> Result<ViscaResponse, ViscaError> { ... }
    pub fn power_off(&self) -> Result<ViscaResponse, ViscaError> { ... }
    pub fn home(&self) -> Result<ViscaResponse, ViscaError> { ... }
    // ... etc

    // From ViscaInquiryExt (33 methods)
    pub fn get_power_state(&self) -> Result<PowerState, ViscaError> { ... }
    pub fn get_pan_tilt_position(&self) -> Result<(i32, i32), ViscaError> { ... }
    // ... etc

    // From ViscaExposureExt (24 methods)
    pub fn set_exposure_mode(&self, mode: ExposureMode) -> Result<ViscaResponse, ViscaError> { ... }
    // ... etc

    // ... and all other extension trait methods
}
```

Each method will have both blocking and async versions:
- Blocking: `method_name()` - uses the existing `send()` method
- Async: `method_name_async()` - uses the existing `send_async()` method

### 3. Delete Extension Trait Files

Remove entirely:
- src/transport_ext.rs
- src/inquiry_ext.rs
- src/exposure_ext.rs
- src/focus_ext.rs
- src/image_ext.rs
- src/pan_tilt_ext.rs
- src/power_ext.rs
- src/preset_ext.rs
- src/white_balance_ext.rs
- src/zoom_ext.rs
- src/position_ext.rs
- src/async_control_ext.rs
- src/async_inquiry_ext.rs

### 4. Update Advanced Features

#### Connection Pool
- Change from using `ViscaTransport` to using the new `Transport` trait
- Update to work with `ViscaClient` instances instead of raw transports

#### Reconnecting Transport
- Change from wrapping `ViscaTransport` to wrapping `Transport`
- Integrate with `ViscaClient`

### 5. Update All Examples

All examples will change from:
```rust
use grafton_visca::{UdpTransport, ViscaTransportExt, send_command_and_wait};
let mut transport = UdpTransport::new("192.168.1.100:5678")?;
transport.power_on()?;
```

To:
```rust
use grafton_visca::ViscaClient;
let client = ViscaClient::connect_udp("192.168.1.100:5678")?;
client.power_on()?;
```

### 6. Clean Up lib.rs

The lib.rs file will be dramatically simplified:
- Remove ~500 lines of old transport code
- Remove deprecated exports
- Export only: `ViscaClient`, command types, error types, and constants

## Benefits

1. **Single API Surface**: No more confusion about which client or trait to use
2. **No Imports Needed**: All methods directly on ViscaClient
3. **Cleaner Codebase**: Removes thousands of lines of deprecated/duplicate code
4. **Better Discoverability**: All methods visible on one type
5. **Simplified Maintenance**: One implementation instead of three

## Drawbacks

1. **Massive ViscaClient**: Will have 124+ methods (arguably too many)
2. **Breaking Change**: All existing code must be rewritten
3. **No Gradual Migration**: Users must update everything at once
4. **Loss of Modularity**: Can't import just the traits you need

## Implementation Order

1. Start by creating the complete ViscaClient with all methods
2. Update tests to use new API
3. Delete old code
4. Update examples
5. Update documentation

## Questions

1. Is a 124+ method client acceptable, or should we reconsider the approach?
2. Should we keep some level of trait organization (e.g., impl blocks for different categories)?
3. How do we handle the async variants - suffix with `_async` or use a different pattern?

## Alternative Approach

Instead of moving all methods to ViscaClient, we could:
1. Keep the extension traits but have ViscaClient implement them directly
2. Users would still import traits, but use them on ViscaClient
3. This maintains organization while eliminating the old transport system

```rust
impl ViscaTransportExt for ViscaClient { ... }
impl ViscaInquiryExt for ViscaClient { ... }
// etc
```

This would be less disruptive while still achieving the consolidation goal.