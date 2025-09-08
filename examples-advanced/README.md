# Advanced Examples

This directory contains examples that demonstrate low-level APIs and advanced features of the grafton-visca library. These are intended for:

- Library maintainers and contributors
- Users who need direct protocol access
- Debugging and troubleshooting scenarios
- Understanding the library's internal architecture

## Examples

### runtime_demo_lowlevel.rs
Demonstrates direct usage of `RuntimeHandle` and VISCA command construction. Shows:
- Raw command sending with `send_command()`
- Direct `ViscaResponse` matching
- Priority scheduling at the runtime level
- Runtime metrics access

### type_safe_commands.rs
Shows the type-safe command system used internally by the library.

## Important Note

For typical camera control applications, use the high-level accessor API demonstrated in the main `examples/` directory. The patterns shown here are considered advanced and may change between versions as they expose internal implementation details.

## Running Advanced Examples

These examples may require additional features to be enabled:

```bash
# For runtime_demo_lowlevel.rs
cargo run --example runtime_demo_lowlevel --features async,rt-tokio
```
