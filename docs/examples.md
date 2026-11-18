# 2.0 Example Policy

Examples are part of the 2.0 public surface. They should teach the owner-backed
APIs we expect to maintain, compile in CI, and avoid surprising camera movement
or camera configuration changes.

## Maintained Examples

The maintained examples in `examples/` fall into these categories:

| Category | Examples | Contract |
| --- | --- | --- |
| Quickstart | `quickstart`, `quickstart_async` | Connect through `CameraSession::connect_tcp`, query read-only state by default, and require `--move` for movement. `quickstart_async` is the Tokio example. |
| Inquiry and typed data | `inquiry_quickstart`, `typed_inquiry_demo`, `type_safe_commands` | Demonstrate accessors, profile metadata, conversion helpers, and capability bounds. |
| Transport setup | `transport_builder_demo`, `transports`, `builder_api`, `serial_async_demo` | Demonstrate current `CameraConfig`, `CameraSession::connect_tcp`, or caller-owned `Session::open` paths; `transports` exits unsuccessfully if any selected connectivity check fails. |
| Operational patterns | `concurrent_control`, `error_handling`, `runtime_demo`, `runtime_agnostic` | Demonstrate runtime and application patterns without relying on private runtime internals. |
| Presets | `preset_demo` | One owner-backed blocking preset operation selected on the command line: `set`, `recall` (targeted, so it waits for settled), or `clear`. Closes the session on every path. |
| Operation handles | `operation_handles`, `operation_handles_async` | Demonstrate blocking and Tokio submission, exact applied waits, profile-selected protocol-settlement waits, explicit detach paired with a bounded stop, a session that is closed on every path, and the caller-written stop-on-exit guard that bounds movement to a scope. |
| Cancellation and safety | `cancellation`, `cancellation_blocking`, `motion_safety` | Tokio `.cancel()` on a cancel-capable EVI-H100 over RS-232C (with an optional PTZOptics G2 unsupported-post-send mode), a hardware-free blocking confirmed-cancellation path on `SonyFr7`, and the blocking `motion()` safety view: `is_moving`, `wait_until_idle`, and the `stop_all` owner halt. |
| Custom requests and profiles | `custom_request` | A downstream custom `PlainCommand` and `OperationCommand`, encoded with `write_into` and submitted through blocking `Camera<AnyProfile>` against a custom runtime `ProfileSpec`. Runs its encoding and profile parts without a camera; uses the default blocking feature. |
| Runtime profile | `dyn_quickstart` | Runtime-selected `Camera<AnyProfile>` with `Operation<Targeted>` (has `settled`) and `Operation<AppliedOnly>` (no `settled`), using `Capabilities::supports` for discovery. Uses the same facade features as compile-time profiles. |
| Multi-camera and recovery | `multi_camera`, `recovery` | A multi-target session addressed by `camera::<P>(target)` with explicit `applied_with_timeout` waits, and fresh-session recovery after transport death or an application-owned silent-peer threshold using `received_frames`, a re-callable transport factory, a reused `SessionConfig`, end-to-end state re-query, and deliberate desired-state restoration. |
| Profile contributions | `profile_spec` | Hardware-free `export <id>` and `check <file>` for complete runtime specs; see the [profile promotion guide](camera_profile_support.md#worked-example-acme_camjson-to-a-registry-row) for the separate read-only ignored probe. |
| Protocol and lab validation | `sony_encapsulation`, `validate_ae_commands` | Profile-selected Sony encapsulation (the application never constructs the eight-byte header) and advanced lab material that should stay consistent with `docs/visca_reference.md`; not the recommended first path. |

## Rules

- Prefer `CameraSession::connect_tcp` for simple examples.
- Use `CameraConfig` for standard transport policy: timeouts, retries, TCP
  keepalive, queue depth, and camera ID.
- Use `Session::open` when the caller already owns the transport.
- Import `CameraSession::connect_tcp`, `CameraConfig`, `SessionConfig`, and profile types from their
  public modules. Use the inherent noun views on `Camera<P>` rather than
  private implementation submodules or compatibility traits.
- Use checked value constructors in examples, such as `UnitInterval::new(...)`
  and `CameraConfig::with_camera_id(IndividualCameraId::new(raw)?)`, when values come from user input.
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
  `submit` plus `applied` or `settled` when the example needs an exact
  operation lifecycle, cancellation, detach, or a profile-selected
  protocol-settlement wait.
- Propagate command, task-join, and shutdown errors in operational examples.
  A deliberate best-effort cleanup must report its failure rather than silently
  discarding it.
- When examples demonstrate optional vendor features, use the support-marker
  bounds for typed controls (`HasNdFilter`, `HasMotionSync`,
  `HasPanTiltSpeedMode`). Runtime discovery reads `ProfileSpec::capabilities()`,
  optional domain records, and `Capabilities::supports`. The
  maintained typed support matrix is: `SonyFr7` for ND filter and variable
  speed controls. Built-in PTZOptics profiles are not marked for typed Motion
  Sync because the current model capability specs do not establish that support.
- When an example or fixture adds support for a new camera profile capability,
  follow the [Camera Profile Support Guide](camera_profile_support.md) so the
  example matches the consolidated [VISCA Protocol Reference](visca_reference.md)
  and typed support markers.

## Release Checks

Before a 2.0 release candidate, run:

```sh
cargo +nightly fmt --all -- --check
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

Hardware smoke checks are optional and private for stable releases and
prereleases (#1008). See the [optional guidance](hardware_release_checklist.md).

The ignored profile probe reads queries on a known camera bench. Its candidate
results require evidence review; they do not install grants. AE validation is a
separate tool whose `--apply` flag explicitly permits control commands:

```sh
VISCA_CAMERA_IP=10.0.0.30 VISCA_PROFILE=generic_visca \
  cargo +1.98.0 test --features serde --lib profile_probe::profile_probe -- \
  --ignored --exact --nocapture
cargo run --example validate_ae_commands -- --apply
```
