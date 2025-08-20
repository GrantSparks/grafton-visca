# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Transport Builder**: Enhanced `TransportBuilder` with comprehensive connection options
  - DNS resolution support for hostnames
  - IPv4/IPv6 address parsing
  - Configurable timeouts and buffer sizes
  - Support for both blocking and async transports
- **Async Wrapper**: New `AsyncWrapper` pattern for simplifying async transport implementations
  - Reduces code duplication between blocking and async transports
  - Provides consistent error handling and timeout behavior
- **Protocol Improvements**:
  - Streamlined runtime and protocol modules
  - Consolidated envelope tests for better maintainability
  - Enhanced frame parsing with better error recovery

### Changed
- **Transport Refactoring**: Major refactoring of transport layer
  - Simplified `ip_raw` and `ip_sony` transport implementations
  - Improved buffer management with cleaner abstractions
  - Better separation of concerns between protocol layers
- **Runtime Improvements**:
  - Enhanced runtime module with better task scheduling
  - Improved scheduler implementation for command prioritization
  - More efficient handling of ACK/Completion sequences
- **Code Quality**:
  - Removed redundant TODO comments throughout codebase
  - Cleaned up imports and module organization
  - Consolidated test utilities for better reusability
  - Simplified example code for better clarity

### Fixed
- TCP test race condition on Windows
- Missing `AsyncTransport` import in build_async_wrapper doctest
- Various clippy warnings and formatting issues
- Visibility issues in envelope and protocol modules

### Internal
- Reduced test suite complexity by removing redundant tests
- Improved test organization with better helper utilities
- Streamlined CI/CD checks for faster builds

## [0.7.0] - 2025-01-18

### Major Architectural Improvements (Issues #226-#243)

This release introduces significant architectural improvements focused on zero-cost abstractions, better type safety, and improved network support.

### Breaking Changes

#### 🚀 Zero-Cost Async Architecture (Issue #226)
- **BREAKING**: Complete redesign of transport traits to eliminate boxing overhead
  - Removed GAT-based `Transport` trait
  - Added separate `AsyncTransport` and `BlockingTransport` traits with native async functions
  - Zero heap allocations in hot paths
- **BREAKING**: Unified camera type with compile-time mode dispatch
  - Single `Camera<Mode, Profile, Transport, Executor>` type
  - `BlockingMode` and `AsyncMode` zero-size type markers
- **BREAKING**: Camera construction changes
  - Changed from direct construction to `CameraBuilder` pattern
  - Explicit transport creation with transport types (`BlockingTcp`, `BlockingUdp`, etc.)
- **BREAKING**: Method name changes for clarity
  - `zoom_in()` → `zoom_tele_std()` (telephoto/zoom in)
  - `zoom_out()` → `zoom_wide_std()` (wide angle/zoom out)
  - Movement await methods: `await_pan_tilt_idle()`, `await_zoom_idle()`, `await_focus_idle()`
- **BREAKING**: Position and speed types now require explicit construction
  - Use `Degrees` and `Normalized` types for positions
  - Use `PanSpeed::new()`, `TiltSpeed::new()` with numeric values

#### 🔄 API Consolidation and Cleanup (Issue #222)
- **BREAKING**: Removed duplicate setter methods from `Camera` struct:
  - `set_nd_filter_mode()` - Use `NDFilterOps` trait methods instead
  - `set_motion_sync_mode()` - Use `MotionSyncControl` trait methods instead
- **BREAKING**: Changed return types for all menu control methods from `Result<Response, Error>` to `Result<(), Error>`
  - Affects `MenuControlOps` and `DirectMenuControlOps` traits
  - Simplifies API by returning unit type for command acknowledgments
- **BREAKING**: Renamed module `generic_methods.rs` to `capability_introspection.rs` to better reflect its purpose

### Added
- **Network**: Full DNS resolution support for hostnames
- **Network**: Consistent IPv6 support across all transports (TCP and UDP)
- **Timeout**: ACK timeout configuration (default 75ms) for camera acknowledgments
- **Timeout**: Fine-grained timeout control per operation category
- **Inquiry**: Complete set of inquiry methods added to Camera struct
  - `get_power_state()`, `get_focus_mode()`, `get_exposure_mode()`, etc.
  - Direct methods on Camera for better ergonomics
- **Documentation**: Comprehensive migration guide in MIGRATION.md
- New `map_ack_to_unit()` helper function in `src/command/util.rs` for standardized response handling
- Async facade now uses `forward_facade!` macro for consistency with blocking implementation

### Changed
- **Performance**: Eliminated all boxing in async code paths
- **Runtime**: Runtime is no longer optional - async always requires runtime, blocking never uses it
- **Transport**: TCP and UDP transports rewritten for zero-cost abstractions
- Module `src/camera/generic_methods.rs` renamed to `src/camera/capability_introspection.rs`
- Documentation updated to clarify that capability introspection methods are read-only
- Async and blocking facades now have identical API surfaces generated from the same macro specification

### Removed
- **Legacy**: Removed `src/async.rs` and `src/blocking.rs` modules
- **Legacy**: Removed GAT-based `Transport` trait
- **Legacy**: Removed boxed futures from all async implementations

### Migration Guide
See [MIGRATION.md](MIGRATION.md) for complete migration instructions from v0.6.x to v0.7.0.

Key points:
1. Update camera construction to use `CameraBuilder` pattern
2. Replace method calls according to the name changes (e.g., `zoom_in()` → `zoom_tele_std()`)
3. Update position/speed types to use explicit constructors
4. Import specific trait modules as needed for accessing control methods

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
  let camera = Camera::<PTZOpticsG2, _>::connect_tcp("192.168.0.110:52381")?;
  
  // After (new):
  let camera = CameraBuilder::tcp("192.168.0.110:52381")
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
let client = Client::connect_tcp("192.168.0.110:52381")?;
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