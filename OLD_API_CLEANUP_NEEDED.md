# Old API Patterns Still Referenced in Code

## Summary of Findings

After searching the codebase, I found numerous references to old API patterns that were removed during the transport redesign but are still being used in examples and tests:

### 1. **BlockingAdapter** - Referenced in 19 files
- Used in examples like `unified_client_demo.rs`, `thread_safe_client.rs`, etc.
- Pattern: `Camera::new(BlockingAdapter(transport))`
- No longer exists in the codebase

### 2. **BlockingTransport trait** - Referenced in 6 files
- Used in test infrastructure (`tests/common/mod.rs`, `tests/transport_tests.rs`)
- Pattern: `impl BlockingTransport for MockTransport`
- Has been replaced with `blocking::Transport`

### 3. **AsyncTcpTransport** - Referenced in 14 files
- Used in async examples
- Pattern: `AsyncTcpTransport::new("192.168.1.100:5678")`
- Expected to be imported from `common::r#async::AsyncTcpTransport`
- But not actually defined anywhere

### 4. **AsyncUdpTransport** - Referenced in several async examples
- Similar issue as AsyncTcpTransport
- Not defined in the codebase

## Files That Need Updates

### Examples with BlockingAdapter references:
- examples/unified_client_demo.rs
- examples/thread_safe_client.rs
- examples/inquiry_demo.rs
- examples/white_balance_tuning_demo.rs
- examples/user_friendly_api_demo.rs
- examples/control_demo.rs
- examples/configurable_timeouts.rs
- examples/capability_query_demo.rs
- examples/image_and_gain_demo.rs
- examples/ptz_builder_demo.rs
- examples/test_extension_traits.rs
- examples/type_safe_api_demo.rs
- examples/enhanced_api_demo.rs
- examples/error_handling_demo.rs

### Examples with AsyncTcpTransport/AsyncUdpTransport references:
- examples/unified_client_demo.rs
- examples/channel_transport_demo.rs
- examples/async_health_check.rs
- examples/custom_profile_builder_demo.rs
- examples/command_sequences_demo.rs
- examples/async_inquiry_demo.rs
- examples/async_configurable_timeouts.rs
- examples/async_concurrent.rs
- examples/camera_readiness_demo.rs
- examples/multi_camera_control_demo.rs
- examples/new_features_demo.rs
- examples/ptz_builder_demo.rs
- examples/custom_retry_logic.rs
- examples/camera_profile_demo.rs

### Test infrastructure with old patterns:
- tests/transport_tests.rs
- tests/common/mod.rs
- tests/camera_concurrency_tests.rs

## Current State vs Expected State

### Current (Broken) Pattern in Examples:
```rust
use common::r#async::AsyncTcpTransport;
let transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
let camera = Camera::new(BlockingAdapter(transport));
```

### What Should Be Used Instead:

For blocking:
```rust
use grafton_visca::transport::blocking::{TcpTransport, ViscaTransport};
let transport = TcpTransport::connect("192.168.1.100:5678")?;
let visca = ViscaTransport::new(transport);
```

For async:
```rust
use grafton_visca::transport::{TcpTransport, ViscaTransport};
let transport = TcpTransport::connect("192.168.1.100:5678").await?;
let visca = ViscaTransport::new(transport);
```

## The Root Problem

The examples are trying to use type aliases (`AsyncTcpTransport`, `AsyncUdpTransport`) that were probably meant to be defined in `examples/common/mod.rs` but were never created. Additionally, they're using `BlockingAdapter` which has been completely removed from the codebase.

The transport redesign introduced a new API structure but didn't update the examples and tests to use it properly.