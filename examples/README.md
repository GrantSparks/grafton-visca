# Examples

To try the library without a camera, run this from the repository root. It
prints the capabilities of three camera profiles and shows how input validation
handles an out-of-range zoom value.

```sh
cargo run --example type_safe_commands
```

With a PTZOptics G2, replace `192.0.2.10` with its address. Both quickstarts
read power and zoom. Add `--move` after the address to drive the zoom briefly.

```sh
cargo run --example quickstart -- 192.0.2.10
cargo run --example quickstart_async --features runtime-tokio -- 192.0.2.10
```

## Without a camera

Pass any feature listed below with `--features`. A dash means the default build suffices.

| Example | Feature | What to try |
| --- | --- | --- |
| [type_safe_commands](type_safe_commands.rs) | — | Inspect profile capabilities, ranges, and checked values. |
| [profile_spec](profile_spec.rs) | `serde` | Export a profile with `-- export generic_visca` or validate a file with `-- check camera.json`. |
| [multi_camera](multi_camera.rs) | — | Address two targets on an in-memory serial bus. |
| [cancellation_blocking](cancellation_blocking.rs) | — | Cancel a zoom drive on a simulated Sony connection. |
| [recovery](recovery.rs) | — | Reconnect after a silent or closed peer and query state before restoring settings. |
| [runtime_agnostic](runtime_agnostic.rs) | `async` | Run tasks and timers with a custom executor. |
| [custom_request](custom_request.rs) | — | Encode a custom command and operation for a runtime profile. Leave the address and `VISCA_CAMERA_ADDR` unset for the offline demo. |

## Reading from a camera

Most network examples accept an address after `--` or in `VISCA_CAMERA_ADDR`.
The selected profile supplies its default port. [Transports and ports by model](../README.md#camera-support)

| Example | Feature | What it reads or configures |
| --- | --- | --- |
| [quickstart](quickstart.rs) | — | Power and zoom over TCP with the blocking API. |
| [quickstart_async](quickstart_async.rs) | `runtime-tokio` | The same inquiries with Tokio. |
| [inquiry_quickstart](inquiry_quickstart.rs) | — | Power state and pan/tilt, zoom, and focus positions. |
| [typed_inquiry_demo](typed_inquiry_demo.rs) | — | Typed power, zoom, and exposure-mode responses. |
| [transports](transports.rs) | — | Check G2 connectivity over TCP and UDP. |
| [transport_builder_demo](transport_builder_demo.rs) | — | Configure timeouts, buffers, and TCP keepalive. Pass `--udp` after the address to use UDP. |
| [builder_api](builder_api.rs) | — | Query power with a caller-owned UDP socket. Supply the port, e.g. `192.0.2.10:1259`. |
| [sony_encapsulation](sony_encapsulation.rs) | `runtime-tokio` | Query a Sony FR7 over encapsulated UDP. |
| [serial_async_demo](serial_async_demo.rs) | `transport-serial-tokio` | Read version and power over serial, e.g. `-- /dev/ttyUSB0 1`. |
| [runtime_demo](runtime_demo.rs) | `runtime-tokio` | Run inquiries concurrently in one session. |
| [error_handling](error_handling.rs) | `runtime-tokio` | Classify errors and try a configured connection. |

## Moving a camera or changing settings

| Example | Feature | What it does |
| --- | --- | --- |
| [operation_handles](operation_handles.rs) | — | Wait for a move, detach a drive, and send STOP on scope exit. |
| [operation_handles_async](operation_handles_async.rs) | `runtime-tokio` | Await movement and send STOP on success and error returns. |
| [concurrent_control](concurrent_control.rs) | `runtime-tokio` | Read power during a zoom drive. |
| [dyn_quickstart](dyn_quickstart.rs) | `runtime-tokio` | Select the G2 profile at runtime, home the camera, and stop the zoom. |
| [preset_demo](preset_demo.rs) | — | Store, recall, or clear a preset, e.g. `-- 192.0.2.10 recall 1`. |
| [cancellation](cancellation.rs) | `transport-serial-tokio` | Cancel an EVI-H100 zoom over serial. Use `--g2-unsupported` with a G2 TCP address to try a camera without socket cancellation. Both paths send STOP. |
| [motion_safety](motion_safety.rs) | — | Observe selected axes, stop movement, and inspect each axis’s result. |

For model-specific controls or instructions for contributing an example, see the
[profile guide](../docs/camera_profile_support.md#methods-by-profile) and the
[example policy](../docs/examples.md).
