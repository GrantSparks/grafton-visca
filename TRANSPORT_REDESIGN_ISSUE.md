# Transport API Redesign: From Async-First to Blocking-First

## Current State (Problems)

The transport API redesign that was started has created several issues:

### 1. **Everything is Feature-Gated Behind `async-client`**
- All transport types (`TcpTransport`, `UdpTransport`, `SerialTransport`) require `async-client` feature
- The `RawTransport` trait itself requires `async-client`
- This makes blocking usage impossible without pulling in async dependencies

### 2. **Breaking API Changes Without Migration Path**
- Removed `BlockingAdapter` and `BlockingTransport` traits
- Removed the `Transport` trait entirely
- Many examples and tests still reference these removed types:
  - `BlockingAdapter` - referenced in 15+ files
  - `Transport` trait - referenced in tests and examples
  - `AsyncTcpTransport`, `AsyncUdpTransport` - type aliases that no longer exist

### 3. **Inconsistent Feature Gating**
- Camera struct has `transport` field only with `async-client`
- But has methods expecting transport without proper feature gates
- `send_raw` method was removed from Camera but trait implementations still try to call it
- Circular dependency: `CameraExtension::send_raw` calls `Camera::send_raw` which doesn't exist

### 4. **Examples Are Broken**
Most examples fail to compile due to:
- Trying to use removed `BlockingAdapter`
- Expecting `Transport::new()` to take a string (it now takes a raw transport)
- Using old type aliases like `AsyncTcpTransport`
- Assuming blocking API exists

### 5. **Tests Are Broken**
- Common test utilities expect `BlockingTransport` trait
- Mock implementations use old trait structure
- Tests assume both blocking and async APIs exist

## Root Cause

The fundamental issue is that the redesign went **async-first** when the library needs to be **blocking-first**:

1. **Serial ports** are inherently blocking - async wrappers add complexity
2. **Many use cases** don't need async - forcing tokio dependency is heavy
3. **Blocking can support async** via runtime, but async can't easily support blocking

## Proposed Solution: Blocking-First Design

### Core Principles

1. **Blocking is the default** - All transports must implement blocking I/O
2. **Async is optional** - Added via feature flag and extension trait
3. **Simple traits** - Easy to implement new transports
4. **Zero overhead** - No runtime required for blocking usage

### New Transport Trait Structure

```rust
// Core blocking transport - everyone implements this
pub trait Transport: Send + Sync + Debug {
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    fn receive(&mut self) -> Result<Vec<u8>, Error>;
    fn is_connected(&self) -> bool;
    fn description(&self) -> &str;
}

// Optional async extension
#[cfg(feature = "async")]
pub trait AsyncTransport: Transport {
    fn send_async<'a>(&'a mut self, data: &'a [u8]) 
        -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    
    fn receive_async<'a>(&'a mut self)
        -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
}
```

### ViscaTransport Works with Both

```rust
// Always available - blocking
impl<T: Transport> ViscaTransport<T> {
    pub fn send_command(&mut self, cmd: &dyn Command) -> Result<Response, Error> {
        // VISCA protocol using blocking transport
    }
}

// Only with async feature
#[cfg(feature = "async")]
impl<T: AsyncTransport> ViscaTransport<T> {
    pub async fn send_command_async(&mut self, cmd: &dyn Command) -> Result<Response, Error> {
        // VISCA protocol using async transport
    }
}
```

## Migration Steps

### Phase 1: Fix Core Transport Module
1. Remove `#[cfg(feature = "async-client")]` from basic transport traits
2. Create blocking `Transport` trait
3. Create optional `AsyncTransport: Transport` trait
4. Update `ViscaTransport` to work with blocking by default

### Phase 2: Update Transport Implementations
1. Implement blocking methods for all transports
2. Use `std::net::TcpStream` for blocking TCP
3. Use `std::net::UdpSocket` for blocking UDP  
4. Add async support as optional extension

### Phase 3: Fix Camera Integration
1. Remove feature gates from Camera's transport field
2. Restore `send_raw` method (blocking)
3. Add `send_raw_async` with proper feature gate
4. Fix `CameraExtension` trait to not have circular calls

### Phase 4: Update Examples
1. Create clear blocking examples (no runtime needed)
2. Create async examples with `#[cfg(feature = "async")]`
3. Update common test utilities
4. Fix all compilation errors

### Phase 5: Update Tests
1. Restore `MockTransport` with blocking implementation
2. Add `MockAsyncTransport` for async tests
3. Fix test infrastructure to support both modes

## Benefits of This Approach

1. **Simpler for users** - Basic usage doesn't require async runtime
2. **Lighter dependencies** - Tokio only needed for async feature
3. **Better for embedded** - No runtime overhead for serial/basic usage
4. **Cleaner code** - Less feature gating, clearer separation
5. **Easier to maintain** - Blocking is foundation, async is extension

## Example Usage After Redesign

```rust
// Simple blocking usage - no runtime needed!
use grafton_visca::{transport::SerialTransport, ViscaTransport};

let transport = SerialTransport::new("/dev/ttyUSB0")?;
let mut camera = ViscaTransport::new(transport);
let response = camera.send_command(&PowerOn)?;

// Async usage - opt-in with feature
#[cfg(feature = "async")]
{
    use grafton_visca::transport::TcpTransport;
    
    let transport = TcpTransport::connect_async("192.168.1.100:5678").await?;
    let mut camera = ViscaTransport::new(transport);
    let response = camera.send_command_async(&PowerOn).await?;
}
```

## Current Blockers

To implement this redesign, we need to:

1. **Agree on the blocking-first approach**
2. **Decide on feature flag names** (`async`? `tokio`? `async-runtime`?)
3. **Plan backward compatibility** (or accept breaking changes)
4. **Coordinate the changes** (it touches many files)

The key insight is that **blocking should be the foundation**, with async as an optional layer on top. This matches Rust's standard library design (std::io::Read/Write are blocking, with async alternatives in external crates).