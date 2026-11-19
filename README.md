# grafton-visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](#license)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

Control VISCA pan-tilt-zoom cameras from Rust over TCP, UDP, or serial. Built-in
profiles for PTZOptics, Sony, and Nearus models let you read lens position,
recall presets, or adjust exposure where supported. Use blocking calls, Tokio,
smol, or your own async executor. [Check your camera](#camera-support)

## Quick start

Add the crate to `Cargo.toml`. The default blocking API needs no async runtime.
Rust 1.98 or later is supported on Linux, macOS, and Windows.

```toml
[dependencies]
grafton-visca = "=2.0.0"
```

Read power and zoom from a PTZOptics G2 over TCP. Replace `192.0.2.10` with
your camera’s address. The profile supplies the default port.

```rust,no_run
use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2, Result};

fn main() -> Result<()> {
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.0.2.10")?;
    let camera = session.camera();
    println!("Power: {:?}", camera.power().status()?);
    println!("Zoom position: {:#06x}", camera.zoom().position()?.value());
    session.close()
}
```

From a repository checkout, run `cargo run --example type_safe_commands` to
inspect profile capabilities and try range checks without a camera. With a G2,
run the blocking or Tokio quickstart to read from the camera.

```sh
cargo run --example quickstart -- 192.0.2.10
cargo run --example quickstart_async --features runtime-tokio -- 192.0.2.10
```

## Camera support

<!-- BEGIN GENERATED PROFILE TRANSPORTS -->
| Profile | TCP default | UDP default | Serial | Envelope |
| --- | --- | --- | --- | --- |
| `PtzOpticsG2` | 5678 | 1259 | yes | `RawVisca` |
| `PtzOpticsG3` | 5678 | 1259 | yes | `RawVisca` |
| `PtzOptics30x` | 5678 | 1259 | yes | `RawVisca` |
| `SonyFr7` | n/a | 52381 | no | `SonyEncapsulated` |
| `SonyBrcH900` | n/a | 52381 | no | `SonyEncapsulated` |
| `SonyEviH100` | n/a | n/a | yes | `RawVisca` |
| `SonyBrc300` | n/a | n/a | yes | `RawVisca` |
| `NearusBrc300` | n/a | n/a | yes | `RawVisca` |
| `GenericVisca` | 5678 | 1259 | yes | `RawVisca` |
<!-- END GENERATED PROFILE TRANSPORTS -->

`PtzOptics30x` covers the Gen-2 PT30X SDI/NDI models. `GenericVisca` provides
a conservative set of shared VISCA commands. The table reflects model
documentation and protocol tests. Hardware verification is limited for raw
serial and Sony UDP. [Protocol sources](docs/visca_reference.md) · [2.0.0 release notes](CHANGELOG.md#200---2026-11-19)

For controls such as G2 focus lock or the FR7’s variable ND filter, a typed
profile checks method availability at compile time. With a runtime-selected
profile (`AnyProfile`), the library checks those controls before sending a
command. [Controls by model](docs/camera_profile_support.md#built-in-controls)

## Cargo features

| Use case | Feature |
| --- | --- |
| Blocking TCP or UDP | `blocking` (default) |
| Async TCP or UDP | `runtime-tokio` or `runtime-smol` |
| Caller-provided async executor and transport | `async` |
| RS-232/422 serial | `transport-serial` (blocking) or `transport-serial-tokio` (Tokio) |
| Serialization, JSON Schema, TypeScript types | `serde`, `schemars`, `ts-rs`, respectively |
| Scripted transports and other test helpers | `test-utils` |

For an async-only build, set `default-features = false` and enable the feature
for your runtime.

## Working with cameras

- **Connect and configure.** Open a single camera, address several cameras on a serial bus, or supply your own transport. [Usage guide](docs/usage_2_0.md)
- **Control movement.** `applied()` waits for protocol completion. Targeted moves also offer `settled()` for profile-specific evidence of rest. Dropping a handle lets the command continue. End continuous motion with an explicit STOP. [Blocking example](examples/operation_handles.rs) · [Tokio example](examples/operation_handles_async.rs)
- **Build an application.** Browse the [example catalog](examples/README.md), [API reference](https://docs.rs/grafton-visca), and [error recovery guide](docs/observability_and_recovery.md).
- **Upgrade from 1.x.** [2.0 migration guide](docs/migration_2_0.md) · [Changelog](CHANGELOG.md)
- **Contribute a fix or camera profile.** [Contributor guide](CONTRIBUTING.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
