# Grafton-VISCA API Design - Phase 2

This document outlines the consolidated API design for grafton-visca v0.5.0, addressing all issues identified in the Phase 1 audit.

## Design Principles

1. **Single source of truth**: Each functionality has exactly one API
2. **Consistent naming**: All types follow predictable patterns
3. **Domain alignment**: Use VISCA specification terminology
4. **Ergonomic design**: Common operations should be easy
5. **No functionality loss**: All existing capabilities preserved

## Core Type Renaming

### Before → After

```rust
// Core types (remove Visca prefix for primary types)
ViscaClient      → Client
ViscaError       → Error  
ViscaSession     → Session
ViscaResponse    → Response

// Keep Visca prefix for protocol-specific types
ViscaCommand     → Command (trait renamed)
ViscaDevice      → ViscaDevice (unchanged - protocol trait)

// Remove development artifacts from public API
AppError         → (make private)
TransportVariant → (make private)
```

## Extension Trait Consolidation

### Power Control

Consolidate 4 APIs into 1:

```rust
// Keep only ViscaPowerExt, enhance with best methods from all versions
pub trait ViscaPowerExt {
    // From current ViscaPowerExt
    async fn power_on(&mut self) -> Result<(), Error>;
    async fn power_off(&mut self) -> Result<(), Error>;
    
    // Better naming from other traits
    async fn get_power_state(&mut self) -> Result<bool, Error>;  // was power_status()
    
    // Deprecate: PowerExt, UnifiedPowerExt, power methods in ViscaTransportExt
}
```

### Zoom Control

Consolidate 3 APIs into 1:

```rust
pub trait ViscaZoomExt {
    // Existing methods (keep all)
    async fn zoom_stop(&mut self) -> Result<(), Error>;
    async fn zoom_tele(&mut self) -> Result<(), Error>;  
    async fn zoom_wide(&mut self) -> Result<(), Error>;
    async fn zoom_tele_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;
    async fn zoom_wide_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;
    async fn zoom_direct(&mut self, position: u16) -> Result<(), Error>;
    
    // Add from other traits
    async fn get_zoom_position(&mut self) -> Result<u16, Error>;
    
    // Deprecate: ZoomExt, UnifiedZoomExt (not found)
}
```

### Preset Management

Fix terminology and consolidate:

```rust
pub trait ViscaPresetExt {
    // Align with VISCA spec terminology
    async fn set_preset(&mut self, preset: PresetNumber) -> Result<(), Error>;     // was store_preset
    async fn recall_preset(&mut self, preset: PresetNumber) -> Result<(), Error>;  // was goto_preset
    async fn clear_preset(&mut self, preset: PresetNumber) -> Result<(), Error>;   // unchanged
    
    // Deprecate: PresetExt, UnifiedPresetExt
}
```

## Module Reorganization

```
src/
├── lib.rs              // Main crate root
├── client.rs           // Client (was ViscaClient)
├── error.rs            // Error type
├── session.rs          // Session type
├── response.rs         // Response type
├── types.rs            // Shared types
├── prelude.rs          // Comprehensive prelude
│
├── transport/          // Transport layer (unchanged structure)
│   ├── mod.rs
│   ├── tcp.rs          // TcpTransport (was async_tcp_transport.rs)
│   ├── udp.rs          // UdpTransport (was async_udp_transport.rs)
│   └── traits.rs       // Transport traits
│
├── command/            // Command definitions (unchanged)
│   └── ...
│
└── ext/                // ALL extension traits
    ├── mod.rs          // Public re-exports
    ├── power.rs        // ViscaPowerExt
    ├── zoom.rs         // ViscaZoomExt
    ├── focus.rs        // ViscaFocusExt
    ├── pan_tilt.rs     // ViscaPanTiltExt
    ├── preset.rs       // ViscaPresetExt
    ├── exposure.rs     // ViscaExposureExt
    ├── image.rs        // ViscaImageExt
    ├── white_balance.rs // ViscaWhiteBalanceExt
    ├── inquiry.rs      // ViscaInquiryExt
    └── camera.rs       // High-level CameraControl trait
```

## Async Module Renaming

Remove redundant `async_` prefixes:

```
async_client.rs           → client.rs
async_tcp_transport.rs     → transport/tcp.rs
async_udp_transport.rs     → transport/udp.rs
async_transport.rs         → transport/traits.rs
async_connection_pool.rs   → connection_pool.rs
async_reconnecting_transport.rs → transport/reconnecting.rs
```

## Prelude Updates

```rust
// src/prelude.rs
pub use crate::{
    // Core types
    Client, Error, Session, Response,
    
    // All extension traits
    ext::{
        ViscaPowerExt, ViscaZoomExt, ViscaFocusExt,
        ViscaPanTiltExt, ViscaPresetExt, ViscaExposureExt,
        ViscaImageExt, ViscaWhiteBalanceExt, ViscaInquiryExt,
        CameraControl,
    },
    
    // Common types
    types::{
        PanSpeed, TiltSpeed, ZoomSpeed, FocusSpeed,
        PresetNumber, // ... other commonly used types
    },
    
    // Transport traits (for advanced users)
    transport::{Transport, BlockingTransport},
};
```

## Migration Strategy

### Phase 3 Implementation Plan

1. **Add deprecation warnings** to all old APIs pointing to new ones
2. **Create type aliases** for backward compatibility:
   ```rust
   #[deprecated(since = "0.5.0", note = "Use `Client` instead")]
   pub type ViscaClient = Client;
   ```
3. **Implement new traits** alongside old ones initially
4. **Update internal code** to use new APIs
5. **Ensure all tests pass** with both old and new APIs

### Phase 4 Documentation

1. **Migration guide** showing before/after for each change
2. **Update all examples** to use new API
3. **Update README** and crate documentation

### Phase 5 Cleanup

1. **Remove deprecated items** (for v1.0)
2. **Final API polish**
3. **Performance optimizations** if needed

## Backward Compatibility

During transition (v0.5.x):
- Old types available via type aliases with deprecation warnings
- Old trait methods forward to new implementations
- Examples show new API but old API still works

For v1.0:
- Remove all deprecated items
- Clean, consistent API surface

## Benefits

1. **Reduced confusion**: One way to do each thing
2. **Better discoverability**: Consistent naming patterns
3. **Smaller API surface**: Fewer traits to import
4. **Domain alignment**: Methods match VISCA terminology
5. **Future-proof**: Clean foundation for v1.0

---

*Phase 2 Design Document for Issue #36*
*Date: January 9, 2025*