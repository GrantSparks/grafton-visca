# Example Policy

Examples are part of the 1.x public surface. They should teach APIs we expect
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
| Operation handles | `operation_handles`, `operation_handles_async` | Demonstrate blocking and Tokio submission, exact applied waits, physical settled waits, explicit detach, and verified pan/tilt/zoom restoration after movement. |
| Protocol and lab validation | `sony_encapsulation`, `validate_inquiries`, `validate_ae_commands` | Advanced or lab-oriented material that should stay consistent with `docs/visca_reference.md`; not the recommended first path. |

## Rules

- Prefer `Connect` for simple examples.
- Use `CameraConfig` for standard transport policy: timeouts, retries, TCP
  keepalive, queue depth, and camera ID.
- Use `CameraBuilder` only when the caller already owns the transport.
- Import public camera construction/session types from `grafton_visca::camera`
  and static control traits from the crate root; do not teach private
  implementation submodules under `camera`.
- Use checked value constructors in examples, such as `UnitInterval::new(...)`
  and `CameraConfig::try_camera_id(...)`, when values come from user input.
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
- Use ordinary noun methods when command completion is sufficient. Use
  `submit` plus `await_applied` or `await_settled` when the example needs an
  exact per-command deadline, cancellation, detach, or a physical-settle signal.
  Use `submit_continuous` for custom applied-only commands.
- Propagate command, task-join, and shutdown errors in operational examples.
  A deliberate best-effort cleanup must report its failure rather than silently
  discarding it.
- When examples demonstrate optional vendor features, use the support-marker
  bounds for typed controls (`HasNdFilter`, `HasMotionSync`,
  `HasVariableSpeed`) and metadata traits only for runtime discovery. The
  maintained typed support matrix is: `SonyFR7` for ND filter and variable
  speed controls. Built-in PTZOptics profiles are not marked for typed Motion
  Sync because the current model capability specs do not establish that support.
- When an example or fixture adds support for a new camera profile capability,
  follow the [Camera Profile Support Guide](camera_profile_support.md) so the
  example matches the consolidated [VISCA Protocol Reference](visca_reference.md)
  and typed support markers.

## Release Checks

Before a 1.x release, run:

```sh
cargo +nightly fmt --all -- --check
cargo check --examples --no-default-features
cargo check --examples --no-default-features --features mode-async
cargo check --examples --no-default-features --features runtime-tokio
cargo check --examples --no-default-features --features runtime-smol
cargo check --example serial_async_demo --no-default-features --features runtime-tokio,transport-serial-tokio
cargo clippy --examples --no-default-features --features runtime-tokio -- -D warnings
bash .github/scripts/test-all-features.sh
```

Lab validation tools can be run separately against a known camera bench:

```sh
cargo run --example validate_inquiries
cargo run --example validate_ae_commands -- --apply
```
