# Feature Gate Architecture

This document explains the feature gating strategy used in grafton-visca to achieve true runtime agnosticism and zero-overhead abstractions.

## Design Principles

1. **Zero Overhead**: When `async` features are disabled, there are NO async dependencies or runtime code in the binary
2. **Runtime Agnostic**: The `async` feature provides runtime-agnostic async support - works with ANY async runtime
3. **Progressive Enhancement**: Features build on each other in a clear hierarchy

## Feature Hierarchy

```
[no features] → Pure blocking API, zero async dependencies
     ↓
  [async] → Runtime-agnostic async traits and implementations
     ↓
[rt-tokio] → Tokio-specific transport implementations (implies async)
```

## Key Implementation Patterns

### 1. Import Gating

```rust
// Async-only imports
#[cfg(feature = "async")]
use crate::transport::AsyncTransport;

// Tokio-specific imports
#[cfg(feature = "rt-tokio")]
use tokio::net::TcpStream;
```

### 2. Type Definitions

```rust
// Runtime handle only exists with async
#[cfg(feature = "async")]
pub struct RuntimeHandle { ... }

// Tokio-specific transports
#[cfg(feature = "rt-tokio")]
pub struct AsyncRawTcpTransport { ... }
```

### 3. Method Implementations

```rust
impl Camera {
    // Available without any features
    pub fn blocking_method(&self) { ... }
    
    // Available with async feature (any runtime)
    #[cfg(feature = "async")]
    pub async fn async_method(&self) { ... }
    
    // Only with tokio runtime
    #[cfg(feature = "rt-tokio")]
    pub async fn tokio_specific_method(&self) { ... }
}
```

### 4. Runtime Loop Pattern

The runtime module demonstrates how to support both tokio and generic async:

```rust
#[cfg(feature = "async")]
async fn runtime_loop() {
    loop {
        #[cfg(feature = "rt-tokio")]
        {
            // Tokio-optimized implementation using tokio::select!
            tokio::select! { ... }
        }
        
        #[cfg(not(feature = "rt-tokio"))]
        {
            // Runtime-agnostic implementation
            // Uses polling and manual async coordination
        }
    }
}
```

## Module-Specific Notes

### `src/runtime/mod.rs`
- Core `RuntimeHandle` requires `async` feature
- Helper constructors like `new_tcp_raw()` require `rt-tokio` because they use tokio-specific transports
- Runtime loop has dual implementation: optimized for tokio, fallback for other runtimes

### `src/transport/`
- Blocking transports always available
- `AsyncTransport` trait requires `async` feature
- Concrete async implementations (e.g., `AsyncRawTcpTransport`) require `rt-tokio`

### `src/camera/`
- `Camera<BlockingMode, ...>` always available
- `Camera<AsyncMode, ...>` requires `async` feature
- Methods properly gated based on mode and features

## Testing Feature Combinations

Always test all three primary configurations:

```bash
# Pure blocking (no async code)
cargo check --no-default-features

# Runtime-agnostic async
cargo check --no-default-features --features async

# Tokio-specific
cargo check --no-default-features --features rt-tokio
```

## Common Pitfalls

1. **Don't leak runtime-specific code into `async`-only sections**
   - Wrong: Using `tokio::select!` when only `async` is enabled
   - Right: Gate tokio-specific code with `rt-tokio`

2. **Always gate imports appropriately**
   - Async types need `#[cfg(feature = "async")]`
   - Runtime-specific types need their runtime feature

3. **Maintain the feature hierarchy**
   - `rt-tokio` implies `async`, don't use `#[cfg(all(feature = "async", feature = "rt-tokio"))]` unless necessary
   - Use the most specific gate needed

4. **Handle all feature combinations in shared code**
   - If code is used by both async and blocking, ensure it compiles in both modes
   - Use conditional compilation to provide appropriate implementations