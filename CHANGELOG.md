# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### MotionControl Trait Enhancement
- Added `stop_all_motion()` method to `MotionControl` trait for convenient single-call motion stopping
- Method calls `pan_tilt_stop()` internally with clear documentation
- Available for all camera types through the control trait

#### Diagnostics Trait Export
- Exported `Diagnostics` trait in prelude for easier access
- Users can now access `probe()`, `ping()`, and `measure_latency()` without explicit trait imports
- Improves discoverability of diagnostic functionality

#### Inquiry Conversions Enhancement
- Enhanced documentation and examples for `inquiry_conversions` module
- `PanTiltPositionRaw::as_degrees()` provides accurate degree conversion from raw VISCA values
- Proper handling of asymmetric pan/tilt ranges (Pan: -170° to +170°, Tilt: -30° to +90°)

### Changed

#### Dependency Updates
- Upgraded `schemars` dependency from 0.8 to 1.0 for JSON schema generation
- Ensures compatibility with latest ecosystem tools

### Impact on Downstream Projects

The serialization improvements in 0.8.0 combined with these enhancements enable significant boilerplate reduction in downstream projects:

- **Wrapper type elimination**: visca-mcp eliminated ~139 lines of wrapper types by using grafton-visca types directly
  - Removed `Normalized01` wrapper (~71 lines) - now uses `Normalized<f32>` directly
  - Removed `PresetId` wrapper (~68 lines) - now uses `PresetNumber` directly
- **Cleaner API**: No more manual serde implementations or bridge TryFrom implementations
- **Type safety**: Maintained compile-time validation while reducing code

Example migration:
```rust
// Before: Custom wrapper types
pub struct Normalized01(f32);
impl Serialize for Normalized01 { /* ... */ }
impl Deserialize for Normalized01 { /* ... */ }
// ~71 lines total

// After: Direct usage with serde feature
use grafton_visca::units::Normalized;
// 1 line, full serialization support included
```

## 0.8.0

This release completes a focus is on eliminating downstream boilerplate, providing first-class timeout and cancellation support, and unifying the API across all transport types.

### 🎯 Philosophy: Runtime-Agnostic Modernization

The central achievement is adding powerful ergonomic features that previously required custom downstream wrappers. The library now ships with feature-gated serialization, intuitive unit conversions, first-class timeout/cancellation support, diagnostic utilities, and uniform transport handling—all without forcing users into a specific async runtime.

### 🚀 Major Features & Improvements

#### Serialization & Schema Support
All public value types now support optional serialization through feature-gated `serde` and `schemars` derives:

```toml
[dependencies]
grafton-visca = { version = "0.8", features = ["serde", "schemars"] }
```

With these features enabled, you can serialize/deserialize all value types directly and generate JSON schemas for API documentation, eliminating the need for downstream wrapper types.

#### Ergonomic Type Conversions
- **From<f64> for numeric types**: `Degrees`, `Normalized`, and other numeric types accept `f64` directly, eliminating manual casts
- **Published MIN/MAX constants**: All range types expose validation bounds (e.g., `PanSpeed::MIN`, `PanSpeed::MAX`)
- **Validated constructors**: All parameter types provide `new()` methods with clear error messages including valid ranges
- **Model-aware validation**: New `new_for_model()` constructors validate against specific camera capabilities

```rust
// Old (0.7.1): Manual casting
camera.pan_tilt_absolute(Degrees(45.0 as f32), Degrees(15.0 as f32), SpeedLevel::Fast)?;

// New (0.8.0): Natural f64 usage
camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;

// Model-aware validation
let speed = PanSpeed::new_for_model(20, CameraVariant::PtzOpticsG2)?;
```

#### Inquiry Conversions
New `inquiry_conversions` module provides helpers for converting raw VISCA values to user-friendly formats:

```rust
use grafton_visca::{
    inquiry_conversions::{PanTiltPositionRaw, PanTiltPositionDeg, ZoomDomain, Normalized},
    ZoomPositionExt,
};

// Convert raw pan/tilt to degrees
let raw = PanTiltPositionRaw::new(1224, 648);
let deg = raw.as_degrees();  // ~85° pan, ~45° tilt

// Domain-aware zoom normalization
let zoom_pos = camera.inquiry().zoom_position().await?;
let optical_norm = zoom_pos.normalize(ZoomDomain::Optical);  // 0.0-1.0 for optical range
let full_norm = zoom_pos.normalize(ZoomDomain::OpticalPlusDigital);  // 0.0-1.0 for full range

// Create zoom position from normalized value
let zoom = zoom_from_normalized(Normalized(0.5), ZoomDomain::Optical)?;
camera.zoom_absolute(zoom)?;
```

Types added:
- `PanTiltPositionRaw` / `PanTiltPositionDeg` - Raw and degree-based position representations
- `ZoomDomain` - Enum for Optical vs OpticalPlusDigital normalization
- `Normalized` - Type-safe wrapper for 0.0-1.0 values
- `ZoomPositionExt` trait - Domain-aware normalization methods

#### Coarse Speed Mapping
Canonical mapping from user-friendly speed levels to device-specific values:

```rust
use grafton_visca::types::Coarse;

// Old (0.7.1): Custom mapping tables in application code
let zoom_speed = match user_speed {
    UserSpeed::Slow => ZoomSpeed::new(2)?,
    UserSpeed::Medium => ZoomSpeed::new(4)?,
    UserSpeed::Fast => ZoomSpeed::new(6)?,
    // ...
};

// New (0.8.0): Built-in canonical mapping
let zoom_speed = ZoomSpeed::from_coarse(Coarse::Fast);  // → 6
let pan_speed = PanSpeed::from_coarse(Coarse::Medium);  // → 12
let tilt_speed = TiltSpeed::from_coarse(Coarse::Slow);  // → 5
```

`Coarse` is a type alias for `SpeedLevel` with five intuitive levels: Slowest, Slow, Medium, Fast, Fastest.

#### Diagnostics & Health Checks
New `diagnostics` module provides tools for camera health monitoring:

```rust
use grafton_visca::diagnostics::Diagnostics;

// Probe camera for connectivity and latency
let report = camera.probe().await?;
if report.is_healthy() {
    println!("Camera responsive, RTT: {:?}", report.rtt);
}

// Simple ping check
if camera.ping().await? {
    println!("Camera is online");
}

// Measure average latency
let latency = camera.measure_latency(5).await?;
println!("Average RTT: {:?}", latency);
```

Types added:
- `ProbeReport` - Health check results with RTT and transport status
- `Diagnostics` trait - Methods for `probe()`, `ping()`, and `measure_latency()`

#### Command Cancellation
Built-in support for canceling in-flight commands without application-level wrappers:

```rust
use grafton_visca::ViscaSocket;

// Start a command and get its ID for later cancellation
let (command_id, future) = camera.start_command_with_id(&zoom_cmd).await?;

// Cancel by command ID
camera.cancel(command_id).await?;
// The future will resolve with Err(Error::CommandCanceled)

// Or cancel all commands on a specific socket
camera.cancel_socket(ViscaSocket::S1).await?;
```

This provides first-class cancellation support for long-running operations like preset recalls or movements, enabling responsive UIs and timeout handling without runtime-specific wrappers.

#### Connection Timeout Enforcement (#417)
Async connectors now properly enforce `connect_timeout` and use non-blocking DNS resolution:

```rust
use grafton_visca::transport::TransportConfig;
use std::time::Duration;

let config = TransportConfig::default()
    .with_connect_timeout(Duration::from_millis(500));

// Old (0.7.1): connect_timeout was ignored in async, blocking DNS
// New (0.8.0): Timeout enforced, async DNS used
let transport = Transport::tcp()
    .address("192.168.0.110:5678")
    .config(config)
    .connect().await?;  // Times out after 500ms if unreachable
```

Changes per runtime:
- **Tokio**: Uses `tokio::time::timeout()` and `tokio::net::lookup_host()`
- **async-std**: Uses `async_std::future::timeout()` and `async_std::net::ToSocketAddrs`
- **smol**: Uses `async_io::Timer` with `futures_lite::future::race()` and `smol::unblock()` for DNS

#### Uniform Transport Support (#415)
Serial transport now integrated into `TransportHandle` enum, enabling uniform trait implementations:

```rust
// Old (0.7.1): Serial used separate type, preventing uniform trait implementations
pub enum TransportHandle<R: Runtime> {
    Udp(UdpTransport<R>),
    Tcp(TcpTransport<R>),
}

// New (0.8.0): All transports unified
pub enum TransportHandle<R: Runtime> {
    Udp(UdpTransport<R>),
    Tcp(TcpTransport<R>),
    #[cfg(feature = "transport-serial-*")]
    Serial(<R as RuntimeSerial>::SerialTransport),
}
```

This enables downstream libraries to implement traits uniformly across all transport types without trait coherence conflicts.

#### Enhanced Error Types
Richer error information with retry hints:

```rust
match camera.send_command(cmd).await {
    Err(e) => {
        println!("Error kind: {:?}", e.kind());
        if e.is_retryable() {
            if let Some(delay) = e.suggested_retry_delay() {
                sleep(delay).await;
                // retry...
            }
        }
    }
    Ok(_) => {}
}
```

New `ErrorKind` variants and methods provide machine-actionable error classification for robust retry logic.

### 📝 API Changes & Migration Guide

#### Import Changes

Most user-facing APIs remain unchanged. The primary additions are new modules:

```rust
// New modules (0.8.0)
use grafton_visca::{
    inquiry_conversions::{Normalized, PanTiltPositionRaw, PanTiltPositionDeg, ZoomDomain},
    diagnostics::{Diagnostics, ProbeReport},
    types::Coarse,
    ViscaSocket,
};
```

#### Type Construction

```rust
// Old (0.7.1): Manual casting from f64
camera.pan_tilt_absolute(
    Degrees(45.0 as f32),
    Degrees(15.0 as f32),
    SpeedLevel::Fast
)?;

// New (0.8.0): Direct f64 usage
camera.pan_tilt_absolute(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
)?;

// New (0.8.0): Model-aware validation
let pan = PanPosition::new_for_model(2000, CameraVariant::PtzOpticsG2)?;
```

#### Speed Mapping

```rust
// Old (0.7.1): Custom mapping in application
fn map_to_zoom_speed(level: UISpeed) -> ZoomSpeed {
    match level {
        UISpeed::Slow => ZoomSpeed::new(2).unwrap(),
        UISpeed::Medium => ZoomSpeed::new(4).unwrap(),
        UISpeed::Fast => ZoomSpeed::new(6).unwrap(),
    }
}

// New (0.8.0): Use built-in Coarse mapping
let speed = ZoomSpeed::from_coarse(Coarse::Medium);  // → 4
```

#### Inquiry Result Handling

```rust
// Old (0.7.1): Manual conversion
let raw_pos = camera.inquiry().pan_tilt_position().await?;
let deg_pan = (raw_pos.pan as f32) * 170.0 / 2448.0;
let deg_tilt = /* complex asymmetric formula */;

// New (0.8.0): Built-in conversions
let raw_pos = camera.inquiry().pan_tilt_position().await?;
let deg_pos = raw_pos.as_degrees();
println!("Pan: {}°, Tilt: {}°", deg_pos.pan.0, deg_pos.tilt.0);

// Or with profile-specific adjustments
let deg_pos = raw_pos.as_degrees_with_profile(&profile);
```

#### Zoom Normalization

```rust
// Old (0.7.1): Manual normalization with hard-coded constants
let zoom_pos = camera.inquiry().zoom_position().await?;
let normalized_optical = (zoom_pos.value() as f32) / 0x4000 as f32;
let normalized_full = (zoom_pos.value() as f32) / 0x7000 as f32;

// New (0.8.0): Domain-aware normalization
use grafton_visca::ZoomPositionExt;

let zoom_pos = camera.inquiry().zoom_position().await?;
let optical_norm = zoom_pos.normalize(ZoomDomain::Optical);  // 0.0-1.0
let full_norm = zoom_pos.normalize(ZoomDomain::OpticalPlusDigital);  // 0.0-1.0

// Set zoom from normalized value
camera.zoom_absolute(
    zoom_from_normalized(Normalized(0.5), ZoomDomain::Optical)?
).await?;
```

#### Health Checks

```rust
// Old (0.7.1): Custom probe using arbitrary inquiry
async fn check_camera(camera: &Camera) -> bool {
    tokio::time::timeout(
        Duration::from_millis(100),
        camera.inquiry().zoom_position()
    ).await.is_ok()
}

// New (0.8.0): Dedicated diagnostics
use grafton_visca::diagnostics::Diagnostics;

let report = camera.probe().await?;
if report.is_healthy() {
    println!("RTT: {:?}", report.rtt);
}
```

#### Serialization (New Feature)

```rust
// Enable in Cargo.toml
// grafton-visca = { version = "0.8", features = ["serde", "schemars"] }

use grafton_visca::types::PanSpeed;

// Serialize to JSON
let speed = PanSpeed::new(12)?;
let json = serde_json::to_string(&speed)?;
assert_eq!(json, "12");

// Deserialize from JSON
let speed: PanSpeed = serde_json::from_str("15")?;
assert_eq!(speed.value(), 15);

// Generate JSON schema with schemars
let schema = schemars::schema_for!(PanSpeed);
```

### 🔄 Breaking Changes

#### Transport Handle (Minor)
If you were pattern matching on `TransportHandle`, add the new `Serial` variant:

```rust
// Old (0.7.1)
match transport {
    TransportHandle::Udp(t) => { /* ... */ }
    TransportHandle::Tcp(t) => { /* ... */ }
}

// New (0.8.0)
match transport {
    TransportHandle::Udp(t) => { /* ... */ }
    TransportHandle::Tcp(t) => { /* ... */ }
    #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
    TransportHandle::Serial(t) => { /* ... */ }
}
```

#### Error Matching (Minor)
`ErrorKind` is now `#[non_exhaustive]`, so wildcard patterns are required:

```rust
// Old (0.7.1): Could exhaustively match
match error.kind() {
    ErrorKind::Timeout => { /* ... */ }
    ErrorKind::Cancelled => { /* ... */ }
    // Could list all variants
}

// New (0.8.0): Must include wildcard
match error.kind() {
    ErrorKind::Timeout => { /* ... */ }
    ErrorKind::Cancelled => { /* ... */ }
    _ => { /* ... */ }  // Required
}
```

### 🎓 Migration Strategy

**To adopt new features:**

1. **Opt into serialization**:
   ```toml
   grafton-visca = { version = "0.8", features = ["serde", "schemars"] }
   ```

2. **Replace custom conversion code** with built-in helpers from `inquiry_conversions`

3. **Replace custom speed mapping** with `Coarse` and `from_coarse()` methods

4. **Replace custom health checks** with the `Diagnostics` trait

5. **Remove f32 casts** when constructing `Degrees` and similar types

6. **Use built-in cancellation** via `start_command_with_id()` and `cancel()` for long-running operations

### 📚 Technical Improvements

- **Runtime-neutral design**: All new features work across tokio, async-std, and smol
- **Zero-cost abstractions**: Type-safe wrappers with no runtime overhead
- **Consistent validation**: All types expose MIN/MAX constants and validated constructors
- **Non-exhaustive enums**: Future-proof API with `#[non_exhaustive]` on key enums
- **Comprehensive testing**: Golden vectors for conversions, domain normalization, and edge cases
- **Improved documentation**: All new types include examples and usage notes

### 🔮 Future Direction

Version 0.8.0 represents a major API evolution before 1.0. The focus has shifted from architectural changes to stability, robustness, and ergonomics. The runtime-agnostic foundation is complete, serialization support is in place, and the API surface is clean and minimal. Upcoming releases will focus on:

- Profile auto-detection and capability discovery
- Optional normalized zoom helpers for all camera control methods
- Enhanced timeout support via builder patterns
- Documentation and migration guide refinements
- Stability and bug fixes toward 1.0

## [0.7.1] - 2025-10-10

### Added
- Serial transport support with `TransportHandle::Serial` variant for uniform trait implementation
- Warning messages for unsupported hardware configurations

### Changed
- Improved `Error` type with `Clone` implementation for better error handling
- Enhanced runtime-agnostic spawn background abstraction with `spawn_with_detach` function
- Standardized import organization and code formatting across the codebase
- Improved layout consistency

### Fixed
- Runtime-agnostic background task spawning now properly handles detached tasks

## [0.7.0] - 2025-09-18

This release represents a complete architectural transformation of the library, fundamentally reimagining how VISCA camera control should work in Rust. After hundreds of iterations and refinements since 0.6.0, we've achieved a design that prioritizes simplicity, type safety, and zero-cost abstractions.

### 🎯 Philosophy: Camera-First API Design

The central breakthrough in 0.7.0 is the **camera-first** approach. Instead of exposing protocol details, transport layers, or complex builders, the API now starts with what matters: the camera itself. This seemingly simple change cascades through the entire architecture, eliminating complexity at every level.

```rust
// The entire connection story in one line
let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

// Direct, intuitive control
camera.power_on()?;
camera.zoom_in()?;
camera.pan_tilt_home()?;
```

### 🔧 Core Architectural Changes

#### Unified Type System
- Single `Camera<Profile>` type replaces complex generic hierarchies
- `CameraSession` provides the actual connection and communication
- Profile system enables compile-time validation without runtime overhead
- Transport details completely hidden from public API

#### Zero-Allocation Command Pipeline
- Commands encoded directly into fixed-size arrays
- No heap allocations in the hot path
- Compile-time size calculation for all VISCA messages
- Protocol framing happens at the last possible moment

#### Runtime-Agnostic Architecture
- Blocking and async modes selected via feature flags (`mode-blocking`, `mode-async`)
- No runtime required for blocking mode
- Multiple async runtimes supported (tokio, async-std, smol) with automatic selection
- Executor abstraction allows runtime switching without code changes

#### Type-State Session Management
- `CameraSession` uses type states (Open/Closed) to prevent use-after-close bugs
- RAII pattern ensures proper resource cleanup
- Connection lifecycle managed automatically
- Compile-time guarantees for session validity

### 📝 API Surface Consolidation

The public API has been dramatically simplified while maintaining full VISCA protocol support:

#### Before (0.6.0)
- Multiple client types (`Client`, `AsyncClient`, `ViscaClient`)
- Exposed transport traits and implementations
- Complex builder patterns with many configuration options
- Protocol details leaked into user code

#### After (0.7.0)
- Single `Camera` type with profile parameter
- One-line connection methods via `Connect` trait
- Transport and protocol completely abstracted
- Clean separation between camera control and infrastructure

### 🚀 Major Features & Improvements

#### Connection Simplicity (#403, #404)
- `Connect` trait provides simple `open_tcp_*` and `open_udp_*` methods
- Auto-detection of VISCA protocol variant (Sony vs Generic)
- IPv6 support with automatic address normalization (#402)
- DNS resolution handled transparently

#### Command Architecture (#400, #405)
- Exact-size VISCA encoding with zero allocations
- Compile-time protocol envelope construction
- Sony IP sequence number allocation fused into framing
- Separate inquiry pipeline to prevent head-of-line blocking (#393)

#### Profile System Enhancement
- Camera profiles now define all model-specific constants
- Compile-time validation of parameters against camera capabilities
- Automatic unit conversions based on camera model
- Profile-aware timeout configurations

#### Unified Scheduler (#392)
- Deadline-driven scheduler replaces fixed tick loop
- Commands and inquiries share same scheduling infrastructure
- Automatic retry handling with exponential backoff
- Per-category timeout configuration

### 🔄 Breaking Changes from 0.6.0

Due to the complete architectural overhaul, this release includes extensive breaking changes. Rather than listing each change individually (there are hundreds), here's how to think about migrating:

#### Connection & Setup
```rust
// Old (0.6.0)
let transport = TcpTransport::new("192.168.0.110:52381")?;
let camera = CameraBuilder::new()
    .with_transport(transport)
    .profile::<PtzOpticsG2>()
    .build()?;

// New (0.7.0)
let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
```

#### Feature Flags
```rust
// Old: Complex feature matrix
[features]
default = ["async", "tokio", "tcp", "udp"]

// New: Simple mode selection
[features]
default = ["mode-blocking"]  # or ["mode-async", "tokio"]
```

#### API Access
```rust
// Old: Traits scattered across modules
use grafton_visca::{ViscaZoomExt, ViscaPanTiltExt, ViscaFocusExt};

// New: Everything through Camera methods
use grafton_visca::{Camera, camera::profiles::PtzOpticsG2, camera::Connect};
// All methods available directly on camera instance
```

#### Command Execution
```rust
// Old: Complex command building
let cmd = ZoomCommand::Direct(ZoomPosition::new(0x4000)?);
camera.send(&cmd)?;

// New: Direct methods
camera.zoom_to(0x4000)?;
// Or with units
camera.zoom_to(Normalized::new(0.5)?)?;
```

### 🎓 Migration Strategy

Given the extensive changes, we recommend:

1. **Start Fresh**: Rather than trying to update existing code incrementally, consider rewriting camera control logic using the new API
2. **Use Examples**: The examples in `/examples` demonstrate all common patterns
3. **Leverage Type Safety**: Let the compiler guide you - most old patterns simply won't compile
4. **Simplify**: The new API requires significantly less code - embrace the simplicity

### 📚 Technical Improvements

- **Macro System** (#397, #399): Complete consolidation of internal macros, cleaner organization
- **Naming Consistency** (#396, #398): All types and methods follow consistent naming patterns
- **Protocol Correctness** (#389, #395): Strict VISCA compliance with proper terminator handling
- **Test Infrastructure** (#394): Deterministic executor for reliable async testing
- **Transport Unification** (#381, #387): Blocking and async transports share core logic
- **Error Handling** (#382): Consistent error semantics across all transport types

### 🔮 Future Direction

This release establishes a stable foundation for the 1.0 release. The camera-first API design, combined with zero-cost abstractions and compile-time safety, provides the ideal balance of simplicity and power for VISCA camera control in Rust.

## [0.6.1] - 2025-01-12

### Internal
- Pre-release version with initial architectural improvements
- Foundation for v0.7.0 release

## [0.6.0] - 2025-01-07

### Added

#### 🛡️ Type-Safe VISCA Terminator Pattern (#203)
- Implemented type-state pattern for VISCA terminator safety
- Added `CommandBuilder` for safe command construction with automatic terminator handling
- Consolidated all command constants to use `VISCA_TERMINATOR` constant
- Prevents protocol violations at compile time

#### ⏱️ Timeout System Enhancements (#202, #204)
- Moved timeout categories to type system constants
- Profile-specific timeout configurations for different camera models
- Compile-time timeout validation

#### 🎯 Event-Driven Movement Detection
- New event-driven system for camera movement detection
- Eliminates polling delays in movement completion detection
- More responsive and efficient movement tracking

#### 🔧 Code Quality Improvements (#193)
- Resolved all Clippy warnings including uninlined format args
- Fixed async feature compilation without tokio
- Eliminated sleep anti-patterns in examples (#191)
- Improved code formatting and documentation

### Fixed
- Corrected README path for crates.io publishing
- Fixed Clippy warnings for async feature without tokio dependency
- Resolved all CI workflow quality check failures

### Internal
- Applied comprehensive code formatting improvements
- Enhanced test coverage for new type-safe patterns
- Improved example code quality and best practices

## [0.5.0] - 2025-01-06

### Breaking Changes

#### 🔥 New CameraBuilder API (Issue #185)
- **BREAKING**: Removed all `connect_*` functions (`connect_tcp`, `connect_udp`, `connect_tokio_tcp`, `connect_tokio_udp`)
- **NEW**: Introduced `CameraBuilder` for a cleaner, more idiomatic API:
  ```rust
  // Before (removed):
  let camera = Camera::<PTZOpticsG2, _>::connect_tcp("192.168.0.110")?;

  // After (new):
  let camera = CameraBuilder::tcp("192.168.0.110")
      .profile::<PTZOpticsG2>()
      .build()?;
  ```
- Benefits of the new API:
  - Runtime parameters (address, protocol) come first
  - Compile-time profile selection comes second
  - Single entry point (`CameraBuilder`) for all transport types
  - More extensible for future transport options

### Changed
- Updated all examples to use the new `CameraBuilder` API
- Updated README and documentation with new builder pattern examples

## [0.4.0] - 2024-12-27

This release represents a major evolution of the library from a low-level VISCA protocol implementation to a high-level camera control solution with a unified, ergonomic API.

### Changed

#### 📝 Documentation Updates
- Toned down overstated claims in README to better reflect development status
- Added development status warning to README
- Adjusted feature claims to be more accurate and modest
- Clarified that test coverage is being expanded rather than complete
- Removed performance optimization claims pending benchmarking

### Added

#### 🎯 Unified Client Architecture
- New unified `Client` that works seamlessly in both sync and async contexts
- Thread-safe and `Clone`able client - share it freely across your application
- Automatic context detection - the client adapts to your code style
- Connection pooling built-in
- Automatic reconnection with configurable retry strategies
- Camera model configuration via `Client::builder()` for automatic command validation

#### 🔄 Resilience Features
- `ReconnectingTransport` with exponential backoff and configurable retries
- `ConnectionPool` for managing multiple cameras efficiently
- Health check system with automatic recovery
- Detailed error types that indicate retry-ability

#### 🎮 Camera Model Validation
- Optional camera model configuration to prevent invalid commands before sending
- Model-specific validation for zoom ranges (20X vs 30X cameras)
- Model-specific validation for pan/tilt absolute positions
- New `ModelValidation` error type for clear validation failure messages
- Maintains backward compatibility - validation is opt-in via `Client::builder()`
- Timeout management with per-command category timeouts

#### 🎨 High-Level Extension Traits
- `ViscaPowerExt`, `ViscaZoomExt`, `ViscaFocusExt`, etc. for domain-specific operations
- `PTZBuilder` for complex camera movements with intuitive units:
  - Degrees for pan/tilt (`pan_to_degrees(45.0)`)
  - Magnification for zoom (`zoom_to_magnification(10.0)`)
  - Percentages for positioning (`set_pan_tilt_percentage(0.5, -0.25)`)
- White balance fine-tuning methods (`white_balance_red_tuning()`, `white_balance_blue_tuning()`)
- Anti-flicker control (`set_anti_flicker()`)
- Sequential and concurrent command execution modes

#### 🚦 Enhanced Error Handling
- Specific error types for different failure modes
- `is_retryable()` method on errors
- `suggested_retry_delay()` for intelligent retry logic
- Context-aware errors with parameter ranges
- Removed `#[non_exhaustive]` for exhaustive error matching

#### 📚 Developer Experience
- Comprehensive prelude module (`use grafton_visca::prelude::*`)
- 30+ real-world examples demonstrating common use cases
- Complete API documentation with examples
- Zero clippy warnings (even on pedantic level)
- Simplified feature flags - just works out of the box

#### 🔧 Procedural Macros (grafton-visca-macros)
- Implemented `#[visca_command_variants]` for generating multiple method variants accepting different input types (raw values, typed wrappers, percentages, etc.)
- Implemented `#[visca_inquiry]` for automatic response parsing based on command type
- Added `#[visca_position_command]` for position-based commands with automatic validation and unit conversions (degrees, normalized values)
- Added `#[visca_speed_command]` for speed-based commands with SpeedLevel enum support
- Added `#[visca_bounded_command]` for bounded value commands with percentage variants and named level enums

### Changed

#### **BREAKING**: Complete API Overhaul
- **Core Type Renames** (cleaner, more idiomatic):
  - `ViscaClient` → `Client`
  - `ViscaError` → `Error`
  - `ViscaSession` → `Session`
  - `ViscaResponse` → `Response`
  - `ViscaCommand` → `Command` trait
  - `ViscaTransport` → `ViscaProtocol` struct

- **Command Naming Improvements**:
  - **Zoom**: `Tele/Wide` terminology → `ZoomIn/ZoomOut` throughout
    - `ZoomCommand::TeleStandard` → `ZoomCommand::ZoomInStandard`
    - `ZoomCommand::WideStandard` → `ZoomCommand::ZoomOutStandard`
    - Extension methods: `zoom_in_variable()` → `zoom_in_speed()`
  - **Focus**: Added clarity with prefixes
    - `FarStandard` → `FocusFarStandard`
    - `AFSensitivity` → `AutoFocusSensitivity`
  - **Presets**: Aligned with VISCA specification
    - `save_preset()` → `set_preset()`
    - `goto_preset()` → `recall_preset()`
  - **White Balance**: Simplified method names
    - `set_color_temperature_direct()` → `set_color_temperature()`

- **Transport Layer Evolution**:
  - Async-first design with blocking adapters
  - Unified transport abstraction across TCP/UDP
  - Built-in connection pooling and reconnection (no more feature flags)

- **Feature Flag Simplification**:
  - Removed complex feature matrix
  - Default is blocking client
  - Single `async` feature for async runtime
  - Connection pooling and reconnection are now standard

### Removed
- All deprecated type aliases from previous versions
- Duplicate convenience methods that didn't match VISCA spec
- Old split client implementations (`ViscaClient` vs `AsyncViscaClient`)
- Complex feature flag requirements for basic functionality
- Direct `power_on()`/`power_off()` methods (use `PowerCommand` instead)
- Orphaned async extension files from earlier refactoring

### Internal
- **Macro Consolidation** (Issue #201): Consolidated all macros into a single module hierarchy at `src/macros/` with clear separation between public API macros, internal implementation macros, and test utilities. Removed `#[macro_export]` from internal macros to prevent namespace pollution.

### Fixed
- Thread safety issues - client is now truly thread-safe without `RefCell`
- Feature gating problems with `no-default-features` builds
- All clippy warnings including pedantic lints
- Protocol edge cases in response handling
- Missing functionality in unified client (`try_send()`, `send_with_timeout()`)
- Example files now have proper feature requirements

### Performance Improvements
- Const functions used where possible
- Reduced allocations in command building
- More efficient response parsing
- Better memory usage patterns

## Migration Guide from v0.3.0

### Step 1: Update Your Cargo.toml
```toml
# Old
[dependencies]
grafton-visca = { version = "0.3", features = ["async", "sync", "reconnect", "pool"] }

# New - Blocking by default
grafton-visca = "0.4"

# OR for async
grafton-visca = { version = "0.4", default-features = false, features = ["async"] }
```

### Step 2: Update Imports
```rust
// Old
use grafton_visca::{ViscaClient, ViscaError, ViscaResponse, ViscaSession};
use grafton_visca::command::ViscaCommand;

// New
use grafton_visca::{Client, Error, Response, Session};
use grafton_visca::command::Command;

// Or use the prelude for common imports
use grafton_visca::prelude::*;
```

### Step 3: Update Client Creation
```rust
// Old - had to choose between sync and async
let client = ViscaClient::new(transport);
let client = AsyncViscaClient::new(async_transport);

// New - unified client works everywhere
let client = Client::connect_tcp("192.168.0.110")?;
// Use the same client in both sync and async code!
```

### Step 4: Update Method Calls
```rust
// Old power control
client.power_on()?;
client.power_off()?;

// New - use PowerCommand directly
use grafton_visca::command::{PowerCommand, power::Power};
client.send(&PowerCommand::new(Power::On))?;
client.send(&PowerCommand::new(Power::Standby))?;

// Or use extension trait
use grafton_visca::ViscaPowerExt;
client.set_power(Power::On)?;

// Old zoom methods (removed)
client.zoom_in()?;
client.zoom_out()?;

// New - use extension trait methods
use grafton_visca::ViscaZoomExt;
client.zoom_in()?;  // Standard speed
client.zoom_in_speed(Some(ZoomSpeed::new(5)?))?;  // Variable speed

// Old preset names
client.save_preset(1)?;
client.goto_preset(1)?;

// New - matches VISCA specification
client.set_preset(1)?;
client.recall_preset(1)?;
```

### Step 5: Update Type Names in Your Code
```rust
// Old
fn connect_camera(addr: &str) -> Result<ViscaClient, ViscaError> {
    ViscaClient::connect_udp(addr)
}

fn handle_response(resp: ViscaResponse) -> Result<(), ViscaError> {
    // ...
}

// New
fn connect_camera(addr: &str) -> Result<Client, Error> {
    Client::connect_udp(addr)
}

fn handle_response(resp: Response) -> Result<(), Error> {
    // ...
}
```

### Step 6: Use High-Level APIs
```rust
// Old - manual VISCA units
let pan_pos = 0x0800;  // What does this mean?
let tilt_pos = 0x0000;
client.send_command(PanTiltAbsolute::new(pan_pos, tilt_pos))?;

// New - intuitive units with PTZ builder
client.ptz()
    .pan_tilt_to(45.0, -15.0)  // Degrees!
    .zoom_to_magnification(5.0)  // 5x zoom
    .wait()  // Execute sequentially
    .execute()?;
```

### Step 7: Update Error Handling
```rust
// Old - generic errors
match result {
    Err(e) => eprintln!("Error: {}", e),
    Ok(_) => {}
}

// New - specific, actionable errors
match result {
    Err(e) if e.is_retryable() => {
        sleep(e.suggested_retry_delay());
        retry()?;
    }
    Err(Error::CameraMoving) => {
        client.wait_for_completion()?;
    }
    Err(Error::OutOfRange { param, min, max }) => {
        println!("{} must be between {} and {}", param, min, max);
    }
    _ => {}
}
```

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
  - Added trait-level documentation for `ViscaProtocol` and `ViscaCommand`
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
