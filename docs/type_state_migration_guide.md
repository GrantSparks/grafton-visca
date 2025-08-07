# Type-State Pattern Migration Guide

This guide helps developers migrate to the new type-safe VISCA command builder API introduced in v0.4.0. The type-state pattern ensures commands are properly terminated at compile time, preventing an entire class of protocol errors.

## Overview

The type-state pattern uses Rust's type system to enforce that VISCA commands are properly terminated before being sent. Commands progress through two states:

1. **Incomplete**: Commands being built (default state)
2. **Terminated**: Commands ready to send (after calling `terminate()`)

## Migration Strategies

### Strategy 1: Keep Using Legacy API (No Changes Required)

The legacy `build()` and `finalize()` methods continue to work:

```rust
// Legacy API - still works
let mut builder = CommandBuilder::<9>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
builder.push_mut(value);
let bytes = builder.finalize(); // Returns &[u8]
```

### Strategy 2: Adopt Type-Safe API (Recommended)

Migrate to the type-safe API for compile-time guarantees:

```rust
// New type-safe API
let builder = CommandBuilder::<9>::from_prefix(&[0x81, 0x01, 0x04, 0x47])
    .push(value)
    .terminate(); // Moves to Terminated state
let bytes = builder.as_bytes(); // Only available on Terminated commands
```

## Common Migration Patterns

### Pattern 1: Simple Command Building

**Before:**
```rust
let mut builder = CommandBuilder::<7>::from_prefix(&[0x81, 0x01, 0x04, 0x00]);
builder.push_mut(0x02);
let bytes = builder.finalize();
```

**After (Type-Safe):**
```rust
let builder = CommandBuilder::<7>::from_prefix(&[0x81, 0x01, 0x04, 0x00])
    .push(0x02)
    .terminate();
let bytes = builder.as_bytes();
```

### Pattern 2: Conditional Command Building

**Before:**
```rust
let mut builder = CommandBuilder::<9>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
if use_high_speed {
    builder.push_mut(0x07);
} else {
    builder.push_mut(0x03);
}
builder.push_mut(value);
let bytes = builder.finalize();
```

**After (Type-Safe):**
```rust
let speed = if use_high_speed { 0x07 } else { 0x03 };
let builder = CommandBuilder::<9>::from_prefix(&[0x81, 0x01, 0x04, 0x47])
    .push(speed)
    .push(value)
    .terminate();
let bytes = builder.as_bytes();
```

### Pattern 3: Complex Command Building

For complex logic, continue using mutable methods:

```rust
let mut builder = CommandBuilder::<16>::from_prefix(&[0x81, 0x01]);
// Complex logic with loops or conditionals
for byte in dynamic_data {
    builder.push_mut(byte);
}
// Finalize when done
let bytes = builder.finalize(); // Automatic termination
```

## API Reference

### Incomplete State Methods

Available only when `State = Incomplete`:

- `from_prefix(prefix: &[u8]) -> Self` - Create from const prefix
- `new() -> Self` - Create empty builder  
- `append(self, bytes: &[u8]) -> Self` - Chain-append bytes
- `push(self, byte: u8) -> Self` - Chain-push single byte
- `push_nibbles(self, high: u8, low: u8) -> Self` - Chain-push nibble pair
- `terminate(self) -> CommandBuilder<N, Terminated>` - Move to Terminated state

### Terminated State Methods

Available only when `State = Terminated`:

- `as_bytes(&self) -> &[u8]` - Get command bytes (includes terminator)
- `len(&self) -> usize` - Get command length
- `with_camera_id(&mut self, id: u8)` - Replace camera ID byte

### Methods Available in Both States (Mutable)

- `append_mut(&mut self, bytes: &[u8])` - Append bytes in place
- `push_mut(&mut self, byte: u8)` - Push byte in place
- `push_nibbles_mut(&mut self, high: u8, low: u8)` - Push nibbles in place
- `with_camera_id_mut(&mut self, id: u8)` - Replace camera ID in place

### Legacy Methods (Both States)

- `build() -> [u8; N]` - Get full buffer (deprecated)
- `finalize(&mut self) -> &[u8]` - Auto-terminate and return slice

## Benefits of Migration

1. **Compile-Time Safety**: Impossible to send unterminated commands
2. **Clear Intent**: Code explicitly shows when commands are complete
3. **Better Error Messages**: Compiler explains what's wrong at build time
4. **Zero Runtime Cost**: Type states compile away completely

## Gradual Migration

You can migrate incrementally:

1. Start with new commands using the type-safe API
2. Update existing commands when touching that code
3. Keep complex commands using mutable methods
4. Use `finalize()` as a bridge during migration

## Example: Full Command Implementation

```rust
use crate::command::const_encoding::{CommandBuilder, Terminated};

pub struct ZoomIn;

impl EncodeVisca for ZoomIn {
    fn encode_visca(&self) -> Result<ViscaCommand> {
        // Type-safe command building
        let builder = CommandBuilder::<6>::from_prefix(&[0x81, 0x01, 0x04, 0x07])
            .push(0x02)  // Zoom in command
            .terminate(); // Required for protocol safety
        
        Ok(ViscaCommand {
            bytes: builder.as_bytes().to_vec(),
            response_type: ResponseType::Ack,
        })
    }
}
```

## Troubleshooting

### Error: "method `as_bytes` not found"

The command hasn't been terminated. Add `.terminate()` before calling `as_bytes()`.

### Error: "cannot move out of borrowed content"

You're mixing ownership and mutable APIs. Either:
- Use chain methods: `.push(x).push(y).terminate()`
- Use mutable methods: `push_mut(x); push_mut(y); finalize()`

### Need to build commands dynamically?

Use mutable methods for complex logic, then call `finalize()`:

```rust
let mut builder = CommandBuilder::<16>::new();
// Dynamic building logic
builder.finalize() // Automatically terminates
```

## Further Reading

- [VISCA Protocol Safety Issue (#203)](https://github.com/GraftonMachine/grafton-visca/issues/203)
- [Type-State Pattern in Rust](https://docs.rs/typestate/)
- [API Documentation](https://docs.rs/grafton-visca/latest/grafton_visca/)