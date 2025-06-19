# Blocking-First Architecture Migration Guide

This document describes the transport layer redesign from async-first to blocking-first architecture in grafton-visca.

## Overview

The transport layer has been redesigned to provide a blocking-first API, with async support as an optional layer. This provides better ergonomics for the primary use case (synchronous camera control) while still supporting async when needed.

## Key Changes

### 1. Blocking Transport is Primary

The core transport trait is now blocking by default:

```rust
// Old (async-first)
pub trait RawTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()>;
    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>>;
}

// New (blocking-first)  
pub trait Transport: Send + Sync {
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error>;
    fn is_connected(&self) -> bool;
    fn description(&self) -> &str;
}
```

### 2. No Async Runtime Required for Blocking

Blocking examples now work without any async runtime:

```rust
// Old approach - required tokio even for blocking
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

// New approach - direct blocking calls
use grafton_visca::{Camera, transport::blocking::create};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = create::tcp("192.168.1.100:5678")?;
    let mut camera = Camera::new(transport);
    
    camera.power_on()?;  // Direct call, no block_on needed
    camera.home()?;
    
    Ok(())
}
```

### 3. Separate Feature Flags

- `blocking-client`: Enables blocking transport and camera API (default)
- `async-client`: Enables async transport and camera API (optional)

### 4. Camera API Changes

The Camera struct now properly supports both modes:

```rust
// Blocking mode (mutable reference required)
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
impl<P: CameraProfile> Camera<P> {
    pub fn power_on(&mut self) -> Result<(), Error> { ... }
    pub fn home(&mut self) -> Result<(), Error> { ... }
}

// Async mode (shared reference with internal locking)
#[cfg(feature = "async-client")]
impl<P: CameraProfile> Camera<P> {
    pub async fn power_on(&self) -> Result<(), Error> { ... }
    pub async fn home(&self) -> Result<(), Error> { ... }
}
```

## Migration Steps

### For Blocking Users

1. Remove any `block_on` or async runtime usage
2. Use `transport::blocking::create` for transport creation
3. Camera methods now take `&mut self` instead of `&self`
4. Remove `async`/`await` from your code

Before:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = block_on(create::tcp(&camera_ip))?;
    let camera = Camera::new(transport);
    
    block_on(camera.power_on())?;
    block_on(camera.home())?;
    
    Ok(())
}
```

After:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = blocking::create::tcp(&camera_ip)?;
    let mut camera = Camera::new(transport);
    
    camera.power_on()?;
    camera.home()?;
    
    Ok(())
}
```

### For Async Users

No changes required if you're already using async. The async API remains the same:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = create::tcp(&camera_ip).await?;
    let camera = Camera::new(transport);
    
    camera.power_on().await?;
    camera.home().await?;
    
    Ok(())
}
```

### For Examples

Examples can now support both modes cleanly:

```rust
// Works with just blocking-client feature
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::transport::blocking;
    
    let transport = blocking::create::tcp("192.168.1.100:5678")?;
    let mut camera = Camera::new(transport);
    camera.home()?;
    Ok(())
}

// Works with async-client feature
#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::transport;
    
    let transport = transport::create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::new(transport);
    camera.home().await?;
    Ok(())
}
```

## Benefits

1. **Simpler Implementation**: No futures, pinning, or async complexity for basic use
2. **Better Performance**: Direct system calls without runtime overhead
3. **Smaller Binary Size**: ~500KB smaller without tokio runtime
4. **Cleaner API**: Blocking users get a natural, ergonomic API
5. **Flexibility**: Async users can still use async via the optional feature

## Compatibility Notes

- The default feature is now `blocking-client` instead of requiring both features
- Examples that previously required both features may now work with just `blocking-client`
- The async API is unchanged for backward compatibility
- Custom transports need to implement the blocking `Transport` trait

## Future Considerations

The blocking-first approach aligns with the primary use case of VISCA camera control, where commands are inherently sequential and response times are fast. This design provides the best ergonomics for most users while maintaining flexibility for advanced async use cases.