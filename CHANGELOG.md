# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

This release represents a major evolution of the library from a low-level VISCA protocol implementation to a production-ready camera control solution. The changes are driven by real-world usage patterns and developer feedback.

### Why These Changes?

The v0.3.0 release revealed several pain points:
- Developers struggled with choosing between sync and async APIs, often needing both
- Thread safety required verbose `RefCell<Box<dyn ViscaTransport>>` patterns (61+ instances!)
- No built-in support for common production needs like reconnection and connection pooling
- Low-level VISCA units made simple operations unnecessarily complex
- Generic errors made it hard to implement proper retry logic

### What's New For You

**🎯 One Client To Rule Them All**
```rust
// Before: Choose your fighter...
let client = ViscaClient::new(...);      // Sync only
let client = AsyncViscaClient::new(...);  // Async only
let client = ViscaClientWrapper::new(...); // Both (but awkward)

// Now: Just use ViscaClient everywhere!
let client = ViscaClient::new("192.168.1.100:52381")?;
client.zoom_in()?;  // Works in sync code
client.zoom_in().await?;  // Works in async code
```
The new unified `ViscaClient` automatically detects your context and does the right thing. It's also thread-safe and `Clone`able - share it freely across your application!

**🔄 Production-Ready Resilience**
```rust
// Your camera connection died? No problem!
let transport = ReconnectingTransport::new(transport)
    .with_max_retries(5)
    .with_exponential_backoff();

// Managing multiple cameras? Built-in pooling!
let pool = ViscaConnectionPool::new()
    .with_capacity(10)
    .with_health_check_interval(Duration::from_secs(30));
```
Network issues are now handled automatically. Connection pooling and health checks ensure your production systems stay running.

**🎬 Cinematic Camera Control**
```rust
// Complex camera movements are now simple
client.ptz()
    .pan_tilt_to(-45.0, 15.0)  // Degrees!
    .zoom_to_magnification(10.0)  // 10x zoom!
    .wait()  // Execute sequentially
    .focus_auto()
    .execute()?;

// Or run commands in parallel
client.ptz()
    .pan_to_degrees(90.0)
    .zoom_in()
    .concurrent()  // Execute simultaneously
    .execute()?;
```
The new PTZ builder makes complex shots easy. Use degrees, percentages, or magnification values instead of cryptic VISCA units.

**🎨 High-Level Operations**
```rust
// Save and recall camera positions
let position = client.get_current_position()?;
client.save_preset(1, "Wide Shot")?;
client.recall_preset_by_name("Wide Shot")?;

// Work with intuitive units
client.zoom_to_magnification(5.0)?;  // 5x zoom
client.pan_to_degrees(45.0)?;        // 45 degrees right
client.set_pan_tilt_percentage(0.5, -0.25)?;  // Center-right, slightly down
```
Extension traits add domain-specific operations that match how you think about camera control.

**🚦 Smarter Error Handling**
```rust
match client.zoom_in() {
    Err(e) if e.is_retryable() => {
        // Network error - wait and retry
        sleep(e.suggested_retry_delay());
        client.zoom_in()?;
    }
    Err(ViscaError::CameraMoving) => {
        // Camera is busy - wait for it to stop
        client.wait_for_completion()?;
    }
    Err(ViscaError::OutOfRange { param, min, max }) => {
        // Invalid parameter - show helpful error
        println!("{} must be between {} and {}", param, min, max);
    }
    _ => {}
}
```
Detailed error types tell you exactly what went wrong and how to fix it.

### Breaking Changes (And Why They're Worth It)

**🔧 Simplified Feature Flags**
```toml
# Before: Confusing feature matrix
[dependencies]
grafton-visca = { version = "0.3", features = ["async", "sync", "reconnect", "pool"] }

# Now: Just pick your runtime model
grafton-visca = "0.4"  # Blocking by default
# OR
grafton-visca = { version = "0.4", default-features = false, features = ["async-client"] }
```
Connection pooling and reconnection are now standard - no more feature flag puzzles! The library just works out of the box.

**🏗️ Transport Layer Evolution**

If you were using transports directly (most users weren't), the API has changed to be async-first:
```rust
// Old way (probably wasn't working well anyway)
let transport = UdpTransport::new(...);
transport.send_command(&cmd)?;

// New way (but you probably want ViscaClient instead)
let client = ViscaClient::new("192.168.1.100:52381")?;
client.zoom_in()?;  // Much simpler!
```

**🎯 Better Errors Mean Better Code**
```rust
// Your error handling just got smarter
match result {
    Err(e) if e.is_retryable() => {
        // The error tells you if retry makes sense!
        tokio::time::sleep(e.suggested_retry_delay()).await;
        retry()?;
    }
    Err(ViscaError::CameraMoving) => {
        // Specific errors for specific situations
        wait_for_camera_stop().await?;
    }
    _ => {}
}
```
We removed `#[non_exhaustive]` from errors - you can now handle every possible error case with confidence.

### Migration Guide

**From v0.3.0 to v0.4.0:**

1. **Update your Cargo.toml:**
   ```toml
   # Remove feature flags for reconnect and pool
   grafton-visca = "0.4"
   ```

2. **Replace split clients with unified client:**
   ```rust
   // Old
   let client = if async { AsyncViscaClient::new() } else { ViscaClient::new() };
   
   // New
   let client = ViscaClient::new("192.168.1.100:52381")?;
   ```

3. **Use high-level operations:**
   ```rust
   // Old: Manual VISCA units
   client.send_command(PanTiltAbsolute::new(0x0800, 0x0000))?;
   
   // New: Intuitive units
   client.pan_to_degrees(45.0)?;
   ```

4. **Update error handling:**
   ```rust
   // Add new error variants to your match statements
   match error {
       ViscaError::CameraMoving => { /* wait */ }
       ViscaError::OutOfRange { param, min, max } => { /* show range */ }
       // ... other cases
   }
   ```

### What We Fixed

- **Thread Safety**: No more `RefCell` gymnastics - the client is truly thread-safe
- **Documentation**: Every public API is now documented with examples
- **Code Quality**: Zero clippy warnings (even pedantic ones!)
- **Type Safety**: Stronger types prevent unit confusion
- **Performance**: Const functions where possible, better memory usage
- **Reliability**: Edge cases in protocol handling, robust timeout management

### Developer Experience Improvements

**📚 Better Examples**
Check out our new examples that show real-world usage:
- `hello_visca.rs` - Your first camera control
- `async_control_demo.rs` - Modern async patterns
- `production_setup.rs` - Reconnection, pooling, and monitoring
- `cinematic_shots.rs` - Complex camera movements
- Plus 15+ more examples!

**🎨 Cleaner Imports**
```rust
use grafton_visca::prelude::*;  // Everything you need!
```

**🔍 Superior Debugging**
Every error now includes context about what went wrong and how to fix it. Response parsing shows exactly where issues occur.

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