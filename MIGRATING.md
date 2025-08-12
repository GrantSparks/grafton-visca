# Migration Guide: v0.4 to v0.5

This guide covers the major breaking changes in v0.5 and how to update your code. The v0.5 release introduces a complete architectural overhaul focused on zero-cost abstractions, explicit runtime configuration, and improved type safety.

## Executive Summary

v0.5 introduces:
- Zero-boxing async transports with native `async fn` in traits (requires Rust 1.75+)
- Explicit runtime configuration (no silent fallbacks)
- Unified `Camera<Mode, Profile, Transport>` type with mode markers
- Complete DNS and IPv6 support across all transports
- Improved timeout configuration with ACK timeout support

## Major Breaking Changes

### 1. Transport Trait Split

The unified `Transport` trait has been split into two distinct traits:

**Before (v0.4):**
```rust
use grafton_visca::Transport;

impl Transport for MyTransport {
    type RecvFuture<'a> = Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + Send + 'a>>;
    type SendFuture<'a> = Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>>;
    
    fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move { /* ... */ })
    }
    
    fn recv<'a>(&'a mut self) -> Self::RecvFuture<'a> {
        Box::pin(async move { /* ... */ })
    }
}
```

**After (v0.5):**
```rust
// For async transports
use grafton_visca::transport::AsyncTransport;

impl AsyncTransport for MyAsyncTransport {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Direct async implementation, no boxing!
    }
    
    async fn recv(&mut self) -> Result<Vec<u8>, Error> {
        // Direct async implementation, no boxing!
    }
}

// For blocking transports
use grafton_visca::transport::BlockingTransport;

impl BlockingTransport for MyBlockingTransport {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Blocking implementation
    }
    
    fn recv(&mut self) -> Result<Vec<u8>, Error> {
        // Blocking implementation
    }
}
```

### 2. Camera Construction and Runtime Requirements

**Before (v0.4):**
```rust
// Async camera with automatic runtime
let camera = Camera::new("192.168.0.110:5678")?;

// Or with builder
let camera = CameraBuilder::tcp("192.168.0.110")
    .profile::<PTZOpticsG2>()
    .build()?;
```

**After (v0.5):**
```rust
// Async cameras now REQUIRE explicit runtime configuration
use grafton_visca::runtime::{TokioRuntime, SharedRuntime};
use std::sync::Arc;

// Create runtime explicitly
let runtime: SharedRuntime = Arc::new(TokioRuntime);

// Method 1: Using builder (runtime must be added after build)
let camera = CameraBuilder::tokio_tcp("192.168.0.110")
    .profile::<PTZOpticsG2>()
    .build()
    .await?
    .with_runtime(runtime.clone());  // REQUIRED for async operations

// Method 2: Direct construction
let transport = grafton_visca::transport::tokio::Tcp::connect("192.168.0.110:5678").await?;
let camera = CameraAsync::<PTZOpticsG2, _>::from_transport(transport)
    .with_runtime(runtime);  // REQUIRED for async operations

// Blocking cameras don't need runtime
let camera = CameraBuilder::tcp("192.168.0.110")
    .profile::<PTZOpticsG2>()
    .build()?;  // No runtime needed for blocking
```

### 3. Camera Type Changes

**Before (v0.4):**
```rust
use grafton_visca::{Camera, AsyncCamera, BlockingCamera};

// Separate types for async and blocking
let async_camera: AsyncCamera<PTZOpticsG2> = /* ... */;
let blocking_camera: BlockingCamera<PTZOpticsG2> = /* ... */;
```

**After (v0.5):**
```rust
use grafton_visca::camera::{Camera, AsyncMode, BlockingMode, CameraAsync, CameraBlocking};

// Single Camera type with mode markers
let async_camera: Camera<AsyncMode, PTZOpticsG2, TokioTcp> = /* ... */;
let blocking_camera: Camera<BlockingMode, PTZOpticsG2, Tcp> = /* ... */;

// Or use convenient type aliases
let async_camera: CameraAsync<PTZOpticsG2, TokioTcp> = /* ... */;
let blocking_camera: CameraBlocking<PTZOpticsG2, Tcp> = /* ... */;
```

### 4. Method Name Changes

Several methods have been renamed for clarity:

| Old Method | New Method | Notes |
|------------|------------|-------|
| `zoom_in()` | `zoom_tele_std()` | Telephoto/zoom in |
| `zoom_out()` | `zoom_wide_std()` | Wide angle/zoom out |
| `Camera::new()` | `Camera::from_transport()` | Direct construction |
| `wait_for_pan_tilt_completion()` | `await_pan_tilt_idle()` | Better async naming |
| `wait_for_zoom_completion()` | `await_zoom_idle()` | Better async naming |

### 5. Timeout Configuration

**Before (v0.4):**
```rust
// Limited timeout configuration
let config = TimeoutConfig {
    command_timeout: Duration::from_secs(5),
    // No ACK timeout control
};
```

**After (v0.5):**
```rust
// Enhanced timeout configuration with ACK support
let config = TimeoutConfig {
    ack_timeout: Duration::from_millis(500),  // NEW: ACK timeout (default 500ms)
    quick_timeout: Duration::from_secs(5),
    movement_timeout: Duration::from_secs(30),
    preset_timeout: Duration::from_secs(90),
    // ... other categories
};

// Or use builder
let config = TimeoutConfig::builder()
    .ack_timeout(Duration::from_millis(250))  // Custom ACK timeout
    .movement_timeout(Duration::from_secs(60))
    .build();
```

### 6. Error Handling

**Before (v0.4):**
```rust
// Silent runtime fallback
let camera = CameraAsync::new("192.168.0.110")?;  // Would create default runtime
```

**After (v0.5):**
```rust
// Explicit error when runtime is missing
let camera = CameraAsync::from_transport(transport);
match camera.zoom_tele_std().await {
    Err(Error::MissingRuntime) => {
        // Must provide runtime via .with_runtime()
    }
    _ => {}
}
```

## DNS and IPv6 Support

All transports now support DNS resolution and IPv6:

```rust
// All of these now work consistently:
let camera = CameraBuilder::tcp("camera.local")  // DNS resolution
    .profile::<PTZOpticsG2>()
    .build()?;

let camera = CameraBuilder::tcp("[::1]:5678")  // IPv6
    .profile::<PTZOpticsG2>()
    .build()?;

let camera = CameraBuilder::udp("239.0.0.1")  // Multicast
    .profile::<PTZOpticsG2>()
    .build()?;
```

## Custom Runtime Implementation

For non-Tokio runtimes, implement the `Runtime` trait:

```rust
use grafton_visca::runtime::Runtime;
use grafton_visca::executor::{SpawnableFuture, Sleep, Spawner};

#[derive(Debug)]
struct MyRuntime;

impl Runtime for MyRuntime {
    fn spawn(&self, task: SpawnableFuture) {
        my_executor::spawn(task);
    }
    
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(my_executor::sleep(duration))
    }
}

// Use your runtime
let runtime = Arc::new(MyRuntime);
let camera = camera.with_runtime(runtime);
```

## Complete Migration Example

**Before (v0.4):**
```rust
use grafton_visca::{Camera, PTZOpticsG2};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Old API with implicit runtime
    let mut camera = Camera::<PTZOpticsG2>::new("192.168.0.110")?;
    
    camera.power_on().await?;
    camera.zoom_in().await?;
    camera.pan_tilt_absolute(45.0, 15.0, 18).await?;
    camera.wait_for_pan_tilt_completion().await?;
    
    Ok(())
}
```

**After (v0.5):**
```rust
use grafton_visca::{
    CameraBuilder,
    camera::profiles::PTZOpticsG2,
    runtime::{TokioRuntime, SharedRuntime},
    command::pan_tilt::{PanPosition, TiltPosition, PanSpeed, TiltSpeed},
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // New API with explicit runtime
    let runtime: SharedRuntime = Arc::new(TokioRuntime);
    
    let mut camera = CameraBuilder::tokio_tcp("192.168.0.110")
        .profile::<PTZOpticsG2>()
        .build()
        .await?
        .with_runtime(runtime);  // Explicit runtime configuration
    
    camera.power_on().await?;
    camera.zoom_tele_std().await?;  // Renamed method
    camera.pan_tilt_absolute(
        PanPosition::from_degrees(45.0),
        TiltPosition::from_degrees(15.0),
        PanSpeed::new(18)?,
        TiltSpeed::new(18)?,
    ).await?;
    camera.await_pan_tilt_idle().await?;  // Renamed method
    
    Ok(())
}
```

## Troubleshooting

### Error: `MissingRuntime`
**Cause:** Attempting async operations without configuring a runtime.
**Solution:** Call `.with_runtime()` on your camera with a valid runtime.

### Error: Type mismatch with `Camera`
**Cause:** The Camera type now requires mode and transport parameters.
**Solution:** Use type aliases `CameraAsync` or `CameraBlocking`, or specify full type.

### Compilation error with `async fn` in traits
**Cause:** Your Rust version is too old.
**Solution:** Update to Rust 1.75 or later.

## Benefits of v0.5

- **Zero-cost abstractions**: No boxing overhead in async hot paths
- **Explicit configuration**: No hidden runtime creation or surprising defaults
- **Better type safety**: Mode markers prevent mixing async/blocking operations
- **Consistent networking**: DNS and IPv6 work everywhere
- **Fine-grained timeouts**: Control ACK timeouts separately from command timeouts

## Getting Help

If you encounter issues not covered in this guide:
1. Check the examples in the `examples/` directory
2. Review the API documentation
3. Open an issue on GitHub with your migration challenge