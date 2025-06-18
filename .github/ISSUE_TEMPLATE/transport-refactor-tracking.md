---
name: Transport Layer Refactor Tracking
about: Main tracking issue for the transport layer architecture overhaul
title: "[EPIC] Transport Layer Refactor - Lean, Idiomatic, Composable API"
labels: epic, refactor, breaking-change, architecture
assignees: ''
---

## 🎯 Overview

This epic tracks the complete overhaul of grafton-visca's transport layer to create a lean, idiomatic, and composable API.

### Current Issues
- **Trait sprawl**: `Transport`, `BlockingTransport`, and `UnifiedTransport` overlap and complicate the public API
- **Embedded resiliency**: ~1,500 LOC of retry/reconnect logic bloats the crate and blurs concerns
- **Protocol leakage**: VISCA socket IDs are passed as transport parameters instead of being handled at the session layer
- **Bundled implementations**: Concrete TCP/UDP types live inside the crate instead of being optional examples

### Goals
1. **Focus on raw byte exchange** - nothing more
2. **Push resiliency & connection policy to the caller**
3. **Keep sync and async support trivial**
4. **Move protocol details back to the Session layer**
5. **Turn concrete network code into examples/demo crates**

## 📋 Implementation Tasks

### Phase 1: Core Refactoring
- [ ] **A. Define new traits** (#task-a)
  - [ ] Create minimal `Transport` trait for sync
  - [ ] Create `AsyncTransport` trait behind feature flag
  - [ ] Remove old trait hierarchy
  - [ ] Add blanket impls for `Read + Write` / `AsyncRead + AsyncWrite`

- [ ] **B. Delete resiliency code** (#task-b)
  - [ ] Remove `src/transport/resilient.rs`
  - [ ] Remove `src/reconnecting_transport.rs`
  - [ ] Clean up references in `lib.rs`, `Cargo.toml`, tests, docs

- [ ] **C. Refactor Session** (#task-c)
  - [ ] Accept `&mut dyn Transport` / `&mut dyn AsyncTransport`
  - [ ] Manage VISCA socket IDs (0-7) internally
  - [ ] Handle packet assembly/disassembly
  - [ ] Update error types

- [ ] **D. Refactor Camera client** (#task-d)
  - [ ] Make it generic over `T: Transport`
  - [ ] Add async variant under feature flag
  - [ ] Update all dependent types

### Phase 2: Reorganization
- [ ] **E. Extract network implementations** (#task-e)
  - [ ] Move TCP impl to `examples/tcp_transport.rs`
  - [ ] Move UDP impl to `examples/udp_transport.rs`
  - [ ] Create serial port example
  - [ ] Ensure examples compile with `--examples`

- [ ] **F. Update documentation** (#task-f)
  - [ ] Update README with new architecture
  - [ ] Document transport trait and rationale
  - [ ] Add retry wrapper example
  - [ ] Remove stale resiliency references

### Phase 3: Quality & Release
- [ ] **G. Testing** (#task-g)
  - [ ] Update unit tests for new API
  - [ ] Add mock transport for testing
  - [ ] Ensure CI passes all checks
  - [ ] Test all feature combinations

- [ ] **H. Feature audit** (#task-h)
  - [ ] Keep `async`, `tokio`, `serialport` as opt-in
  - [ ] Remove resilient/reconnect features
  - [ ] Update feature documentation

- [ ] **I. Release preparation** (#task-i)
  - [ ] Update CHANGELOG.md
  - [ ] Bump version to 0.x.0 (breaking)
  - [ ] Create migration guide

## 🎨 Target Design

```rust
/// Minimal synchronous transport for VISCA byte frames.
pub trait Transport: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Send request bytes and return the full response bytes.
    fn exchange(&mut self, req: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

#[cfg(feature = "async")]
pub trait AsyncTransport: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn exchange(&mut self, req: &[u8]) -> Result<Vec<u8>, Self::Error>;
}
```

## ✅ Acceptance Criteria
- [ ] `cargo test` passes on stable 1.78+
- [ ] `cargo test --all-features` passes
- [ ] `cargo doc --no-deps` succeeds
- [ ] No `unsafe` code added
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] Examples demonstrate TCP, UDP, serial, and retry wrapper
- [ ] README accurately reflects new API
- [ ] Public items have comprehensive rustdoc

## 📊 Metrics
- Lines of code removed: ~1,500
- Trait complexity: 3 traits → 2 traits
- API surface reduction: TBD
- Compile time improvement: TBD

## 🔗 Related Issues
<!-- Add links to individual task issues as they are created -->

## 💬 Discussion
<!-- Use this section for design decisions and alternatives considered -->

### Design Rationale
- **Single-responsibility**: Transport = "move bytes"; Session/Client = "speak VISCA"; App = "ensure reliability"
- **Idiomatic Rust**: Simple, composable traits over deep hierarchies
- **Extensibility**: Users can layer retries, logging, throttling without patching the crate
- **Lower maintenance**: Fewer edge cases, faster compile times, easier to audit

### Breaking Changes
This is a complete API overhaul. Users will need to:
1. Update transport initialization code
2. Move retry logic to application layer
3. Update error handling
4. Potentially implement custom transports

---
*This is a living document. Updates will be made as implementation progresses.*