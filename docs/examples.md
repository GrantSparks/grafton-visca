# Maintaining examples

Keep each example runnable with public APIs and focused on one task. List its
features, arguments, and hardware requirements in the [example catalog](../examples/README.md).

## Rules

- Use `CameraSession::connect_tcp` for a simple connection, `CameraConfig` for custom settings, and `Session::open` for a caller-owned transport.
- Declare the example’s feature requirements with Cargo’s `required-features`. Use checked input types such as `UnitInterval` and `IndividualCameraId`.
- Accept an address as an argument or environment variable. Quickstarts query state by default and move only with `--move`. Make movement or configuration changes explicit in other examples.
- Use camera accessors for ordinary controls. Show operation handles when a caller needs to wait, cancel, or detach. End continuous drives with an explicit STOP.
- Propagate command, task, and shutdown errors. Report failures during best-effort cleanup.
- For model-specific controls, use the appropriate `Has*` bound or check `Capabilities::supports` at runtime. Document the model evidence when adding a capability. [Camera profile support guide](camera_profile_support.md)

## Release Checks

Check formatting, feature requirements, and the hardware-free examples before a release.

```sh
cargo +nightly-2026-08-26 fmt --all -- --check
cargo check --examples --no-default-features
cargo check --examples --no-default-features --features async
cargo check --examples --no-default-features --features runtime-tokio
cargo check --examples --no-default-features --features runtime-smol
cargo check --example serial_async_demo --no-default-features --features runtime-tokio,transport-serial-tokio
cargo run --example type_safe_commands --no-default-features
cargo run --example runtime_agnostic --no-default-features --features async
cargo run --example custom_request --no-default-features --features blocking
cargo run --example multi_camera --no-default-features --features blocking
cargo run --example recovery --no-default-features --features blocking
cargo run --example cancellation_blocking --no-default-features --features blocking
cargo clippy --examples --no-default-features --features runtime-tokio -- -D warnings
cargo clippy --examples --no-default-features --features blocking -- -D warnings
bash .github/scripts/test-all-features.sh
```

Hardware smoke checks are optional and private. [Hardware guidance](hardware_release_checklist.md)

Use the [gated hardware wire tests](../tests/integration/hardware_wire_rows_test.rs)
for settings such as auto-exposure. The
[read-only profile probe](camera_profile_support.md#3-collect-read-only-evidence)
collects query and decoder candidates for evidence review.

```sh
VISCA_CAMERA_IP=10.0.0.30 VISCA_PROFILE=generic_visca \
  cargo +1.98.0 test --features serde --lib profile_probe::profile_probe -- \
  --ignored --exact --nocapture
```
