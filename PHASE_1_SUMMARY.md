# Phase 1: Transport Layer Consolidation - Clean Refactor Plan

## Goal: Clean Unified System with ALL Functionality

This is NOT about backward compatibility or migration paths. This is about creating a clean, unified system that has ALL the capabilities of the old system but none of the legacy code.

## Current State Analysis

### What Exists Now (To Be Removed)
- Old `ViscaTransport` trait in lib.rs
- Old `UdpTransport` and `TcpTransport` implementations in lib.rs  
- `LegacyTransportAdapter` and migration helpers (not needed)
- Separate extension traits that require `ViscaTransport`
- `send_command_and_wait` function

### What ViscaClient Currently Lacks
- Methods from extension traits (dozens of convenience methods)
- Timeout configuration per command category
- Connection statistics and health monitoring
- Direct support for some advanced patterns

## Proposed Clean Architecture

### 1. ViscaClient as the Single Interface
```rust
// All functionality in one place
pub struct ViscaClient {
    // Internal transport (new Transport trait)
    // Session management
    // Statistics
    // Timeout config
}

impl ViscaClient {
    // Core methods
    pub fn connect_tcp(addr: &str) -> Result<Self>
    pub fn connect_udp(addr: &str) -> Result<Self>
    pub fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse>
    
    // From ViscaInquiryExt (30+ methods)
    pub fn get_power_state(&self) -> Result<bool>
    pub fn get_camera_state(&self) -> Result<CameraState>
    pub fn get_pan_tilt_position(&self) -> Result<(i32, i32)>
    // ... etc
    
    // From ViscaTransportExt (20+ methods)  
    pub fn zoom_to_magnification(&self, magnification: f32) -> Result<()>
    pub fn pan_tilt_to_position(&self, pan: f32, tilt: f32) -> Result<()>
    // ... etc
    
    // From other extension traits
    pub fn set_exposure_mode(&self, mode: ExposureMode) -> Result<()>
    pub fn focus_to_position(&self, position: u16) -> Result<()>
    // ... etc
    
    // Configuration and monitoring
    pub fn set_timeout_config(&mut self, config: TimeoutConfig)
    pub fn stats(&self) -> &ConnectionStats
    pub fn is_healthy(&self) -> bool
}
```

### 2. Clean Internal Structure
- Use new `Transport` trait from `src/transport/mod.rs` internally
- Connection pooling works with `Transport` trait
- Reconnecting wrapper works with `Transport` trait
- No exposure of old traits or types

### 3. Complete Removal List
- [ ] Delete `ViscaTransport` trait
- [ ] Delete old `UdpTransport`/`TcpTransport` from lib.rs
- [ ] Delete `send_command_and_wait`
- [ ] Delete `LegacyTransportAdapter`
- [ ] Delete `from_legacy_transport` method
- [ ] Delete migration example
- [ ] Remove all extension traits as separate entities (move methods to ViscaClient)

## Implementation Steps

1. **Enhance ViscaClient** - Add all missing methods and features
2. **Update Dependencies** - Connection pool, reconnecting transport to use new Transport
3. **Delete Legacy Code** - Remove everything old
4. **Update Examples** - All 16 examples use ViscaClient
5. **Update Tests** - All tests use new API
6. **Clean Documentation** - Remove migration guides, update for clean API

## Result

A single, clean, fully-featured API with no legacy code or confusion about what to use.