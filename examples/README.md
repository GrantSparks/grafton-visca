# grafton-visca Examples

This directory contains working examples for the current `grafton-visca` API surface.

Preferred usage:

- Use `Connect` for simple blocking and async camera connections.
- Use `CameraConfig` when a standard TCP, UDP, or serial connection needs explicit timeouts, retry policy, keepalive, or camera ID.
- Use `Session::open` when you already own a custom transport and need to attach it to a camera.
- Import construction/session types from their public modules (`Session`, `Connect`, and `CameraConfig`).
- Use checked public value types such as `UnitInterval::new(...)` and `CameraId`/`try_camera_id(...)` instead of raw normalized floats or raw camera ID setters.
- Treat raw transport, protocol, and lab-validation programs as advanced integration/reference material.
- Use ordinary noun methods for simple command completion. Use operation handles
  when a command needs an exact deadline, profile-selected protocol-settlement
  wait, cancellation, or explicit detach.

Example quality bar:

- A runnable example should either connect to a camera, validate public API types, or be clearly marked as reference material.
- Examples that move hardware should take the camera address from an argument or environment variable and should keep movement focused.
- Hard-coded lab IPs belong only in validation utilities, not in getting-started examples.

See [docs/examples.md](../docs/examples.md) for the example maintenance policy.

## Important: Feature Flags

This library uses feature flags to control dependencies:
- **`blocking`**: Blocking API
- **`async`**: Runtime-agnostic async API; provide your own executor/runtime integration
- **`runtime-tokio`**: Built-in Tokio runtime support (implies `async`)
- **`runtime-smol`**: Built-in smol runtime support (implies `async`)
- **`transport-serial`**: Blocking serial (RS-232/RS-422)
- **`transport-serial-tokio`**: Tokio serial transport (implies `runtime-tokio`)
- **`test-utils`**: Deterministic test transports and executors

## Getting Started

If you're new to the library, start with these examples in order:

1. **[quickstart.rs](quickstart.rs)** - Blocking connection and read-only state query
2. **[inquiry_quickstart.rs](inquiry_quickstart.rs)** - Blocking inquiry flow with the current accessor API
3. **[quickstart_async.rs](quickstart_async.rs)** - Tokio async connection and read-only state query
4. **[operation_handles.rs](operation_handles.rs)** - Blocking applied/settled waits, detach, session close on every path, and a scoped stop-on-exit guard
5. **[operation_handles_async.rs](operation_handles_async.rs)** - Tokio waits, detach, session close on every path, and the async form of a bounded drive
6. **[type_safe_commands.rs](type_safe_commands.rs)** - Profile metadata, validation, and compile-time capability bounds
7. **[transport_builder_demo.rs](transport_builder_demo.rs)** - Configured connection setup with `CameraConfig`

## Examples by Category

### Basic Usage
- **[quickstart.rs](quickstart.rs)** - Blocking high-level API; read-only unless `--move` is passed
- **[quickstart_async.rs](quickstart_async.rs)** - Async version for Tokio
- **[inquiry_quickstart.rs](inquiry_quickstart.rs)** - High-level inquiry accessors and typed responses
- **[preset_demo.rs](preset_demo.rs)** - Single preset set, recall, or clear operation
- **[operation_handles.rs](operation_handles.rs)** - Blocking applied/settled waits, explicit detach, and a session closed on every path
- **[operation_handles_async.rs](operation_handles_async.rs)** - Tokio applied/settled waits, explicit detach, and a session closed on every path
- **[motion_safety.rs](motion_safety.rs)** - Blocking `motion()` view: `is_moving`, `is_moving_axes`, `wait_until_idle`, and the `stop_all_motion` composite
- **[type_safe_commands.rs](type_safe_commands.rs)** - Compile-time profile and capability safety (no camera required)

### Connection, Transport, and Configuration
- **[transports.rs](transports.rs)** - Check PTZOptics G2 TCP and UDP connectivity without changing camera state
- **[transport_builder_demo.rs](transport_builder_demo.rs)** - Current `CameraConfig` transport policy setup
- **[builder_api.rs](builder_api.rs)** - Attach a caller-owned transport with `Session::open`
- **[sony_encapsulation.rs](sony_encapsulation.rs)** - Sony encapsulated protocol with 8-byte header (advanced)
- **[serial_async_demo.rs](serial_async_demo.rs)** - Tokio serial `CameraConfig`, checked camera ID, read-only inquiries, and explicit session close

### Advanced Patterns
- **[runtime_agnostic.rs](runtime_agnostic.rs)** - Runnable custom `Executor` adapter exercising spawn, sleep, and timeout behavior
- **[runtime_demo.rs](runtime_demo.rs)** - Tokio runtime setup with concurrent read-only inquiries
- **[concurrent_control.rs](concurrent_control.rs)** - Concurrent async inquiries, safely ordered movement, and producer-consumer commands
- **[error_handling.rs](error_handling.rs)** - Error classification with propagated connection and inquiry failures
- **[cancellation.rs](cancellation.rs)** - Tokio `.cancel()`: the supported `Cancellation`/`outcome` path and the G2 unsupported-post-send `NotSupported`/`CancelRejected` recovery
- **[custom_request.rs](custom_request.rs)** - A custom runtime `ProfileSpec` driven through blocking `camera_dyn`, including downstream `PlainCommand` and `OperationCommand` submission (requires `dyn-api`)
- **[dyn_quickstart.rs](dyn_quickstart.rs)** - Profile-erased `DynSessionCamera` with a `DynTargetedOperation` and a `DynAppliedOperation` (requires `dyn-api`)
- **[multi_camera.rs](multi_camera.rs)** - One session with two registered targets selected by `camera_for`, using explicit-timeout waits
- **[recovery.rs](recovery.rs)** - Fresh-session recovery after transport death or an application-owned silent-peer threshold: `received_frames` heartbeat evidence, re-callable transport factory, reused config, and re-query

### Validation and Reference
- **[typed_inquiry_demo.rs](typed_inquiry_demo.rs)** - Typed inquiry API walkthrough
- **[validate_inquiries.rs](validate_inquiries.rs)** - Lab inquiry validation tool with hard-coded defaults
- **[validate_ae_commands.rs](validate_ae_commands.rs)** - Lab auto-exposure validation tool with hard-coded defaults

## Running Examples

### Prerequisites

1. Ensure you have a VISCA-compatible camera connected to your network
2. Pass the camera address as documented by the example, or set its documented environment variable (default: `192.168.0.110`)
3. Verify the port number; defaults vary by camera model:
   - PTZOptics cameras: TCP port `5678`, UDP port `1259`
   - Sony professional profiles: UDP port `52381`; TCP is not a supported standard construction path
   - The selected profile will provide the default port when you omit it

Most user-facing examples accept a camera address as the first positional argument. Most also read `VISCA_CAMERA_ADDR`; `builder_api` uses `VISCA_CAMERA_UDP_ADDR`. Check the example header for the exact input.

### Basic Execution

Run blocking examples:
```bash
cargo run --example quickstart -- 192.168.0.110
cargo run --example quickstart -- 192.168.0.110 --move
cargo run --example inquiry_quickstart -- 192.168.0.110
cargo run --example operation_handles -- 192.168.0.110
cargo run --example motion_safety -- 192.168.0.110
cargo run --example custom_request --features dyn-api                       # encoding + ProfileSpec only; no camera needed
cargo run --example custom_request --features dyn-api -- 192.168.0.110:5678 # also submits through the runtime profile
cargo run --example preset_demo -- 192.168.0.110 recall 1
cargo run --example transports -- 192.168.0.110
cargo run --example builder_api -- 192.168.0.110:1259
cargo run --example type_safe_commands
cargo run --example multi_camera                   # in-memory serial bus; no camera needed
cargo run --example recovery                       # in-memory transport; no camera needed
cargo run --example transport_builder_demo -- 192.168.0.110
cargo run --example transport_builder_demo -- 192.168.0.110 --udp
```

Run async examples:
```bash
cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110
cargo run --example operation_handles_async --features runtime-tokio -- 192.168.0.110
cargo run --example concurrent_control --features runtime-tokio -- 192.168.0.110
cargo run --example error_handling --features runtime-tokio -- 192.168.0.110
cargo run --example cancellation --features runtime-tokio -- 192.168.0.110
cargo run --example dyn_quickstart --features runtime-tokio,dyn-api -- 192.168.0.110
cargo run --example runtime_demo --features runtime-tokio -- 192.168.0.110
cargo run --example sony_encapsulation --features runtime-tokio -- 192.168.0.110
cargo run --example serial_async_demo --features runtime-tokio,transport-serial-tokio -- /dev/ttyUSB0 1

# Runtime-agnostic async example
cargo run --example runtime_agnostic --features async
```

Run lab validation tools only after editing their camera lists or confirming the checked-in defaults match your test bench:
```bash
cargo run --example validate_inquiries
cargo run --example validate_ae_commands -- --apply
```

### With Logging

Enable debug logging to see VISCA commands and responses:
```bash
RUST_LOG=debug cargo run --example quickstart -- 192.168.0.110
RUST_LOG=grafton_visca=debug cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110
```

## Camera Profiles

The examples use different camera profiles to demonstrate compile-time type safety:

- `PtzOpticsG2` - PTZOptics Generation 2 cameras (most examples)
- `PtzOpticsG3` - PTZOptics Generation 3 cameras with enhanced features
- `PtzOptics30X` - PTZOptics 30X optical zoom models
- `SonyFR7` - Sony FR7 cameras with ND filter and advanced imaging
- `SonyBRCH900` - Sony BRC-H900 professional cameras
- `SonyBRC300` - Sony BRC-300 standard PTZ cameras
- `GenericVisca` - Basic VISCA profile for unknown cameras

Profiles provide compile-time protocol selection, typed capability gates, and runtime capability metadata. Capability-gated typed commands do not compile for unsupported profiles; baseline VISCA commands can still fail at runtime when hardware or firmware differs from the selected profile. Optional vendor-specific typed controls are intentionally narrower than runtime metadata: `SonyFR7` exposes ND filter and variable speed controls; built-in PTZOptics profiles are not marked for typed Motion Sync from the current model capability specs.

Profile capability contributions should follow the [Camera Profile Support Guide](../docs/camera_profile_support.md) and the [VISCA Protocol Reference](../docs/visca_reference.md), which define how protocol evidence, metadata traits, and typed support markers fit together.

## Current API Shape

```rust
use grafton_visca::camera::CameraConfig;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::runtime::TokioRuntime;
use grafton_visca::transport::{TcpKeepaliveConfig, TransportConfig};

let blocking_session = grafton_visca::blocking::Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
let blocking_camera = blocking_session.camera::<PtzOpticsG2>()?;
let is_on = blocking_camera.power().state()?;
blocking_session.close()?;

let runtime = TokioRuntime::from_current()?;
let async_session = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
    .transport_config(TransportConfig {
        tcp_keepalive: Some(TcpKeepaliveConfig::default()),
        ..TransportConfig::default()
    })
    .open_async(runtime)
    .await?;
let async_camera = async_session.camera::<PtzOpticsG2>()?;
let is_on = async_camera.power().state().await?;
async_session.close().await?;
```

## Troubleshooting

### Connection Issues
- Verify camera IP address and port
- Check network connectivity: `ping <camera-ip>`
- Ensure camera supports VISCA over IP
- Use a transport declared by the selected profile; `transports.rs` can compare both defaults for `PtzOpticsG2`

### Command Failures
- Enable debug logging: `RUST_LOG=debug`
- Check camera is powered on
- Verify camera profile matches your hardware
- Some commands require specific camera modes

### Performance
- Use async for concurrent operations
- Choose among the transports declared by the selected profile and supported by the camera configuration
- TCP is an ordered byte stream; UDP preserves datagram boundaries but requires application-appropriate timeout and retry policy
- Adjust timeouts, retry policy, and TCP keepalive based on network conditions

## Contributing

Found an issue or have an improvement? Please:
1. Test with your specific camera hardware
2. Include camera model and firmware version
3. Provide debug logs if reporting issues
4. Submit PRs with new examples for unique use cases

## License

These examples are part of the grafton-visca project and are licensed under MIT OR Apache-2.0 dual license.
