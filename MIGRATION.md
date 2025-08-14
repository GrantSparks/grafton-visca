# Migration Guide

## Executor-Based API Changes (Issue #235)

### Overview
The library now uses a unified `Executor` trait to prevent runtime/spawner mismatches. The Camera type has been updated from 3 to 4 type parameters to include the executor type.

### Key Changes

**Old API (could mismatch runtime/spawner):**
```rust
let camera = CameraBuilder::new()
    .runtime(tokio_runtime)      // From Tokio
    .spawner(async_std_spawner)  // From async-std - MISMATCH!
    .build()?;
```

**New API (mismatch impossible):**
```rust
// Option 1: Convenience constructor
let camera = Camera::<_, PTZOpticsG2, _, _>::tokio(transport)?;

// Option 2: Explicit executor
let executor = TokioExecutor::from_current()?;
let camera = Camera::with_executor(transport, executor);
```

### Camera Type Changes
- Old: `Camera<M, P, T>` (mode, profile, transport)
- New: `Camera<M, P, T, E>` (mode, profile, transport, executor)

### For Blocking Mode
```rust
// Still works
let camera = Camera::from_transport(transport);

// Or explicitly
let camera = Camera::<BlockingMode, PTZOpticsG2, _, ()>::new(transport);
```

### For Async Mode
```rust
// Must provide executor
let executor = TokioExecutor::from_current()?;
let camera = Camera::with_executor(transport, executor);

// Or use convenience methods
let camera = Camera::tokio(transport)?;
```

---

# Migration Guide: v0.6.x to v0.7.0

This guide helps you migrate from grafton-visca v0.6.x to v0.7.0, which introduces significant architectural improvements for better performance and type safety.

## Overview of Changes

v0.7.0 brings major improvements to the library architecture:
- **Zero-cost async**: Native async functions without boxing overhead
- **Unified Camera type**: Single `Camera<Mode, Profile, Transport>` type with compile-time dispatch
- **Better DNS/IPv6 support**: Consistent network handling across all transports
- **Improved timeout configuration**: Fine-grained control over different operation types

## Breaking Changes

### 1. Camera Construction

The camera construction API has changed to use a builder pattern with explicit transport creation.

**Before (v0.6.x):**
```rust
// Async
let camera = PTZOpticsG2Cam::new("192.168.0.110").await?;

// Blocking
let camera = PTZOpticsG2Cam::new("192.168.0.110")?;
```

**After (v0.7.0):**
```rust
// Async
use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};

let camera = CameraBuilder::new()
    .with_profile(PTZOpticsG2)
    .connect_tcp("192.168.0.110")
    .await?;

// Blocking
let camera = CameraBuilder::new()
    .with_profile(PTZOpticsG2)
    .connect_tcp_blocking("192.168.0.110")?;
```

### 2. Camera Type Changes

The camera type is now parameterized with mode markers for compile-time async/blocking dispatch.

**Before (v0.6.x):**
```rust
use grafton_visca::camera::PTZOpticsG2Cam;
let camera: PTZOpticsG2Cam = /* ... */;
```

**After (v0.7.0):**
```rust
use grafton_visca::camera::{CameraAsync, CameraBlocking, profiles::PTZOpticsG2};
use grafton_visca::transport::tokio::Tcp;

// Async camera
let camera: CameraAsync<PTZOpticsG2, Tcp> = /* ... */;

// Blocking camera  
let camera: CameraBlocking<PTZOpticsG2, TcpBlocking> = /* ... */;
```

### 3. Transport Traits

The transport layer has been completely redesigned to eliminate boxing overhead.

**Before (v0.6.x):**
```rust
// GAT-based trait with boxing
trait Transport {
    type SendFuture<'a>: Future<Output = Result<()>> + Send where Self: 'a;
    type RecvFuture<'a>: Future<Output = Result<Bytes>> + Send where Self: 'a;
    
    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a>;
    fn recv(&self) -> Self::RecvFuture<'_>;
}
```

**After (v0.7.0):**
```rust
// Native async without boxing
trait AsyncTransport: Send + Sync {
    fn send(&self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;
    fn recv(&self) -> impl Future<Output = Result<Bytes, Error>> + Send;
}

// Separate blocking trait
trait BlockingTransport: Send + Sync {
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error>;
    fn recv_blocking(&self) -> Result<Bytes, Error>;
    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes, Error>;
}
```

### 4. Method Name Changes

Several methods have been renamed for clarity and consistency.

| Old Method (v0.6.x) | New Method (v0.7.0) | Notes |
|---------------------|---------------------|-------|
| `zoom_in()` | `zoom_tele_std()` | Telephoto (zoom in) |
| `zoom_out()` | `zoom_wide_std()` | Wide angle (zoom out) |
| `get_pan_tilt_position()` | `get_pan_tilt_degrees()` | Returns degrees |
| `get_zoom_position()` | `get_zoom()` | Returns zoom value |
| `Camera::new()` | `Camera::from_transport()` | For custom transports |

### 5. Position and Speed Types

Pan/tilt operations now use strongly-typed position and speed values.

**Before (v0.6.x):**
```rust
use grafton_visca::units::{Degrees, SpeedLevel};

camera.pan_tilt_absolute(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
).await?;
```

**After (v0.7.0):**
```rust
use grafton_visca::types::{PanPosition, TiltPosition, PanSpeed, TiltSpeed};

camera.pan_tilt_absolute(
    PanPosition::from_degrees(45.0)?,
    TiltPosition::from_degrees(15.0)?,
    PanSpeed::new(18)?,
    TiltSpeed::new(18)?
).await?;
```

### 6. Timeout Configuration

Timeout configuration now supports different timeout values for different operation categories.

**Before (v0.6.x):**
```rust
// Single timeout for all operations
let config = TimeoutConfig::default();
```

**After (v0.7.0):**
```rust
use grafton_visca::timeout::{TimeoutConfig, CommandCategory};

let config = TimeoutConfig {
    ack_timeout: Duration::from_millis(75),      // ACK response timeout
    quick_timeout: Duration::from_secs(5),       // Inquiry commands
    movement_timeout: Duration::from_secs(30),   // Pan/tilt/zoom
    preset_timeout: Duration::from_secs(90),     // Preset operations
    long_timeout: Duration::from_secs(300),      // Discovery
    network_timeout: Duration::from_secs(5),     // Network commands
    default_timeout: Duration::from_secs(60),    // Fallback
};
```

## Common Migration Patterns

### Pattern 1: Basic Camera Setup

**v0.6.x:**
```rust
use grafton_visca::camera::PTZOpticsG2Cam;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let camera = PTZOpticsG2Cam::new("192.168.0.110").await?;
    camera.power_on().await?;
    camera.zoom_in().await?;
    Ok(())
}
```

**v0.7.0:**
```rust
use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let camera = CameraBuilder::new()
        .with_profile(PTZOpticsG2)
        .connect_tcp("192.168.0.110")
        .await?;
    
    camera.power_on().await?;
    camera.zoom_tele_std().await?;
    Ok(())
}
```

### Pattern 2: Custom Transport

**v0.6.x:**
```rust
let transport = TcpTransport::new("192.168.0.110:52381").await?;
let camera = Camera::new(transport);
```

**v0.7.0:**
```rust
use grafton_visca::transport::tokio::Tcp;

let transport = Tcp::connect("192.168.0.110:52381").await?;
let camera = Camera::from_transport(transport, PTZOpticsG2);
```

### Pattern 3: Concurrent Operations

**v0.6.x:**
```rust
let (power, zoom, position) = tokio::join!(
    camera.get_power_state(),
    camera.get_zoom_position(),
    camera.get_pan_tilt_position()
);
```

**v0.7.0:**
```rust
let (power, zoom, pan_tilt) = tokio::join!(
    camera.get_power_state(),
    camera.get_zoom(),
    camera.get_pan_tilt_degrees()
);
```

### Pattern 4: Error Handling

Error types remain largely the same, but some error variants have been updated.

**v0.6.x:**
```rust
match camera.power_on().await {
    Ok(()) => println!("Powered on"),
    Err(ViscaError::Timeout) => println!("Timeout"),
    Err(e) => println!("Error: {}", e),
}
```

**v0.7.0:**
```rust
match camera.power_on().await {
    Ok(()) => println!("Powered on"),
    Err(Error::Timeout { .. }) => println!("Timeout"),
    Err(e) => println!("Error: {}", e),
}
```

## Feature Flags

Feature flags have been reorganized:

| Old Flag | New Flag | Description |
|----------|----------|-------------|
| `async` | `rt-tokio` | Tokio runtime support |
| `blocking` | (default) | Blocking support is now default |
| N/A | `async` | Base async support (no runtime) |

## Troubleshooting

### Compilation Errors

1. **"method not found" errors**: Check the method name changes table above
2. **Type mismatch errors**: Ensure you're using the new position/speed types
3. **Import errors**: Update imports to use new module paths

### Runtime Issues

1. **Timeout errors**: Adjust timeout configuration for your network conditions
2. **Connection failures**: Verify DNS resolution works with your address format
3. **Command failures**: Some cameras may require specific profiles or capabilities

## Getting Help

If you encounter issues not covered in this guide:

1. Check the [examples](examples/) directory for working code
2. Review the [API documentation](https://docs.rs/grafton-visca)
3. Open an issue on [GitHub](https://github.com/GrantSparks/grafton-visca/issues)

## Benefits of Upgrading

- **Better Performance**: Zero-cost abstractions eliminate boxing overhead
- **Type Safety**: Compile-time guarantees prevent runtime errors
- **Network Support**: Improved DNS resolution and IPv6 support
- **Timeout Control**: Fine-grained timeout configuration
- **Cleaner API**: Unified camera type with mode markers

The migration effort is worth it for the significant performance improvements and better type safety.