# Split Module Pattern Guide

This guide describes the split module pattern used throughout the grafton-visca codebase to manage conditional compilation cleanly.

## When to Use the Split Module Pattern

Use this pattern when you have:
- Multiple functions with different implementations for async vs blocking
- Substantial conditional compilation within a single module
- Complex structs or enums that vary significantly between feature configurations

## The Pattern

Instead of interleaving `#[cfg]` attributes throughout a single file:

```rust
// ❌ Don't do this
pub struct Manager {
    #[cfg(feature = "async")]
    sender: tokio::sync::mpsc::UnboundedSender<Command>,
    #[cfg(not(feature = "async"))]
    sender: std::sync::mpsc::Sender<Command>,
}

impl Manager {
    #[cfg(feature = "async")]
    pub async fn send(&self, cmd: Command) -> Result<(), Error> {
        // async implementation
    }
    
    #[cfg(not(feature = "async"))]
    pub fn send(&self, cmd: Command) -> Result<(), Error> {
        // blocking implementation
    }
}
```

Split the implementations into separate files:

```
src/manager/
├── mod.rs       # Re-exports and shared types
├── tokio.rs     # Async implementation
└── blocking.rs  # Blocking implementation
```

### mod.rs - The Facade

```rust
// Shared types and constants
pub(crate) struct Command { /* ... */ }

// Feature-based re-exports
#[cfg(feature = "async")]
mod tokio;
#[cfg(feature = "async")]
pub use tokio::*;

#[cfg(not(feature = "async"))]
mod blocking;
#[cfg(not(feature = "async"))]
pub use blocking::*;
```

### tokio.rs - Async Implementation

```rust
use super::Command;

pub struct Manager {
    sender: tokio::sync::mpsc::UnboundedSender<Command>,
}

impl Manager {
    pub async fn send(&self, cmd: Command) -> Result<(), Error> {
        // async implementation
    }
}
```

### blocking.rs - Blocking Implementation

```rust
use super::Command;

pub struct Manager {
    sender: std::sync::mpsc::Sender<Command>,
}

impl Manager {
    pub fn send(&self, cmd: Command) -> Result<(), Error> {
        // blocking implementation
    }
}
```

## Benefits

1. **Compilation Clarity**: Each file compiles entirely or not at all
2. **Easier Reviews**: Reviewers can focus on one implementation at a time
3. **Better IDE Support**: No confusing inactive code regions
4. **Reduced Cognitive Load**: No mental context switching between feature configurations

## Examples in the Codebase

- `src/socket_manager/` - Split between tokio.rs and mod.rs (blocking removed)
- `src/channels/` - Split between tokio.rs and mod.rs (blocking removed)
- `src/executor/` - Split between async_impl.rs and blocking.rs
- `src/camera/methods/reexports/` - Split between async_exports.rs and blocking_exports.rs
- `src/camera/generic/` - Split between async_impl.rs and blocking.rs

## When NOT to Use This Pattern

Don't use the split pattern for:
- Simple conditional imports
- Single-line feature differences
- Enum variants that need to exist conditionally (e.g., TransportKind)
- Struct fields that vary by feature (unless the entire struct differs significantly)

## Guidelines

1. **Rule C1**: No function body, impl block, or enum/struct definition should contain more than one `#[cfg(...)]` gate
2. **Naming Convention**: Use `tokio.rs`/`blocking.rs` for runtime-specific code, or `async_impl.rs`/`blocking.rs` for async/sync splits
3. **Public API**: Ensure both implementations expose identical public APIs
4. **Shared Code**: Put genuinely shared types and constants in mod.rs
5. **Documentation**: Document the public API in mod.rs, not in the split files

## Enforcing the Pattern

The codebase uses `#![deny(unreachable_code, dead_code)]` to ensure no dead code paths remain. Future work includes adding a custom lint to enforce rule C1 automatically.

## Migration Checklist

When refactoring existing code to use the split pattern:

- [ ] Create the module directory
- [ ] Create mod.rs with shared types and re-exports
- [ ] Create feature-specific implementation files
- [ ] Move code to appropriate files, removing `#[cfg]` attributes
- [ ] Verify both configurations compile: `cargo check --no-default-features` and `cargo check --features tokio`
- [ ] Run tests in both modes
- [ ] Update imports in other modules
- [ ] Remove the original file