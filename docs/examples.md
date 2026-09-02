# 2.0 Example Policy

Examples are part of the 2.0 public surface. They should teach the owner-backed
APIs we expect to maintain, compile in CI, and avoid surprising camera movement
or camera configuration changes.

## Maintained Examples

The maintained examples in `examples/` fall into these categories:

| Category | Examples | Contract |
| --- | --- | --- |
| Quickstart | `quickstart`, `quickstart_async` | Connect through `Connect`, query read-only state by default, and require `--move` for movement. `quickstart_async` is the Tokio example. |
| Inquiry and typed data | `inquiry_quickstart`, `typed_inquiry_demo`, `type_safe_commands` | Demonstrate accessors, profile metadata, conversion helpers, and capability bounds. |
| Transport setup | `transport_builder_demo`, `transports`, `builder_api`, `serial_async_demo` | Demonstrate current `CameraConfig`, `Connect`, or caller-owned `Session::open` paths. |
| Operational patterns | `concurrent_control`, `error_handling`, `runtime_demo`, `runtime_agnostic` | Demonstrate runtime and application patterns without relying on private runtime internals. |
| Presets | `preset_demo` | One owner-backed blocking preset operation selected on the command line: `set`, `recall` (targeted, so it waits for settled), or `clear`. Closes the session on every path. |
| Operation handles | `operation_handles`, `operation_handles_async` | Demonstrate blocking and Tokio submission, exact applied waits, profile-selected protocol-settlement waits, explicit detach paired with a bounded stop, a session that is closed on every path, and the caller-written stop-on-exit guard that bounds movement to a scope. |
| Cancellation and safety | `cancellation`, `motion_safety` | Tokio `.cancel()` — both the supported `Cancellation`/`outcome` path and the PTZOptics G2 unsupported-post-send `NotSupported`/`CancelRejected` recovery — and the blocking `motion()` safety view: `is_moving`, `is_moving_axes`, `wait_until_idle`, and the `stop_all_motion` composite. |
| Custom requests and profiles | `custom_request` | A downstream custom `PlainCommand` and `OperationCommand`, encoded with `write_into` and submitted through blocking `camera_dyn` against a custom runtime `ProfileSpec`. Runs its encoding and profile parts without a camera; requires `dyn-api`. |
| Dynamic API | `dyn_quickstart` | The profile-erased `DynSessionCamera`: a `DynTargetedOperation` (has `settled`) and a `DynAppliedOperation` (no `settled`), with `supports_typed` capability discovery. Requires `dyn-api`. |
| Multi-camera and recovery | `multi_camera`, `recovery` | A multi-target session addressed by `camera_for` with explicit `applied_with_timeout`/`settled_with_timeout` waits, and fresh-session recovery after transport death or an application-owned silent-peer threshold using `received_frames`, a re-callable transport factory, a reused `SessionConfig`, and end-to-end state re-query. |
| Protocol and lab validation | `sony_encapsulation`, `validate_inquiries`, `validate_ae_commands` | Advanced or lab-oriented material that should stay consistent with `docs/visca_reference.md`; not the recommended first path. |

## Rules

- Prefer `Connect` for simple examples.
- Use `CameraConfig` for standard transport policy: timeouts, retries, TCP
  keepalive, queue depth, and camera ID.
- Use `Session::open` when the caller already owns the transport.
- Import `Connect`, `CameraConfig`, `SessionConfig`, and profile types from their
  public modules. Use the inherent noun views on `Camera<P>` rather than
  private implementation submodules or compatibility traits.
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
  `submit` plus `applied` or `settled` when the example needs an exact
  operation lifecycle, cancellation, detach, or a profile-selected
  protocol-settlement wait.
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

Before a 2.0 release candidate, run:

```sh
cargo +nightly fmt --all -- --check
cargo check --examples --no-default-features
cargo check --examples --no-default-features --features async
cargo check --examples --no-default-features --features runtime-tokio
cargo check --examples --no-default-features --features runtime-smol
cargo check --example serial_async_demo --no-default-features --features runtime-tokio,transport-serial-tokio
cargo clippy --examples --no-default-features --features runtime-tokio -- -D warnings
cargo clippy --examples --no-default-features --features runtime-tokio,dyn-api -- -D warnings
cargo clippy --examples --no-default-features --features blocking -- -D warnings
bash .github/scripts/test-all-features.sh
```

Lab validation tools can be run separately against a known camera bench:

```sh
cargo run --example validate_inquiries
cargo run --example validate_ae_commands -- --apply
```
