# Example Policy

Examples are part of the 1.0 public surface. They should teach APIs we expect
to maintain, compile in CI, and avoid surprising camera movement or camera
configuration changes.

## Maintained Examples

The maintained examples in `examples/` fall into these categories:

| Category | Examples | Contract |
| --- | --- | --- |
| Quickstart | `quickstart`, `quickstart_async` | Connect through `Connect`, query read-only state by default, and require `--move` for movement. `quickstart_async` is the Tokio example. |
| Inquiry and typed data | `inquiry_quickstart`, `typed_inquiry_demo`, `type_safe_commands` | Demonstrate accessors, profile metadata, conversion helpers, and capability bounds. |
| Transport setup | `transport_builder_demo`, `transports`, `builder_api`, `serial_async_demo` | Demonstrate current `CameraConfig`, `Connect`, or `CameraBuilder` paths. |
| Operational patterns | `concurrent_control`, `error_handling`, `runtime_demo`, `runtime_agnostic` | Demonstrate runtime and application patterns without relying on private runtime internals. |
| Protocol references | `sony_encapsulation`, `validate_inquiries`, `validate_ae_commands` | Advanced or lab-oriented material; not the recommended first path. |

## Rules

- Prefer `Connect` for simple examples.
- Use `CameraConfig` for standard transport policy: timeouts, retries, TCP
  keepalive, queue depth, and camera ID.
- Use `CameraBuilder` only when the caller already owns the transport.
- Default examples should be read-only where possible.
- Examples that move hardware must make that behavior explicit in the command
  line or in the operation name.
- Examples should accept a camera address as an argument or environment
  variable. Hard-coded lab IPs are allowed only in validation tools.
- Prefer Cargo `required-features` over fallback `main` functions for examples
  that require a positive feature such as `runtime-tokio`.
- Avoid examples that only print code snippets. Put conceptual guidance in docs
  and keep examples runnable.
- Keep examples focused. A single example should demonstrate one API path or one
  operational pattern, not a broad hardware tour.

## Release Checks

Before a 1.0 release, run:

```sh
cargo fmt
cargo check --examples
cargo check --examples --features runtime-tokio
cargo check --examples --features runtime-smol
cargo check --example serial_async_demo --features runtime-tokio,transport-serial-tokio
```

Lab validation tools can be run separately against a known camera bench:

```sh
cargo run --example validate_inquiries
cargo run --example validate_ae_commands
```
