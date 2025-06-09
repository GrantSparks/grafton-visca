# Grafton-VISCA API Audit Report - Phase 1

This document provides a comprehensive audit of the current API structure, identifying naming issues, overlapping APIs, and areas for improvement as part of implementing issue #36.

## Executive Summary

The grafton-visca library currently suffers from API sprawl with multiple overlapping interfaces, inconsistent naming patterns, and exposed development artifacts. This audit identifies specific issues that need to be addressed to create a clean, ergonomic API for v1.0.

## 1. Overlapping APIs

### 1.1 Power Control APIs

The library currently has **FOUR** different ways to control camera power:

1. **`ViscaPowerExt` trait** (src/power_ext.rs)
   - Methods: `power_on()`, `power_off()`, `power_status()`
   
2. **`PowerExt` trait** (src/ext/unified.rs)
   - Methods: `power_on()`, `power_off()`, `is_powered_on()`

3. **`UnifiedPowerExt` trait** (src/ext/unified_power.rs and unified_power_v2.rs)
   - Two versions of the same trait!
   - Methods vary between versions

4. **`ViscaTransportExt` trait** (src/transport_ext.rs)
   - Methods: `send_power_on()`, `send_power_off()`, `query_power_status()`

### 1.2 Zoom Control APIs

Three different interfaces for zoom control:

1. **`ViscaZoomExt` trait** (src/zoom_ext.rs)
   - Methods: `zoom_in()`, `zoom_out()`, `zoom_stop()`, `zoom_direct()`, etc.

2. **`ZoomExt` trait** (src/ext/unified.rs)
   - Methods: Similar but with slightly different signatures

3. **`CameraControl` trait** (src/api.rs)
   - Methods: High-level zoom control through builders

### 1.3 Preset Management APIs

Three overlapping preset interfaces:

1. **`ViscaPresetExt` trait** (src/preset_ext.rs)
   - Methods: `store_preset()`, `goto_preset()`, `clear_preset()`

2. **`PresetExt` trait** (src/ext/unified.rs)
   - Methods: `save_preset()`, `recall_preset()`, `clear_preset()`
   - Note: Different naming (store vs save, goto vs recall)

3. **Referenced but missing**: `UnifiedPresetExt`

## 2. Naming Inconsistencies

### 2.1 Type Name Prefixes

- **Inconsistent use of "Visca" prefix**:
  - Has prefix: `ViscaClient`, `ViscaError`, `ViscaCommand`, `ViscaResponse`
  - No prefix: `Client` (in unified_client.rs), `Transport`, `Speed`, `PresetNumber`
  - Should be consistent throughout

### 2.2 Extension Trait Naming

- **Three different patterns**:
  - `Visca*Ext`: ViscaPowerExt, ViscaZoomExt, etc.
  - `*Ext`: PowerExt, ZoomExt, CameraExt
  - `Unified*Ext`: UnifiedPowerExt
  - `Async*Ext`: AsyncViscaExt, AsyncCameraExt

### 2.3 Method Naming Patterns

- **Query methods inconsistent**:
  - `power_status()` vs `is_powered_on()` vs `get_power_state()`
  - `zoom_position()` vs `get_zoom_position()`
  - Some use `get_` prefix, others don't

- **Action methods inconsistent**:
  - `goto_preset()` vs `recall_preset()` (domain terminology issue)
  - `store_preset()` vs `save_preset()`

### 2.4 Module Naming

- **Async module prefixes**:
  - Files: `async_client.rs`, `async_transport.rs`, etc.
  - Since library is async-first, these prefixes are redundant

## 3. Development Artifacts in Public API

### 3.1 Exposed Internal Types

1. **`AppError`** - Application-level error that should be internal
2. **`TransportVariant`** - Internal enum exposed through Debug impls
3. **`BlockingTransport`** - Marked `#[doc(hidden)]` but still public

### 3.2 Incomplete Module Structure

The `src/ext/mod.rs` file references modules that don't exist:
```rust
pub mod unified_zoom;     // File not found
pub mod unified_preset;   // File not found  
pub mod unified_inquiry;  // File not found
```

### 3.3 Version Artifacts

- Two versions of unified_power: `unified_power.rs` and `unified_power_v2.rs`
- Suggests iterative development without cleanup

## 4. Domain Terminology Misalignment

### 4.1 VISCA Specification Terms

The library uses inconsistent terminology compared to VISCA spec:

- **Preset operations**:
  - Library: `goto_preset()`, `store_preset()`
  - VISCA spec: "Memory Recall", "Memory Set"
  - Better: `recall_preset()`, `set_preset()`

- **Power operations**:
  - Inconsistent between "power on/off" vs "powered on" state

### 4.2 Camera Industry Terms

- Using `goto` instead of industry-standard `recall` for presets
- Missing alignment with PTZ camera conventions

## 5. Module Organization Issues

### 5.1 Scattered Extension Traits

Extension traits are split across multiple locations:
- Root `src/` directory: Most extension traits
- `src/ext/` directory: "Unified" traits
- No clear organization principle

### 5.2 Prelude Incompleteness

The prelude (`src/prelude.rs`) is missing commonly used types:
- Missing `CameraControl` trait
- Missing unified extension traits
- Includes some command types but not others

### 5.3 Re-export Confusion

- `command/mod.rs` uses wildcard re-exports
- Makes it unclear what's actually part of public API
- No documentation on what should be imported vs used through prelude

## 6. Recommendations for Phase 1

### 6.1 Immediate Actions (This PR)

1. **Document all issues** ✓ (this report)
2. **Create deprecation plan** for duplicate APIs
3. **Identify canonical API patterns** to standardize on

### 6.2 API Consolidation Strategy

1. **Choose primary trait pattern**: Recommend `Visca*Ext` for all extension traits
2. **Standardize method naming**:
   - Queries: Always use `get_*()` or `*_state()`
   - Actions: Verb without prefix
3. **Pick canonical implementations**:
   - Keep `ViscaClient` as primary client
   - Keep category-specific `Visca*Ext` traits
   - Remove duplicate unified traits

### 6.3 Module Reorganization Plan

```rust
grafton_visca/
├── src/
│   ├── client.rs          // ViscaClient (renamed from ViscaClient)
│   ├── error.rs           // Error types
│   ├── transport/         // Transport implementations
│   ├── command/           // Command definitions
│   ├── ext/               // ALL extension traits
│   │   ├── mod.rs
│   │   ├── power.rs       // ViscaPowerExt
│   │   ├── zoom.rs        // ViscaZoomExt
│   │   └── ...
│   └── prelude.rs         // Comprehensive prelude
```

## 7. Breaking Change Impact

These changes will be breaking, but issue #36 explicitly allows this for v0.5.0:

- **High impact**: Removing duplicate traits will break existing code
- **Medium impact**: Renaming methods for consistency
- **Low impact**: Module reorganization (if re-exports maintained)

## 8. Next Steps

1. Review and approve this audit
2. Create detailed migration guide
3. Begin implementation with deprecation warnings
4. Update all examples and tests
5. Document new API structure

---

*Generated for Issue #36: API Naming Improvements*
*Date: January 9, 2025*