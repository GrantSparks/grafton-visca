# Migrating from 1.x to 2.0

2.0 is a clean break in naming and ownership. The mappings below describe the
supported destination for applications migrating from pre-2.0 vocabulary; the
historical names are not part of the 2.0 contract.

## First compile break: control calls return operation handles

Movement and other lifecycle-bearing control methods no longer return a bare
`Result<()>`. They return a `#[must_use] Result<Operation<K>>` on the blocking
facade, or an async future yielding that result. The handle is the authority to
observe application, wait for profile-selected settlement, cancel, or detach.

| 1.x pattern | 2.0 pattern |
| --- | --- |
| `camera.pan_tilt_home().await?;` | `camera.pan_tilt().home().await?.applied().await?;` |
| `camera.zoom_stop()?;` | `camera.zoom().stop()?.applied()?;` |
| Fire a command and later cancel by command/socket ID | Keep the returned `Operation<K>` and call `cancel()` on that handle. |
| Ignore a successful control return | Bind the handle and explicitly call `applied()`, `settled()`, `cancel()`, or `detach()`; `#[must_use]` makes an accidental drop visible. |

This applies across pan/tilt, zoom, focus, presets, iris, and ND-filter
operations—not only normalized zoom. A completed VISCA command is not always a
physical-rest observation. `settled()` returns `Settlement`: either exact
profile-declared completion evidence or stable position samples over a reported
window and tolerance. Stable samples do not prove arrival at the requested endpoint.

## Extensible public payloads (#788)

Public configuration records and named error, diagnostic, response, transport
option and test-script payloads reserve room for future fields. Construct them
through their existing fluent methods or new narrow constructors. For records
with public writable fields and Default, assign only the fields you change:

```rust
use grafton_visca::{Error, transport::TransportConfig};

let mut config = TransportConfig::for_tcp();
config.buffer_config.max_buffer_size = 16_384;
let error = Error::connection_closed(None);
match error {
    Error::ConnectionClosed { reason, .. } => assert!(reason.is_none()),
    _ => {},
}
```

Named patterns need `..` even when the parent enum was already non-exhaustive.
Matches over `InquiryKind`, `InquiryData`, `FrameSequence`, `ReceiveOutcome`,
`Step` and `TestExecutorType` also need a wildcard arm. These registries can grow;
fixed wire/value records and command payloads retain their literal construction
and exhaustive field patterns. Opaque types with private fields continue to use
their existing constructors.

Custom parsers use `Response::cmd_ack`, `completion` or `unknown`; custom transports
report every receive as a `ReceiveOutcome` (`complete`, `truncated`,
`possibly_truncated`, or `copy_message` for a source that yields whole messages)
and use `FrameMeta::new`. Capability validators use `ValidationError::out_of_range` or
`invalid_value`; test scripts use `Step::on_send` or `after`. Error constructors
use snake_case names such as `Error::feature_not_supported`, `invalid_parameter`,
`runtime_queue_full` and `stream_poisoned`; `Error::timeout(stage, certainty)`
retains structured deadline evidence. `Settlement::profile_completion`,
`observed_stable` and `HaltReport::new` support downstream fake adapter results.

Exported macros follow the same construction rules. A configured `ViscaEnum`
error type whose final path segment is `Error` must offer
`invalid_response(expected, actual)`, accepting `Cow<'static, str>` (or
`impl Into<Cow<'static, str>>`) and a `Vec<u8>` payload. Other custom error names retain the `From<String>` convention.
Renamed dependency aliases remain supported.

## Construction and feature selection

| 1.x category | 2.0 destination |
| --- | --- |
| `mode-async` | `async`; add `runtime-tokio` or `runtime-smol` when using a built-in runtime. |
| Generic mode/transport/executor `Camera<Mode, P, T, E>`, `CameraSession<Async, P, T, E>`, and `AsyncCamera` | `blocking::Session` or async `Session`, then a target view. An owned async adapter generally stores `Camera<P>`; a single-target connection may instead retain `CameraSession<P>`. Transport and executor types no longer appear in either facade type. |
| `BlockingClient` and direct generic camera constructors | `blocking::Connect`, `blocking::CameraConfig`, or `blocking::Session::open`. |
| 1.2.0's deprecation of the blocking `CameraSession` methods, which pointed at `BlockingClient` | `blocking::CameraSession<P>` again — see [The `BlockingClient` inversion](#the-blockingclient-inversion) below. |
| Legacy `Connect`/`CameraConfig` single-target shortcuts | Keep the convenience path, but use `SessionConfig` when configuration must be cloned, reused, or shared across targets. |
| Direct `Camera::new_*`, `open_*` compatibility constructors | `Connect`/`CameraConfig` for standard transports; `Session::open` for a caller-owned transport. |
| One implicit camera ID or raw camera-ID setters | `CameraId`, `SessionConfig::for_target`, `try_camera_id`, and explicit `camera_for`. |
| Root camera-number constants | `CameraId` plus a validated `ProfileSpec`/compile-time profile. (`CameraVariant` had already been removed before the 1.x oracle used for this guide.) |
| `RuntimeHandle` and private scheduler/runtime modules | `TokioRuntime`, `SmolRuntime`, or a coherent public `Executor`; never construct the owner directly. |
| `CameraBuilder` and its `with_executor(...).from_transport(...).profile::<P>().open_async()` chain | `Connect` or `CameraConfig` for standard transports; async `Session::open(transport, SessionConfig, executor)` or blocking `blocking::Session::open(transport, SessionConfig)` for a caller-owned one. `camera_id(...)` becomes a `SessionConfig` target (`for_target`/`register_target`) or `CameraConfig::camera_id`; `timeout_config`/`retry_config` become an `OperationalTuning` supplied through `with_tuning`. |
| Implicit Sony sequence synchronization at connection startup | Startup remains write-free by default. Opt in with `SessionConfig::with_sony_sequence_reset_on_connect(true)` or the matching `CameraConfig` builder only for a Sony-encapsulated profile; the RESET is sent before owner work begins. Observe its one-/two-byte reply as `DiagnosticResponse::SonyControl { code }`. |
| `Runtime::connect_tcp` / `connect_udp` and the `TransportHandle` enum | Both remain under `async` as `runtime::{Runtime, TransportHandle}`. Prefer `Connect`/`CameraConfig`; when driving the transport yourself, use `Session::open(TransportHandle::Tcp(runtime.connect_tcp(addr, cfg).await?), config, runtime.clone())` (and the corresponding UDP variant). In 2.0.0-rc.2 the generic **connector** futures carry the `Send` contract needed to box or spawn this construction path; this does not change `Session::open` after a caller has already created a custom transport. See the rc.1 note below. |
| Preview-only `BufferConfig::send_buffer_size` / `NetTransportBuilder::send_buffer_size` | Removed before rc.1 because neither affected production allocation. Owner transmit buffers are fixed and reused at protocol-bounded capacity; configure only receive/framing memory with `recv_buffer_size` and `max_buffer_size`. |

`SessionConfig` accepts only individual VISCA IDs 1–7. Broadcast, duplicate
registration, an empty registry, and unsupported profile/transport pairs are
errors before I/O. `camera()` is only for a sole target; multi-target code must
select `camera_for(target)`.

### Porting an async connection adapter

The 1.x async suffix is gone from `Connect`; the enclosing async facade now
provides the distinction:

| 1.x call | 2.0 call and result |
| --- | --- |
| `Connect::open_tcp_async::<P, _>(address, runtime).await?` | `Connect::open_tcp::<P, _>(address, runtime).await?` → `CameraSession<P>` |
| `Connect::open_udp_async::<P, _>(address, runtime).await?` | `Connect::open_udp::<P, _>(address, runtime).await?` → `CameraSession<P>` |
| `Connect::open_serial_async::<P, _>(port, baud, runtime).await?` | `Connect::open_serial::<P, _>(port, baud, runtime).await?` → `Session` |

> **2.0.0-rc.1 construction-future defect.** The rc.1 `Runtime` connector
> trait did not expose a usable `Send` guarantee through generic TCP, UDP, and
> serial construction. An application that boxed or spawned a
> **connector-backed** camera-opening future could therefore see
> `implementation of Send is not general enough`. Upgrade to 2.0.0-rc.2: its
> connector contract fixes the `Connect`/`CameraConfig::open_async` and
> `Runtime::connect_*` boundary. It does not change direct `Session::open` with
> an already-created transport or the `Send` obligations of a custom transport
> implementation. This is not an application ownership, noun-accessor, or
> cancellation rewrite.

For TCP or UDP, call `into_camera()` when an application abstraction should own
only a cloneable `Camera<P>` view. That view keeps the owner and connection
alive; the tradeoff is giving up the consuming `CameraSession::close` teardown
barrier. Serial is a multi-target transport even when only one target was
configured, so select an owned view with `session.camera::<P>()?` for the sole
target or `session.camera_for::<P>(target)?` for an explicit target. Keep the
`Session` as well when the application needs deterministic `close` or additional
target views.

This changes generic adapter design. A blanket implementation over
`CameraSession<Async, P, T, E>` cannot preserve its old type parameters because
transport and executor are now private owner details. Implement the adapter for
`Camera<P>`, retain `CameraSession<P>` when single-target teardown is part of
the abstraction, or use `Session` plus `Camera<P>` / `DynSessionCamera` when
targets or profiles are selected at runtime.

### Feature resolution inverted

1.x had **no `blocking` feature** and shipped `default = []`. The blocking API
was the implicit baseline, and `mode-async` — pulled in transitively by
`runtime-tokio`, `runtime-smol`, `dyn-api`, and `transport-serial-tokio` —
structurally *replaced* it: enabling any async feature dropped the blocking
types (`BlockingCamera`, `BlockingClient`) from the crate and exported
`AsyncCamera` instead. A build was therefore guaranteed to be blocking **XOR**
async. There was no `compile_error!` guard, because the exclusion was enforced
by `cfg(mode-async)` on the exported items rather than by a check.

2.0 inverts this:

| 1.x feature reality | 2.0 |
| --- | --- |
| `default = []`, blocking implicit | `default = ["blocking"]`, blocking explicit |
| `mode-async` (the only mode toggle) | `async`; add `runtime-tokio` or `runtime-smol` for a built-in runtime. For dynamic async views, enable `dyn-api` **and** `async` (or a `runtime-*` feature); `dyn-api` alone is the native blocking projection when `blocking` is enabled. |
| blocking XOR async, enforced by `cfg` | `blocking` and `async` are independent and **co-enableable** in one build |
| `--no-default-features` ⇒ blocking crate | `--no-default-features` (no facade) ⇒ pure engine/domain layers only |
| `transport-serial` did not select an explicit blocking feature | `transport-serial` now enables `blocking`; use `transport-serial-tokio` for async Tokio serial |

So one 2.0 build can expose both `grafton_visca::Camera` (async) and
`grafton_visca::blocking::Camera`. If you relied on 1.x's implicit-blocking
default you are unaffected — it is now the explicit default. If you enabled
`mode-async`, switch to `async` (or a `runtime-*` feature) and add `blocking`
only if you also want the blocking facade in the same build. A manifest that
used 1.x `dyn-api` for an async object-safe surface should be explicit in 2.0:
use `default-features = false, features = ["async", "dyn-api"]` (or
`runtime-tokio`/`runtime-smol` in place of `async`). With defaults retained,
`features = ["dyn-api"]` selects the blocking dynamic projection as well as
the default blocking facade; it does not enable async by itself.

### The `BlockingClient` inversion

Read this if you acted on the 1.2.0 deprecation warnings.

1.2.0 deprecated the blocking-only `CameraSession` methods and told callers to
use `BlockingClient` instead: at the time `CameraSession::new` was gated behind
`mode-async`, so no blocking caller could construct one, and `BlockingClient`
was the single blocking handle.

2.0 reverses both halves of that. `BlockingClient` does not exist, and
`CameraSession` is the promoted name for the single-camera blocking handle:
`blocking::CameraSession<P>` owns its session and hands out the `P` camera view
with the profile bound at compile time. A caller who obeyed the 1.2.0 warning
migrated *away from* the name 2.0 kept.

Nothing about the deprecation is salvageable as a mechanical rename — the
constructors, the ownership model, and the noun surface all changed — so
migrate from wherever you are now:

* `BlockingClient` for one network camera →
  `blocking::Connect::open_tcp::<P>` or `open_udp::<P>` (configured form:
  `blocking::CameraConfig::<P>::open`). These return
  `blocking::CameraSession<P>`; `session.camera()` is the noun view and takes
  no turbofish.
* `BlockingClient` for cameras on a serial bus → under `transport-serial`,
  `blocking::Connect::open_serial::<P>` (configured form:
  `blocking::CameraConfig::<P>::open_serial`). These return
  `blocking::Session`; select each registered address with
  `session.camera_for::<P>(target)`.
* A caller-owned transport → `blocking::Session::open(transport, config)`, or
  `blocking::CameraSession::open(transport, &CameraConfig::<P>::new())` for the
  single-camera bind.
* 1.x per-handle state on `BlockingClient` is not per-handle in 2.0. Camera ID
  is a registered target on `SessionConfig`, and timeout overrides are supplied
  to `CameraConfig::with_tuning`.

The 1.2.0 `CameraSession` **async** surface was never deprecated and maps to
the async `Session` / `CameraSession<P>` rows above.

## Static and dynamic nouns

| 1.x category | 2.0 destination |
| --- | --- |
| Root control-trait calls that duplicate noun views | `camera.power()`, `zoom()`, `system()`, `pan_tilt()`, `focus()`, `exposure()`, `white_balance()`, `image()`, `presets()`, `tally()`, `nd_filter()`, `motion_sync()`, `menu()`, and `advanced()`. |
| 1.x `is_moving` / `stop_all_motion`, and the movement waits `await_idle` / `await_pan_tilt_idle` / `await_zoom_idle` / `await_focus_idle` / `await_axes_idle` (each taking a `Duration`) | `camera.motion().is_moving(MotionQuery)` (`MotionQuery::default()` samples `AffectedAxes::MOVEMENT`; `MotionQuery::new(axes)` selects an explicit axis set), `wait_until_idle(IdleWait)`, and `stop_all_motion()` returning a per-axis `HaltReport`. Inspect the report or explicitly collapse it with `into_result()`. There was **no** 1.x `wait_until_idle`; that name is 2.0's. |
| 2.0.0-rc.2 `MotionQuery { axes, tolerance }` and an `is_moving`/`is_moving_axes` verdict from two back-to-back snapshots | `MotionQuery::new(axes)`, optionally `.with_tolerance(..)` and `.with_window(..)`. `MotionQuery` and `IdleWait` are `#[non_exhaustive]`, so struct literals no longer compile; use the constructors and `with_*` methods. The second snapshot now starts at least `window` (default 100 ms) after the first, so each call takes at least that long. `false` means no movement detected over the window. A zero window returns `Error::InvalidParameter`, and a window that cannot fit the deadline returns `Error::Timeout`. Raise the window to detect slower creep (#781). |
| 1.x `AwaitConfig` and `await_with_config(&AwaitConfig)` (`for_pan_tilt`/`for_zoom`/`for_focus`/`for_preset_recall`, `poll_interval`, `tolerance`, `debug`) | `camera::IdleWait` (same `for_*` presets plus `with_interval`/`with_tolerance`/`with_timeout`) passed to `wait_until_idle`, or `camera::MotionQuery` for `is_moving`. The `debug` field has no counterpart — use `tracing`. |
| Noun-specific idle/wait aliases | The separate `motion()` safety/observation view. |
| 1.x `Axes::ALL` as "everything that moves" | `AffectedAxes::MOVEMENT`. The 1.x `Axes` type is renamed `AffectedAxes` **and** `ALL` changed meaning — see [`Axes` → `AffectedAxes`: rename and `ALL` meaning change](#axes--affectedaxes-rename-and-all-meaning-change) below. |
| `DynCameraControl` | `DynSessionCameraControl` plus `DynSessionCameraNouns`. |
| `DynPanTiltControl`, `DynZoomControl`, `DynFocusControl`, `DynPresetsControl`, and `DynMotionControl` | `DynPanTilt`, `DynZoom`, `DynFocus`, `DynPresets`, and `DynMotion`. The final dynamic surface also has `DynPower`, `DynSystem`, `DynExposure`, `DynWhiteBalance`, `DynImage`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`, and `DynAdvanced`. |
| `IntoDynCamera` and wrapper-specific dynamic constructors | `Session::camera_dyn` / `camera_dyn_for`, returning `DynSessionCamera` (async) or `BlockingDynSessionCamera` (blocking). |
| Metadata-only optional control fallback | Static `Has*` marker gates, or dynamic `supports_typed(...)` followed by the matching `Dyn*` noun. |
| Duplicate `NdFilterInquiry` accessor vocabulary | `nd_filter().position()` only. |
| Separate focus `lock()`/`unlock()` twins | One parameterized `focus().set_lock(FocusLock)`. |
| Root `toggle_menu()` (the 1.x `DirectMenuControl` method on the camera) | `menu().toggle_display()` is the direct replacement. Prefer `menu().display(true)` / `menu().display(false)` when the intended state is known; `menu().status()`, `navigate(...)`, and `select()` cover the remaining menu operations. |
| Zoom `set_normalized(UnitInterval)` / `set_normalized_in_domain(UnitInterval, ZoomDomain)` (1.x and 2.0.0-rc.3) | One method, `zoom().set_normalized(position, domain)`: pass `ZoomDomain::Optical` for the former one-argument form and the domain directly for the former `set_normalized_in_domain`. It is a **targeted operation** returning an `Operation<Targeted>`; await it with `applied()`/`settled()` instead of getting a bare `Result<()>`. |
| `pan_tilt().up/down/left/right(pan_speed, tilt_speed)` (1.x and 2.0.0-rc.3) | `pan_tilt().move_direction(PanTiltDirection::Up, pan_speed, tilt_speed)`, and likewise `Down`, `Left` and `Right`. The frame is identical. The same applies to `DynPanTilt`. |
| `nd_filter().set_value(u16)` / `set_stops(f32)` (2.0.0-rc.3) | `nd_filter().set_value(NdFilterValue)`, building the value with `command::NdFilterValue::new(raw)?` or `NdFilterValue::from_stops(stops)?`. An out-of-range raw value or stop count is now rejected by the constructor, before any noun call. |
| `motion().is_moving()` / `is_moving_axes(MotionQuery)` (2.0.0-rc.3) | `motion().is_moving(MotionQuery)`. Pass `MotionQuery::default()` for the former no-argument form, which samples `AffectedAxes::MOVEMENT`. |

The **async** dynamic views are object-safe and erase profile/request types;
the blocking dynamic projection is native and has no futures. Both share the
static session's owner, timeout, pacing, cancellation, and state cache. There
is no dynamic policy layer that can bypass static preparation.

For an adapter that previously expressed support through root control-trait
bounds, replace each call site with its noun rather than looking for a renamed
root trait:

| 1.x trait family | 2.0 noun |
| --- | --- |
| `MotionControl` / `PanTiltControl` | `camera.pan_tilt()` for commands; `camera.motion()` for aggregate observation, idle waits, and STOP-all |
| `ZoomControl` / `DirectZoomControl` | `camera.zoom()` |
| `FocusControl` | `camera.focus()` |
| `ExposureControl` / iris traits | `camera.exposure()` |
| `WhiteBalanceControl` and its color-temperature/gain controls | `camera.white_balance()`; saturation, hue, and general image controls belong to `camera.image()` |
| `PresetsControl`, `PowerControl`, `MenuControl` | `camera.presets()`, `camera.power()`, and `camera.menu()` respectively; vendor streaming-quality controls live under the profile-gated `camera.advanced()` noun. |
| `InquiryControl` | The inquiry on the noun that owns the value; there is no aggregate replacement trait. |
| `SystemControl` | `camera.system().version()` for the Sony-format version reply on profiles that implement `HasVersionInquiry` (Sony and `GenericVisca`; not the PTZOptics profiles, whose reply layout is unsourced, so send a `raw::Inquiry` there; custom profiles must implement `HasVersionInquiry`, include `TypedSupportSurface::VersionInquiry` in `TYPED_SUPPORT`, and list `"version-inquiry"` in persisted runtime `typed_support`, as described in the next section's gate table) and the profile-gated `camera.system().save_settings()` for PTZOptics settings persistence. Configure serial Address Set and I/F Clear at connection time with `transport::serial::Startup`, through `transport::serial::Config::startup` or `CameraConfig::serial_startup` (Address Set runs first when both are selected; the default writes nothing to the bus). Cancel a submitted command through its `Operation::cancel()` handle; use `camera.motion()` only for movement observation and typed STOP-all. |

### Layered async trait adapters

A noun accessor is a borrowed view, and its async methods borrow that accessor.
Do not return a future directly from a temporary such as
`camera.image().contrast()` through an application-erased future. Bind the
accessor inside the async block and await its method there:

```rust,ignore
let future = Box::pin(async move {
    let image = camera.image();
    image.contrast().await
});
```

The rc.1 constructor-future defect above is separate: if the diagnostic passes
through connector-backed `Connect`, `CameraConfig::open_async`, or
`Runtime::connect_*` construction, upgrade to rc.2. Direct `Session::open`
with an already-created/custom transport is outside that fix; its future and
transport bounds remain the application's responsibility. Boxing domain futures
or cloning a `Camera<P>` cannot repair a connector future before the camera has
been constructed.

There is also an application-owned Rust type-system edge for libraries with
multiple trait layers. If *your* domain trait returns
`impl Future + Send + '_` and a second object-safe trait boxes that opaque
future, some compiler versions can report `implementation of Send is not
general enough` because the higher-ranked lifetime proof crosses both opaque
layers (the general limitation is tracked by
[rust-lang/rust#100013](https://github.com/rust-lang/rust/issues/100013)). Box
once at the application domain-trait boundary instead—for example, return the
application's `Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>` alias from
that trait—and have the outer facade delegate that boxed future directly. An
owned adapter may instead clone an already-open `Camera<P>` into a `'static`
future. Dropping `Send` merely moves the failure to an executor boundary and is
not a migration.

The old duration-only movement helpers map mechanically to `IdleWait` values:

| 1.x | 2.0 |
| --- | --- |
| `await_pan_tilt_idle(duration)` | `motion().wait_until_idle(IdleWait::for_pan_tilt().with_timeout(duration))` |
| `await_zoom_idle(duration)` | `motion().wait_until_idle(IdleWait::for_zoom().with_timeout(duration))` |
| `await_focus_idle(duration)` | `motion().wait_until_idle(IdleWait::for_focus().with_timeout(duration))` |
| `await_idle(duration)` | `motion().wait_until_idle(IdleWait::default().with_timeout(duration))` |

### Pan/tilt widths and explicit profile gates

Raw pan/tilt coordinates are `i32` in 2.0 so the BRC-300's documented signed
20-bit pan field can be represented without truncation. This changes
`PanTiltPosition`, the pan/tilt field of
`MovementTolerance`, inquiry payloads, `PanTiltExt::{validate_pan,validate_tilt}`,
`ProfileSpecBuilder::pan_tilt`, `Capabilities::{pan_range,tilt_range}`, and
`PanTilt::{PAN_RANGE,TILT_RANGE}` (including every built-in profile constant).
Code that persisted `i16` remains value-compatible after an explicit widening;
generic signatures and custom profile implementations must change their type to
`i32`. Use checked narrowing only when talking to a standard 16-bit profile.
`Error::CameraMoving` does not need a width migration because that unused
variant is removed in 2.0 (#722).

Each profile also selects a `PanTiltWireCodec`. Do not assume that every
VISCA-compatible model uses the common two-speed/four-plus-four-nibble layout;
the profile owns coordinate widths, signedness, axis polarity, and framing.

Optional typed controls now name the exact evidence boundary:

| 1.x/broad assumption | 2.0 bound or action |
| --- | --- |
| Exposure-mode methods were available through broad exposure support | Add `HasExposureMode`; `SonyFR7` intentionally does not implement it because its documented family is different. |
| `iris_control()` followed the standard iris-control marker | Add `HasIrisControlInquiry`. No built-in profile opts into the ambiguous `09 04 2B` status inquiry; standard iris position/control remains under `HasIrisControl` where documented. |
| One focus-zone marker covered command and inquiry | The inquiry requires `HasFocusZoneInquiry`. Only `PtzOpticsG2` and the legacy `PtzOptics30X` profile carry it; `PtzOpticsG3` and `SonyFR7` no longer compile for this unsourced inquiry. |
| Aggregate noise-reduction support implied setters and inquiries | Use `HasNoiseReduction2D` / `HasNoiseReduction3D` for inquiries and the matching `*Control` markers for setters, as detailed below. |
| PTZOptics advanced methods were ungated | Add the relevant `HasPtzOpticsAntiFlicker`, `HasPtzOpticsMulticastStreaming`, `HasPtzOpticsNdiQuality`, `HasPtzOpticsPresetRecallSpeed`, or `HasPtzOpticsSettingsSave` bound. |
| Sony auto-slow-shutter and spotlight methods were broadly exposed | Add `HasSonyAutoSlowShutter` or `HasSonySpotlight`; unsupported profiles reject through the dynamic API before encoding. |
| USB-audio methods were broadly exposed | Add `HasUsbAudio`. Only `PtzOpticsG2` and legacy `PtzOptics30X` currently carry source-backed support; `PtzOpticsG3` does not. |
| `system().version()` was available on every profile and assumed the Sony 7-byte reply | Add `HasVersionInquiry`. The Sony profiles and `GenericVisca` carry it; `PtzOpticsG2`, `PtzOpticsG3`, and `PtzOptics30X` do not, because G2 hardware replies `90 50 00 52 FF` and no source defines that layout. Dynamic calls on those profiles return `FeatureNotSupported` before any I/O. Read the bytes with `raw::Inquiry` and `81 09 00 02 FF`. A custom profile needs all three: implement `HasVersionInquiry` on the compile-time type, include `TypedSupportSurface::VersionInquiry` in `ProfileTypedSupport::TYPED_SUPPORT`, and add `"version-inquiry"` to the runtime `capabilities.typed_support` list before persisting it; a spec saved by 2.0.0-rc.3 does not load and must be rebuilt (see below) (#795). |
| `white_balance().color_temperature()` needed only `HasColorTemperature` | Add `HasColorTemperatureInquiry`. Only `PtzOpticsG2` and `PtzOptics30X` carry it, because only the PTZOptics G2 one-byte `90 50 pq FF` reply is sourced; `PtzOpticsG3` and `SonyBRCH900` keep the color-temperature setters. Dynamic calls on the other profiles return `FeatureNotSupported` before any I/O. Read the bytes with `raw::Inquiry` and `81 09 04 20 FF`. |
| `HasImageProcessing` arrived through a blanket implementation | Built-in profiles receive an explicit implementation only when at least one source-backed image surface exists. A downstream profile must opt in deliberately. |
| `SonyBRC300` in 2.0.0-rc.1 advertised typed backlight support but could not construct `image()` | Upgrade to rc.2, which restores `camera.image().backlight()` and `camera.image().set_backlight(...)` while keeping every other image row independently capability-gated. |
| `CapabilityRange` serde accepted `min > max`, and checked scalar wrappers could deserialize invalid values | Deserialization now validates the same invariants as construction; handle the serde error and repair invalid persisted data before retrying. |

### Wire corrections and removed ambiguous inquiries

2.0 deliberately does not preserve several incorrect 1.x byte sequences. The
complete audited delta is recorded in the
[immutable rc.1 CHANGELOG entry for "Breaking wire corrections versus 1.x"](https://github.com/GrantSparks/grafton-visca/blob/ad6982fb95d3d13c3dd6ec496fa59b495fe3107d/CHANGELOG.md#L682-L725); the migrations a
caller can observe directly are:

| 1.x assumption | 2.0 destination |
| --- | --- |
| Autofocus sensitivity Low/Normal/High encoded as `00/01/02` | The sourced wire values are `03/02/01`. Keep semantic enum values in application state rather than treating an integer cast as a stable wire code. |
| Bright Direct used `04 0D`, and the 2.0 preview exposed byte-identical `Brightness::SetLevel` / `Brightness::Direct` variants | Use only `Brightness::SetLevel` or the noun method `brightness_set`. They encode the sourced direct register `04 4D`; `Brightness::Direct` and `brightness_direct` are removed, while `04 0D` remains only the reset/up/down family. |
| UpRight limit corner was `03` | It is `01` in limit set and clear frames. |
| Focus-zone inquiry was `09 04 3C` | It now matches the focus-zone register at `09 04 AA`. |
| `FocusZone` had exactly Top/Center/Bottom (`00/01/02`), and the inquiry rejected any other reply | `FocusZone` is `#[non_exhaustive]` and adds `Zone03` (`03`), which PTZOptics G2 firmware reports, accepts and reads back, though no vendor source names it. Add a wildcard arm to exhaustive matches. Sending is gated per profile by `Capabilities::focus_zones` (`Focus::FOCUS_ZONES`): `PtzOpticsG2` and `PtzOptics30X` admit `Zone03`, `PtzOpticsG3` and custom profiles default to Top/Center/Bottom, and an unlisted value fails with `Error::InvalidParameter { parameter: "focus_zone", .. }` before any I/O. Decoding `03` is never gated (#795). |
| Picture-effect inquiry was `09 04 32` | It is `09 04 63`. |
| USB-audio inquiry was `09 04 7A`, and on was decoded as `03` | It is the vendor frame `2A 02 A0 04`; `02` means on and `03` means off. |
| A final data byte of `FF` doubled as the terminator | Preset 255 and Direct Menu values ending in `FF` now contain both the data byte and a separate terminating `FF`. Direct Menu rejects `FF` followed by an address byte because that sequence would begin a second frame. |
| `TiltSpeed` stopped at `0x14` for every camera | The value type admits through `0x18`; the selected profile still rejects values above its own limit. PTZOptics remains capped at `0x14`, while BRC-300 position framing allows one `VV` through `0x18`. |
| 2D/3D noise-reduction level zero was invalid | Zero is the sourced off value. The admitted domains are `0..=5` for 2D and `0..=8` for 3D, subject to profile support. |
| Sony device-setting/control payload types were `01 02`, `01 20`, `01 21` | The R7 header types are `01 20`, `02 00`, `02 01`. This affects custom-envelope code that inspected raw Sony headers. |

If an application persisted `AutoFocusSensitivity as u8`, migrate stored
values before constructing the 2.0 enum:

| Stored 1.x integer | Semantic setting | 2.0 wire value |
| ---: | --- | ---: |
| `0` | Low | `3` |
| `1` | Normal | `2` |
| `2` | High | `1` |

Serde's named `low` / `normal` / `high` forms do not need translation. Do not
feed an old integer directly to the 2.0 wire decoder: old `1` meant Normal but
now decodes as High, and old `2` meant High but now decodes as Normal.

The BRC-300 profile keeps its R12-specific position grammar:
`8x 01 06 02/03 VV 00 0Y 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF`. It is not the common
two-speed `VV WW` layout. Since the public request still accepts separate pan
and tilt speed wrappers, pass equal values for BRC-300 absolute/relative
positions; unequal values are rejected rather than silently discarding one.

The source audit also removed public names whose registers or meanings were
not established:

| Removed 1.x API | 2.0 migration |
| --- | --- |
| `ResolutionInquiry`, `ResolutionMode`, `image().resolution()` | No typed replacement. The claimed `09 04 63` register is the sourced Picture Effect inquiry. Use a model-specific `raw::Inquiry` only when that camera's documentation establishes a resolution query. |
| `BlackWhiteInquiry`, `BlackWhiteModeInquiry`, `BlackWhiteMode`, `image().black_white*` | Use `image().picture_effect()` when BlackAndWhite/Off is what the profile documents. Otherwise use a model-specific raw inquiry with evidence. |
| `NrLevelInquiry`, `NrModeInquiry`, `NrSpeedInquiry`, `NoiseReductionLevel`, `NoiseReductionMode`, `NoiseReductionSpeed` and aggregate NR accessors | Use the dimension-specific 2D-mode, 2D-level, and 3D-level API described below. |
| `PictureEffectMode::{Negative, Sepia, Sketch, Emboss, Mosaic}` | Built-ins expose only sourced `Off` and `BlackAndWhite` names. Use `PictureEffectMode::Unknown(raw)` only when the selected model's documentation establishes that raw value. |

These are breaking source and wire corrections, not aliases: code that names a
removed item must choose the model-specific behavior it intended.

### Noise-reduction controls and independent gates

The surviving typed NR surface is split by direction and dimension. The
inquiries keep `HasNoiseReduction2D` / `HasNoiseReduction3D`; controls require
the independent `HasNoiseReduction2DControl` /
`HasNoiseReduction3DControl` markers (and the matching dynamic
`TypedSupportSurface::NoiseReduction2DControl` /
`TypedSupportSurface::NoiseReduction3DControl` checks). Do not replace one
bound with the other: an inquiry permission is not a control permission.

| Earlier/removed vocabulary | 2.0 destination |
| --- | --- |
| 2D mode setter | `camera.image().set_noise_reduction_2d_mode(...)` |
| 2D level setter / disable helper | `camera.image().set_noise_reduction_2d(...)` / `camera.image().disable_noise_reduction_2d()` |
| 3D level setter / disable helper | `camera.image().set_noise_reduction_3d(...)` / `camera.image().disable_noise_reduction_3d()` |
| 2D mode and 2D/3D level inquiries | `camera.image().noise_reduction_2d_mode()`, `.noise_reduction_2d()`, and `.noise_reduction_3d()` under their inquiry markers |
| `HasNoiseReduction`, `TypedSupportSurface::NoiseReduction`, `noise-reduction` serde tag, `noise_reduction_level`, `noise_reduction_mode`, or aggregate modes/speeds/strength mappings | **No typed replacement.** These aggregate APIs remain removed; use a deliberately specified raw request only for an unsupported profile or aggregate vendor dialect. |

The four individual NR markers are emitted only for `PtzOpticsG2`,
`PtzOpticsG3`, and the explicitly legacy PT30X SDI/NDI G2 profile
`PtzOptics30X`. This does not follow from a PTZOptics family name and does not
cover Move, Link, or newer 30X models. R14 documents command inputs separately
from current query outputs, so migration must not assume a set-to-query numeric
round trip; see [the source-specific NR contract](visca_reference.md#78-image-processing).

### `Axes` → `AffectedAxes`: rename and `ALL` meaning change

This axis change is both a rename and a semantic change, and the semantic half
ports cleanly then fails on the device — so port it deliberately.

1.x's axis type was `Axes` (`grafton_visca::camera::Axes`), whose `ALL` was the
three mechanical movement axes: pan/tilt, zoom, and focus (`Axes::default()` was
`Axes::ALL`). 2.0 renames the type to `AffectedAxes` and gives it five axes —
pan/tilt, zoom, focus, iris, and ND filter — so `AffectedAxes::ALL` now means
all five. A mechanical `Axes::ALL` → `AffectedAxes::ALL` rename therefore
silently widens the selection.

A motion observation only queries the axes it is given, and preparation
requires the profile to declare a position inquiry for every selected axis.
`AffectedAxes::ALL` therefore now demands iris **and** ND-filter position
inquiries. No built-in profile declares both, so
`motion().is_moving(MotionQuery::new(AffectedAxes::ALL))`,
`wait_until_idle(IdleWait::new(AffectedAxes::ALL, ..))`, and any operation
declaring `ALL` fail on all nine built-in profiles with
`Error::FeatureNotSupported` before a frame is sent. The port compiles; it just
never runs.

Use `AffectedAxes::MOVEMENT` for "wait for everything that moves". It is
exactly 1.x's three `Axes::ALL` axes and is what `MotionQuery::default()`,
`IdleWait::default()`, and the named `IdleWait` presets already select. Reserve
`AffectedAxes::ALL` for an explicit profile that declares both iris and
ND-filter position inquiries.

## Requests, inquiries, and operations

| 1.x category | 2.0 destination |
| --- | --- |
| Concrete async `_op` methods (`pan_tilt_*_op`, `set_*_op`, `preset_recall_op`, and similar) | Construct the typed request and call `submit`; use the returned targeted/applied-only handle. |
| `start_*`, `*_and_wait`, `_result`, and `*_op` twins | One noun method for ordinary completion, or one `submit` call for lifecycle control. No aliases or result twins. |
| `await_completion` | `applied`; use `settled` only on a targeted operation. |
| `InFlight::await_applied(timeout)` / `BlockingInFlight::await_applied(timeout)`, and `await_settled(timeout)` | `Operation::applied()` / `settled()` for the request's configured deadline, or `applied_with_timeout(timeout)` / `settled_with_timeout(timeout)` for an explicit one. `settled*` exists only on a targeted operation and returns `Settlement` evidence. Stable samples are attributed to that operation only while no later conflicting motion has been admitted; they do not prove arrival at its endpoint. |
| `send_command_with_id(&cmd) -> (CommandId, _)` followed later by `cancel(command_id)` | `camera.submit::<K, _>(&op)` returns an `Operation<K>` **handle**; hold it and call `operation.cancel()`, which waits for the cancellation's `CancellationOutcome`. The handle itself is the cancellation authority. |
| `CommandId` used as a cancellation key | `OperationId` (from `operation.id()`) is read-only observability only; it can no longer authorize waiting or cancellation. Cancel through the owning `Operation` handle. |
| `cancel_command(ViscaSocket)` and `cancel_socket(ViscaSocket)` (cancel by socket) | There is no public cancel-by-socket call — socket cancellation is owner-only and is driven by cancelling the specific `Operation`. `ViscaSocket` still exists (`grafton_visca::ViscaSocket`) as a value type but is not a cancellation entry point. To force motion to end, submit the typed STOP. |
| `InFlightDyn`/legacy dynamic operation wrappers | `DynTargetedOperation` and `DynAppliedOperation`. Applied-only handles have no settled operation. |
| Dropping an operation handle | Unchanged from 1.x: drop is `detach` and never stops hardware. See [Drop never stops hardware](#drop-never-stops-hardware) for the scoped stop-on-exit pattern. |
| Raw `command::RawInquiryPayload`/untyped response assumptions | `raw::Plain`, `raw::Inquiry`, `raw::Targeted`, or `raw::AppliedOnly`, with an explicit response parser/spec. |
| `ViscaCommand` response-associated-type extensions | The typed `Request`/`Inquiry`/`OperationCommand` contract and `ResponseParser` for custom decoding. |
| Unvalidated vendor opcodes that 1.x let through because it validated nothing on send (e.g. vendor tally mode `0A 02 02 0p` on a PTZOptics profile) | The typed and `execute` routes validate against profile capability, so a profile without the typed surface refuses the opcode (tally mode needs `HasTally`). Send a firmware-confirmed vendor opcode through `raw::Plain` instead. |
| Plain requests submitted as operations or operations without affected axes | Match the request class exactly: `execute` for plain, `inquire` for inquiry, and `submit` for a typed operation with non-empty affected axes. |
| Caller-selected lifecycle IDs, retry class, target, or settlement metadata | Owner-derived preparation metadata. Callers select a request, a timeout, and a scheduling class; they do not select protocol identity or queue positions. |
| `runtime::Priority` and the `*_priority` camera methods | `SubmissionClass` and the `with_submission_class` / `set_submission_class` camera-view surface. `ControlClass::Urgent` is now intrinsic safety metadata, not caller QoS. See [Submission priority](#submission-priority). |

The built-in command classification remains one closed semantic ledger. A
custom request must declare its class explicitly; wire opcode or response shape
does not infer lifecycle semantics.

### Type-erasing operation handles

1.x `InFlight` was borrow-oriented, so downstream crates could put it behind an
object-safe trait whose `await_applied`, `await_settled`, and `cancel` methods
took `&self`. 2.0 waits borrow too, exclusively: `applied`, `settled`, and
`cancel` take `&mut self`, so such a trait ports by changing the receiver to
`&mut self` and boxing the borrowed future. An applied-only operation has no
settlement method; its wrapper reports settlement as unsupported.

```rust
#[cfg(feature = "async")]
mod erased {
    use std::{future::Future, pin::Pin};

    use grafton_visca::{
        completion::{AppliedOnly, Targeted},
        CancellationOutcome, Error, Operation, Settlement,
    };

    type Wait<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

    pub trait Observed: Send {
        fn applied(&mut self) -> Wait<'_, ()>;
        fn settled(&mut self) -> Wait<'_, Settlement>;
        fn cancel(&mut self) -> Wait<'_, CancellationOutcome>;
    }

    impl Observed for Operation<Targeted> {
        fn applied(&mut self) -> Wait<'_, ()> {
            Box::pin(Operation::applied(self))
        }
        fn settled(&mut self) -> Wait<'_, Settlement> {
            Box::pin(Operation::settled(self))
        }
        fn cancel(&mut self) -> Wait<'_, CancellationOutcome> {
            Box::pin(Operation::cancel(self))
        }
    }

    impl Observed for Operation<AppliedOnly> {
        fn applied(&mut self) -> Wait<'_, ()> {
            Box::pin(Operation::applied(self))
        }
        fn settled(&mut self) -> Wait<'_, Settlement> {
            Box::pin(async { Err(Error::NotSupported) })
        }
        fn cancel(&mut self) -> Wait<'_, CancellationOutcome> {
            Box::pin(Operation::cancel(self))
        }
    }
}
```

What does not port is sharing: one handle cannot be waited on from two places
at once. Wait from the task that holds it, and pass `&mut` where 1.x passed
`&`.

## Submission priority

1.x's `runtime::Priority` is gone. 2.0 separates caller-selected ordinary-work
QoS (`SubmissionClass`) from a request's intrinsic classification
(`ControlClass`). That separation prevents submission APIs from manufacturing
or demoting the urgent lane used by stops and protocol cancellation.

| 1.x `runtime::Priority` | 2.0 submission choice |
| --- | --- |
| `Priority::Low` | `SubmissionClass::Background` |
| `Priority::Normal` | `SubmissionClass::Normal` |
| `Priority::High` | `SubmissionClass::User` |
| `Priority::Critical` | No general submission override. Typed stops and owner-issued protocol cancellation are intrinsically `ControlClass::Urgent`; ordinary traffic may be raised only to `SubmissionClass::User`. |

The dispatch rule remains highest occupied class first and admission order
within a class; a class decides only which *queued* request is written next.
1.x had no per-request classification, so a handle's priority was the only
signal and everything a handle submitted sat in one lane. In 2.0 every request
already carries an intrinsic class — ordinary control is `Normal`, drives and
absolute moves are `User`, and the typed stops plus owner-issued protocol cancellation are `Urgent`
— so an emergency stop preempts queued work with no API call at all. Public QoS
can move ordinary traffic among the lower three lanes but cannot cross that
safety boundary. On a raw profile an intrinsically `Urgent` stop may also cross
one un-acknowledged raw command and create one explicit two-candidate
safety-lane state (#714). An ACK observed while both raw candidates are open
binds to neither; either operation may therefore report
`UnsequencedCommandUnconfirmed` even though the stop bytes reached the camera.
Likewise a stop is never held back by a raw `NoReply` command's hold (#700):
it is written at once, and a socketless error that the `NoReply` command could
also have sent binds to neither, so the stop may report
`UnsequencedCommandUnconfirmed`.
Both facades queue work that cannot be written yet; nothing fails with a
contention error.

| 1.x call | 2.0 call |
| --- | --- |
| `camera.set_command_priority(Priority::Low)` | `camera.set_submission_class(Some(SubmissionClass::Background))` |
| `camera.command_priority()` | `camera.submission_class()` — returns `Option<SubmissionClass>`, where `None` means "use each request's intrinsic class" |
| `camera.execute_with_priority(cmd, Priority::High)` | `camera.with_submission_class(SubmissionClass::User).execute(&cmd)` |
| `camera.execute_with_priority(stop, Priority::Critical)` | Submit the typed stop normally; it is intrinsically `ControlClass::Urgent`. |
| — (no 1.x equivalent) | `camera.with_submission_class(class)` derives a view whose `execute`, `inquire`, `submit`, and noun methods all use that ordinary-work class. |
| `BlockingClient` priority methods | The same names on `blocking::Camera`; `blocking::CameraSession` exposes forwarding default helpers and returns that `Camera` from `camera()` |
| — (no 1.x equivalent) | `with_submission_class` and `set_submission_class` are identical on typed, dynamic async, and native blocking dynamic camera views. |

Three behavioural differences are worth reading before porting:

- **No public QoS override can weaken an urgent request.** A handle set to
  `Background`, and a view derived with
  `with_submission_class(SubmissionClass::Background)`, still submits `PanTiltStop`, `ZoomStop`,
  `FocusStop`, and owner-issued protocol cancellation as `ControlClass::Urgent`.
  Nor can ordinary work manufacture the urgent lane: `SubmissionClass` has no
  `Urgent` variant, and the raw escape hatch's `raw::Policy` rejects
  `ControlClass::Urgent` at construction (a raw caller who needs preemption
  issues the typed stop instead). The urgent lane is reachable only by the
  crate's own stops and owner-issued cancellation.
- **Inquiries are covered.** 1.x kept inquiries at a fixed polling priority; in
  2.0 they share the same four lanes, so a handle demoted to `Background` moves
  its telemetry reads out of the way as well as its commands. Owner-internal
  traffic — settlement polling behind `settled()`, and the observation inquiries
  behind `motion()` — keeps its own built-in class.
- **The default is per handle, not per session.** Cloning an async `Camera`
  copies the current value and then diverges, and two views taken from one
  `Session` are independent. This matches 1.x.

`runtime::testing::Priority` has no 2.0 equivalent, because 2.0's tests do not
need one: `SubmissionClass` is public in every build configuration, so a test
names the QoS through the same API an application uses, and the crate's own
lane-ordering tests (`tests/issue_630_submission_class_*.rs`) assert the
resulting dispatch order at the transport boundary rather than reaching into the
scheduler. For deterministic scheduling in downstream tests, use the
`test-utils` transports and executors.

## Drop never stops hardware

Dropping an operation handle is exactly `detach`. It relinquishes the observer
and nothing else: the owner keeps the protocol lifecycle, never reads a dropped
handle as cancellation, and no STOP reaches the camera. An early `?`, a panic
unwinding past a live handle, or a forgotten binding therefore leaves physical
movement running until something else ends it.

**This is not a 2.0 change.** 1.x behaved identically — `src/camera/inflight.rs`
on the 1.x line documents "drop == detach ... Dropping never stops the command",
pinned there by `tests/issue_539_blocking_handle_test.rs`. Nothing to migrate;
it is documented here because the consequence is physical and easy to assume
otherwise.

To end motion, submit a stop: `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
`camera.focus().stop()`, or `camera.motion().stop_all_motion()`. `cancel` records
protocol cancellation and does not by itself prove motion ended.

`stop_all_motion()` is one owner halt with a common deadline for preparation,
admission, STOP dispatch, and observation. An outer error means the owner did
not accept the halt. After acceptance, `HaltReport` independently reports
`Unsupported`, `Applied`, or `Failed(error)` for pan/tilt, zoom, and focus.
Inspect every supported axis result, or call `into_result()` when the first
failure is sufficient. For example, the blocking form is
`camera.motion().stop_all_motion()?.into_result()?;`.

The fence is established when the owner accepts the halt. Older declared motion
on supported axes is suppressed if still queued, including work retained in an
ingress channel; already-written attempts retain correlation and uncertainty,
while their future retries are suppressed. Submissions ordered after acceptance
remain eligible. Concurrent calls are ordered by this acceptance boundary, not
by when a caller begins preparation. Independent STOP admission and dispatch respect protocol socket, ACK-correlation,
and pacing gates; they do not wait for another STOP's completion. Each STOP retains the same absolute cutoff
and cannot begin a late write. Once the owner claims acceptance, the caller may
finish waiting for its bounded bookkeeping reply; that never renews the STOP or
observation budget. Applied STOPs provide protocol evidence, not physical feedback.

Custom plain requests may declare `Request::motion_axes()`; `raw::Plain` has
`with_motion_axes`. Operation requests already declare their affected axes.
Undeclared raw/custom requests remain outside the fence. An axis declaration
never grants reserved STOP priority or changes the command's wire semantics.

Superseded motion fails with `Error::MotionSuperseded { axes, context }`.
`context` is a `FailureContext` (also returned by `failure_context()`): its
certainty is `NotAccepted` when the request never reached the camera — it was
still queued, or every written attempt was rejected before ACK — and
`Unconfirmed` only when an earlier attempt may have taken effect. A submission
the fence rejects at admission reports stage `PreAdmission`. Code that built
the error with `Error::motion_superseded(axes)` now passes the context as a
second argument; patterns that already use `{ .. }` are unaffected. In earlier
release candidates the variant had no context and always reported `Unconfirmed`.

A typed STOP that the camera refuses with `0x41` is reported at once as
`Error::CommandNotExecutable` and is not resent. A PTZOptics G2 in auto-focus
mode answers the halt's focus STOP that way (`90 6y 41 FF`, naming its next
free socket), so `HaltReport::focus` is `Failed(CommandNotExecutable)` there;
`into_result()` returns that error. Ordinary movement keeps its bounded `0x41`
retry. No retry path writes a command again after the camera acknowledged it,
on either envelope: an error that arrives after the ACK ends the request with
`Error::CommandFailedAfterAck { source }` (the camera's exact error as
`source`, `Terminal`/`Unconfirmed`, never retryable), because a relative move
or preset may have partly executed; and a Sony command that is ACKed and then
silent ends with an unconfirmed `Timeout` instead of the same-sequence resend
earlier release candidates made (#795). Code that matched, say,
`Err(Error::CommandNotExecutable)` for a rejection after the ACK now matches
`Err(Error::CommandFailedAfterAck { .. })` and reads `source`.

On a raw-VISCA stream (TCP or serial) a command that ends
`UnsequencedCommandUnconfirmed` before its ACK still owes that answer. Later
ordinary commands to the camera wait for it through the owing command's
ambiguity window and then fail unwritten with
`Error::CommandCorrelationLost { camera }` until it arrives or the camera
answers a later inquiry or STOP, while STOPs are always sent; the late answer
is discarded instead of acknowledging the next command (#795).

A raw `RawReplyShape::CompletionOnly` command is exclusive on its camera until
its completion or rejection arrives: inquiries and ordinary commands to that
camera now queue behind it, and on a raw stream keep queuing after its own
deadline ends it unconfirmed (failing with `InquiryCorrelationLost` or
`CommandCorrelationLost` once its ambiguity window passes). STOPs are never
held back, and may now be written behind it on a stream (#795).

A raw `RawReplyShape::NoReply` command is no longer "never blocked" on a raw
stream: like an ordinary command it waits while an earlier command's answer
is owed, and fails unwritten with `CommandCorrelationLost` once that lane
latches, because its own possible rejection could not be told from the owed
answer. Its possible rejection is in turn owed until the camera's next answer
(a later ACK, inquiry reply, or completion): a `CompletionOnly` command to the
camera waits for that, and fails with `CommandCorrelationLost` instead of
timing out if the `NoReply` command's ambiguity window ends first. STOPs and
inquiries are never held back by it (#795).

A later admitted motion on overlapping axes supersedes an earlier operation's
unfinished polled settlement, even if the later motion is cancelled before it
writes. Rejected submissions and other axes or targets do not supersede it.
`Error::SettlementSuperseded` preserves the earlier operation's application
result; it does not prove that movement failed or authorize replay. Successful
cached evidence and profile-declared exact completion remain valid after later
motion. Polling failures retain their source in
`Error::SettlementObservationFailed`, with `Observation/Unconfirmed` context for
the original operation. A polling inquiry's conclusive failure does not make
replaying the original move safe.

Borrowing waits classify outcomes by the injected owner clock at delivery.
Delivery before or exactly at the observer deadline can satisfy that wait;
a later delivery is cached for a subsequent wait. An on-time cancellation
failure therefore remains visible even when a late original completion is
ready when the waiter resumes.

### A refused `cancel` leaves the handle observing

Cancelling a command that has already been written needs profile support for the
standard VISCA socket-cancel command. The PTZOptics profiles (`PtzOpticsG2`,
`PtzOpticsG3`, `PtzOptics30X`) are the built-in profiles without it, and there
the owner refuses with `Error::NotSupported` and
deliberately leaves the original request scheduled and able to complete — the
same per-phase contract 1.x had, where a still-queued command cancels locally on
every profile.

`cancel` borrows the handle, so a refusal is a plain `Error::NotSupported` and
the handle keeps observing the original:

```rust
#[cfg(feature = "async")]
async fn recover_refused_cancel(
    camera: &grafton_visca::Camera<grafton_visca::camera::profiles::PtzOpticsG2>,
    mut operation: grafton_visca::Operation<grafton_visca::completion::AppliedOnly>,
) -> Result<(), grafton_visca::Error> {
    use grafton_visca::Error;

    match operation.cancel().await {
        Ok(_outcome) => Ok(()),
        Err(Error::NotSupported) => {
            // Still observable — and the axis is stopped the usual way.
            camera.zoom().stop().await?.applied().await?;
            operation.applied().await
        }
        Err(error) => Err(error),
    }
}
```

The [v1.1.0 `InFlight` implementation](https://github.com/GrantSparks/grafton-visca/blob/v1.1.0/src/camera/inflight.rs)
made async and dynamic `cancel` borrow the handle and documented that its exact
response could still be awaited, so a `NotSupported` result also left the
caller holding the handle. 2.0 keeps that shape on every facade. The same
v1.1.0 implementation made the blocking handle consume on this path; 2.0 does
not reproduce that asymmetry.

### Scoped stop-on-exit guard

To bound movement by a scope rather than by an explicit call on every path,
write a guard whose own `Drop` submits the typed STOP. This is caller-owned
code — the library deliberately offers no such type, so the policy, the axes,
and the failure handling stay yours.

Every complete Rust snippet on this page other than the explicitly marked
`rust,ignore` adapter sketch is compiled by the crate's own test suite. That
sketch intentionally omits application-specific profile/capability bounds; the
`#[cfg(feature = "...")]` attributes below are load-bearing and name the Cargo
feature a compiled snippet needs.

```rust
use grafton_visca::blocking::Camera;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::command::PanTiltDirection;
use grafton_visca::types::{PanSpeed, TiltSpeed};
use grafton_visca::Error;

struct StopPanTiltOnExit<'a> {
    camera: &'a Camera<PtzOpticsG2>,
}

impl Drop for StopPanTiltOnExit<'_> {
    fn drop(&mut self) {
        // `Drop` cannot report a failure and may run while unwinding, so the
        // stop is best effort — as in any scope guard.
        if let Ok(mut stop) = self.camera.pan_tilt().stop() {
            let _ = stop.applied();
        }
    }
}

fn bounded_drive(
    camera: &Camera<PtzOpticsG2>,
    direction: PanTiltDirection,
    pan: PanSpeed,
    tilt: TiltSpeed,
    do_fallible_work: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    // Hold the guard for the region that must stay bounded.
    let _stop_on_exit = StopPanTiltOnExit { camera };
    let mut drive = camera.pan_tilt().move_direction(direction, pan, tilt)?;
    do_fallible_work()?; // an early `?` here still stops pan/tilt
    drive.applied()
}
```

That guard runs on every way out of the scope, including a panic unwinding
through it.

`Drop` cannot await, so the async form is a wrapper rather than a guard type:
run the fallible region, stop, then propagate the body's result.

```rust
#[cfg(feature = "async")]
mod async_form {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::command::PanTiltDirection;
    use grafton_visca::types::{PanSpeed, TiltSpeed};
    use grafton_visca::{Camera, Error};

    pub async fn bounded_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
        let result = drive_up(camera).await;
        if let Ok(mut stop) = camera.pan_tilt().stop().await {
            let _ = stop.applied().await;
        }
        result
    }

    async fn drive_up(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Up, PanSpeed::new(6)?, TiltSpeed::new(6)?)
            .await?
            .applied()
            .await
    }
}
```

**The two forms do not cover the same exits.** The wrapper covers exactly the
two ways `drive_up` can *return*: `Ok` and `Err`. It does not cover a panic
inside the body, and it does not cover the caller dropping the `bounded_drive`
future before it completes — a `select!` loser, a `tokio::time::timeout` that
expires, an aborted task. In both of those cases the wrapper's stop is simply
never reached and the camera keeps moving. The synchronous guard does cover
both, because `Drop` runs while unwinding and runs when the value goes out of
scope for any reason. If the async path must survive cancellation, hold a
guard that submits the STOP through a channel or a detached task from its own
`Drop`, or bound the movement at the camera instead of at the future.

Both forms are demonstrated end to end in `examples/operation_handles.rs` and
`examples/operation_handles_async.rs`.

## Values, optional features, and internal modules

| 1.x category | 2.0 destination |
| --- | --- |
| Raw normalized floats, root normalized helpers, `ZoomPosition` float conversions | Checked public values such as `UnitInterval::new/try_from`, `ZoomPosition`, and profile-aware noun methods. |
| `inquiry_conversions` module (`ZoomDomain`, `ZoomPositionExt`, `PanTiltPositionRaw`, `PanTiltPositionDeg`, `zoom_from_normalized`) | `ZoomDomain`, `PanTiltPositionDeg` and `zoom_from_normalized` remain. `ZoomPositionExt::normalize_with_max` is the inherent `ZoomPosition::normalize_with_max`, and `ZoomPositionExt::from_normalized` is `zoom_from_normalized`. `PanTiltPositionRaw` is `camera::PanTiltPosition`. The profile-less `as_degrees()` / `PanTiltPositionDeg::to_raw()` / `PanPosition::{from_degrees,to_degrees}` helpers applied PTZOptics G2 geometry (14.4 units per degree) to every camera and are removed: use `PanTiltPosition::as_degrees_with_profile`, `PanTiltPositionDeg::to_raw_with_profile`, or `PanTiltCoordinateConversion` from `ProfileSpec::pan_tilt_coordinates()`. |
| `capabilities::Capabilities` runtime discovery (`from_profile::<P>()`, `supports_typed`, zoom/magnification helpers) | Preserved: `Capabilities` stays at `grafton_visca::capabilities::Capabilities`. Reach it with `camera.capabilities()` (static or dynamic) or `Capabilities::from_profile::<P>()`; build a runtime profile from `Capabilities::runtime_baseline(..)` plus `ProfileSpec::builder`. |
| Model-aware constructors that embed a profile in a value | Plain checked values plus the profile-gated camera noun; profile validation belongs at preparation. |
| Generic optional accessors or unsupported PTZOptics/Sony controls | Compile-time `Has*` gates; dynamic callers inspect capability support. Unsupported controls are not exposed through metadata fallback. |
| Direct PTZOptics ND filter, Motion Sync, variable-speed, Sony color-temperature, or legacy quality controls | The matching supported noun only when its profile marker permits it; otherwise use a raw extension deliberately. |
| Public `camera::*`, `command::*`, `protocol::*`, response, cache, runtime, or transport implementation modules | Supported root/module exports and owner methods. Implementation submodules are not extension points. |
| `diagnostics::Diagnostics`/probe-style compatibility API | `Session::metrics`, and `subscribe_diagnostics` on either facade. |
| Legacy mutable `cache::StateCache` | Owner-backed read-only root `StateCache`; use `target()` and `value(StateKey)`. |
| 1.x `Camera::set_timeout_config` / `timeout_config` | `Session::set_tuning` / `tuning` (and the same pair on `CameraSession`), taking an `OperationalTuning`. Standard `CameraConfig` construction uses `with_tuning`. See [Reconfiguring timeouts at runtime](#reconfiguring-timeouts-at-runtime). |

### Removed compatibility and helper vocabulary

Several 1.x names described implementation machinery rather than a stable
camera operation. They have no name-preserving alias in 2.0:

| 1.x API | 2.0 destination |
| --- | --- |
| `mode::{Mode, Blocking, Async}` and the public `mode` module | Select the `blocking` and/or `async` Cargo feature and use `blocking::Camera<P>` or async `Camera<P>`. The two facades can coexist; mode is no longer a camera type parameter. |
| `GenericViscaCam<T>`, `NearusBRC300Cam<T>`, `PtzOptics30XCam<T>`, `PtzOpticsG2Cam<T>`, `PtzOpticsG3Cam<T>`, `SonyBRC300Cam<T>`, `SonyBRCH900Cam<T>`, `SonyEVIH100Cam<T>`, and `SonyFR7Cam<T>` | Spell the profile directly: async `Camera<Profile>` or blocking `blocking::Camera<Profile>`. Construction returns a session; select the view with `session.camera::<Profile>()` or `camera_for::<Profile>(target)`. |
| Items formerly obtained incidentally from the broad `prelude` contents | The async and blocking preludes now contain their documented facade-specific quick-start sets. Import extension contracts such as `Request`, `Inquiry`, `OperationCommand`, `Envelope`, profile builders, and transport traits explicitly from their owning root/module paths. |
| `ClosedSession` and the open/closed session typestate markers | `Session::close(self)` consumes the session and returns `Result<()>`; success is the closed-state proof. Do not retain or pass a closed token. |
| `ResponseFuture` | Keep the returned `Operation<K>` and call its terminal method, or await the typed `inquire`/`execute` call directly. The owner retains response routing; there is no public boxed response-future alias. |
| `RawSender` | Construct `raw::Plain`, `raw::Inquiry<R>`, `raw::Targeted`, or `raw::AppliedOnly` and pass it to the matching camera `execute`, `inquire`, or `submit` method. Raw values still carry explicit timeout, retry, control, and reply-shape policy. |
| `submit_continuous` | Express the lifecycle in the request type: implement `OperationCommand<completion::AppliedOnly>` for a custom typed request, or construct `raw::AppliedOnly`, then call `submit::<completion::AppliedOnly, _>`. |
| `CommandBehavior` and `InquiryResponseSpec` | Implement the public `Request` class plus `Inquiry`/`OperationCommand<K>` contract. Custom inquiry decoding belongs in the response type/decoder and `InquiryRoute`; action-versus-inquiry routing is no longer supplied as mutable runtime metadata. |
| `CachedFlipState` | `StateCache::flip_state()` returns the public `command::FlipState` value. |
| Aggregate `PanTiltLimits` cache state | `StateCache::pan_tilt_limits()` returns the most recent `PanTiltLimitUpdate` (corner, optional position, and cleared state). An application that needs the full two-corner rectangle must fold those updates into its own state. |
| `transport::BackoffStrategy` and `RetryAttempt` | Retry scheduling is owner policy, with deterministic equal jitter rather than a caller-selected strategy. Configure conservative bounds with `OperationalTuning::retry_limit` / `retry_timing`; observe attempts through diagnostics/metrics rather than constructing an attempt counter. |
| `Error::{LockPoisoned, ChannelClosed, SocketManagerUnavailable, SocketManagerChannelClosed, ResponseChannelClosed, TransportMismatch, NoTransport, TransportChannelClosed}` | Delete explicit arms for these never-produced variants. Boundary closure is normalized to `RuntimeShutdown`; actual session death is `ConnectionClosed` or `StreamPoisoned`. Prefer `requires_new_session()` for the recovery decision. |
| `Error::{CommandTimeout, CameraBusy, CameraMoving, CameraNotReady, CommandRejected, PresetNotFound, NoResponse, ValidationError, UnknownResponseKind}` | Delete explicit arms for these never-produced variants. Deadlines report `Timeout { context }`, or `ObservationTimeout { operation }` when only your wait expired (see below); VISCA capacity/state failures use `CommandBufferFull`, `NoSocket`, or `CommandNotExecutable`; preset/value validation uses the reachable checked-value errors; capability construction returns `capabilities::ValidationError` directly. Keep a wildcard arm because `Error` remains non-exhaustive. |

For a central application error adapter, prefer `error.kind()` when several
wire-level variants have the same application meaning. Map `ErrorKind::Timeout`
to a request timeout, but decide replay with `error.failure_context()`, not
`is_retryable()`: an `ObservationTimeout` (your wait expired, the request is
still running) is never retryable, and a resubmission is safe only after
`Certainty::NotAccepted` or `Certainty::FailedConclusively`. 1.x had one unit `Timeout` for every deadline; 2.0's
`Timeout` carries a `FailureContext`, so match it as `Error::Timeout { .. }`,
and a custom transport reports an expired read or write with
`Error::io_timeout()`. Map `BufferFull` to a retryable busy/capacity state, and
`NotExecutable` to a current-state or precondition rejection: it can be the
camera's `0x41` response or a local `InvalidState` validation failure. Re-query
or establish the prerequisite before retrying, and retry only when the command's
semantics make that safe. Map `Unsupported` to an unsupported feature; retain a
wildcard because `ErrorKind` is non-exhaustive. Preserve the original `Error`
as the source when possible. Decide whether to rebuild the
connection with `error.requires_new_session()` rather than treating every
`IoClosed` kind as an unexpected disconnect: deliberate `RuntimeShutdown` has
that kind but does not require a replacement session.

Retained public protocol values also changed shape:

| 1.x/earlier RC call | 2.0 call |
| --- | --- |
| `DirectMenuControl::new(control1, control2)` returning the value directly | `DirectMenuControl::new(control1, control2)?`; the constructor rejects a data `FF` followed by an address byte because that would begin a second VISCA frame. |
| `envelope.frame_into(visca, kind, out)` returning framing metadata directly | `envelope.frame_into(visca, kind, out)?`; malformed/non-terminated Sony payloads are validation errors and leave `out` unchanged. An empty message is rejected with `Error::InvalidRequest` (it used to clear `out` and succeed), and with IP addressing every address byte, including the `88` broadcast address, is sent as camera 1 (`81`), because VISCA over IP fixes the camera address at 1. |
| Earlier 2.0 RC matching on `FrameSequence::Lower16(value)` | Match `FrameSequence::MaybeTruncated(value)`. A zero upper half does not prove truncation; the new name preserves that uncertainty while `value()` still returns the numeric value. |
| Tuple construction or one-field matching of `ZoomTarget(position)` | `ZoomTarget::new(position)` for a raw target, or `ZoomTarget::from_normalized(value, domain, profile)?` when normalization provenance must survive until profile validation. |
| `<R as RuntimeSerial>::SerialTransport` | `<R as Runtime>::SerialTransport`; `RuntimeSerial` remains the `connect_serial` extension trait, while the associated transport type lives on `Runtime` so `TransportHandle<R>` stays runtime-paired. |
| `command::FocusSpeed` (a second focus-speed type) | `grafton_visca::FocusSpeed` (also `types::FocusSpeed`), the one type `Focus::FarWithSpeed`, `FocusDrive`, and `focus().far_variable(..)` take. Out-of-range values now report `ParameterOutOfRange`. |
| `Coarse` (`grafton_visca::Coarse`, `types::Coarse`), a second name for `SpeedLevel`, and `PanSpeed::from_coarse`, `TiltSpeed::from_coarse`, `ZoomSpeed::from_coarse`, `FocusSpeed::from_coarse` | `SpeedLevel` and `From`: `ZoomSpeed::from(SpeedLevel::Fast)` or `SpeedLevel::Fast.into()`. The mapping to wire speeds is unchanged. |
| `EncodeError`, a second name for `Error`, in `Request::write_into(..) -> Result<usize, EncodeError>` | `Error`: implement `fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize>`. `#[derive(ViscaInquiry)]` expands to the same signature. |
| `command::Version`, `command::TallyStatus`, `command::NightDayMode`, `command::IrisControl` | `command::VersionInfo` and `command::TallyStatusState`, which the accessors return; the unused `NightDayMode`/`IrisControl` enums are removed. `InquiryData::Version { info }` and `InquiryData::TallyStatus { state }` carry those structs instead of repeating their fields. |
| `SetMotionSyncPreset::new(u8)?`, `SetMotionSyncPreset::from_preset`, `MotionSyncSpeed::from_preset`, and the `motion_sync().set_preset(u8)` accessor | `motion_sync().set_speed(MotionSyncSpeed)`; `SetMotionSyncPreset::new(MotionSyncSpeed)` and `speed() -> MotionSyncSpeed`. `MotionSyncSpeed::new(u8)?` owns the `1..=24` range and `MotionSyncSpeed::from(MotionSyncPreset)` is the one preset mapping. |
| `ShutterSpeed::try_from(Fraction)` | `capabilities.shutter_speed_for(fraction)?`: shutter codes are camera-specific, and the removed table matched no built-in profile. `ShutterSpeed` wraps the `0p 0q` field's byte: `ShutterSpeed::new(u8)` is infallible and `value()` returns `u8`; the profile's table decides which codes a camera accepts. |
| `Fraction { numerator, denominator }` literals and `Fraction::new(n, d)` returning `Self` | `Fraction::new(n, d)` returns `Option<Fraction>` (`None` for a zero denominator) in lowest terms, so `==` is value equality; read it with `numerator()`/`denominator()`. Its text and serde form is `"1/60"`. |
| `capabilities::ShutterSpeed { label, value }`, `RuntimeShutterSpeed { label, value }`, and `ExposureExt::find_shutter_speed` | One `capabilities::ShutterSpeedEntry { exposure: Fraction, value }` for both `Exposure::SHUTTER_SPEEDS` and `Capabilities::shutter_speeds`. Look codes up with `Capabilities::shutter_speed_for`; `find_shutter_speed`'s nearest-code guess is removed. |
| `ColorTemp::try_from(Kelvin(k))` (a second, `2000..=8000` K formula) | Unchanged call; it is now `ColorTemp::from_kelvin(k)` (`2500..=8000` K, nearest 100 K step, `ParameterOutOfRange` outside). `3200` K encodes `0x07`, not `0x0B`. |
| `From<Raw<_>>` for speed and level wrappers (silently clamped to `MIN`, #828) | `T::try_from(Raw(value))?`, which rejects a value outside the type's domain. |
| `PanTiltExt::degrees_to_{pan,tilt}_units(degrees) -> i32` (truncating) | `-> Option<i32>`, rounded like request preparation (`10.05°` on a G2 is `145`, not `144`); `None` for non-finite input. |
| `Zoom::ZOOM_MAGNIFICATION_TO_UNITS`, `Capabilities::zoom_magnification_to_units`, `{magnification_to_zoom_units, zoom_units_to_magnification, max_optical_zoom, max_combined_zoom}` on `Capabilities` and `ZoomExt` | `Zoom::OPTICAL_ZOOM_RATIO` / `Capabilities::optical_zoom_ratio` (`Option<f32>`) and one `capabilities::ZoomScale` (`units(magnification)`, `magnification(units)`), from `caps.zoom_scale()?` or, for a profile that spans several lenses such as the PTZOptics G2 family, `caps.zoom_scale_for_lens(20.0)?` for the installed 20x lens; `zoom_scale()` on such a profile fails with an error naming `zoom_scale_for_lens`. Only `PtzOptics30X` (30x, R3) fixes its lens; the old scales could not reach the nominal ratio (PT30X: `29 × 565 > 0x4000`). Digital magnification has no sourced scale and is rejected. |
| `camera::PanTiltPosition::as_degrees_with_profile` returning `(f64, f64)` | Returns `PanTiltPositionDeg`. |
| `types::{PanPosition, TiltPosition}`, `PanTilt::{AbsolutePositionRaw, RelativePositionRaw, LimitSetRaw}` | `PanTilt::{AbsolutePosition, RelativePosition, LimitSet}` take the raw `i16` wire words directly (an unsigned-centered word `w` is `w as i16`); the value types restated PTZOptics G2 ranges as if they were protocol limits. Profile-aware requests take `Degrees`. |
| `units::Magnification<T>` | Removed; it had no consumer. `ZoomScale::units(f32)` and `ZoomScale::magnification(u16) -> Option<f32>` name the quantity in the method. |
| `ExposureExt::fstop_to_iris_units` | Removed (#828): an uncalibrated placeholder that returned the minimum iris for `NaN`. Use `FStop` → `IrisLevel` and the profile's `iris_range`. |
| `camera::profiles::{G2Gain, G2PresetId}` | Removed. `G2Gain` offered 24 dB, which the G2 profile rejects; use `GainLevel` and `PresetNumber` with the profile's `gain_range`/preset count. |
| `Capabilities::{supports_hue, has_gamma, has_luminance, has_color_temp, has_rgb_gain, has_noise_reduction}` and `ImageProcessing::{SUPPORTS_HUE, SUPPORTS_GAMMA, SUPPORTS_LUMINANCE, SUPPORTS_NOISE_REDUCTION}`, `WhiteBalance::{SUPPORTS_COLOR_TEMP, SUPPORTS_RGB_GAIN}` (#818) | Removed. The range is the fact: `hue_range`, `gamma_range`, `luminance_range`, `color_temp_range` `.is_some()`; manual RGB gain is `red_gain_range` and `blue_gain_range` both `Some`; noise reduction is `has_2d_nr \|\| has_3d_nr`. |
| `Capabilities::{has_iris_control, has_digital_zoom, has_focus_zone, has_exposure_comp, exposure_comp_profile_range}`, `Focus::SUPPORTS_FOCUS_ZONE`, `Exposure::SUPPORTS_EXPOSURE_COMP` | Removed. Use `iris_range.is_some()`, `zoom_range_digital.is_some()`, `!focus_zones.is_empty()` (`Focus::FOCUS_ZONES` defaults to empty) and `exposure_comp_range` (`Exposure::EXPOSURE_COMP_RANGE: Option<CapabilityRange<i8>>`, default `None`). |
| `Presets::PRESET_SPEED_RANGE: CapabilityRange<u8>`, `Capabilities::preset_speed_range: RangeInclusive<u8>` | `Option<..>`: `None` when the model documents no preset recall speed (EVI-H100, Generic VISCA). |
| `Capabilities::{has_motion_sync, max_motion_sync_speed, max_motion_sync_speed_profile}`, `MotionSyncMetadata::{SUPPORTS_MOTION_SYNC, MAX_MOTION_SYNC_SPEED}` | One fact: `MotionSyncMetadata::MOTION_SYNC_SPEED_RANGE: Option<CapabilityRange<u8>>` and `Capabilities::motion_sync_speed_range: Option<RangeInclusive<u8>>`. `None` means no motion sync; `Some(range)` bounds every typed motion-sync speed and must lie in `MotionSyncSpeed`'s `1..=24`. Replace `has_motion_sync` with `motion_sync_speed_range.is_some()`. No built-in profile documents motion sync. |
| `Capabilities::{default_tcp_port, default_udp_port}`, `SupportsTcp::DEFAULT_TCP_PORT`, `SupportsUdp::DEFAULT_UDP_PORT`, `ProfileMetadata::POSITION_INQUIRY_SUPPORT` (#822) | Removed. Ports are `ProfileSpec::transports()` / `CompileTimeProfile::TRANSPORTS` (`CameraConfig::tcp`/`udp` resolve through it and fail to build for a profile that implements `SupportsTcp`/`SupportsUdp` without the port); position inquiries are `CompileTimeProfile::POSITION_INQUIRIES`. |
| `PanTiltExt::validate_{pan,tilt}_speed`, `ZoomExt::validate_zoom_speed`, `FocusExt::validate_focus_speed` returning a clamped `u8` (#823) | Return `Result<u8, ValidationError>` and reject an out-of-range speed, as request validation does. `CapabilityRange::clamp` is removed; `CapabilityRange::validate` is the shared range check. |
| `Error::InvalidPreset { preset, max }` and `Error::invalid_preset` | Removed; nothing produced it. An out-of-profile preset number is `Error::ParameterOutOfRange` with the profile's `0..=highest_preset` bounds. |
| `Error::DecoderNotFound { inquiry_kind, payload_hex }` and `Error::decoder_not_found` | Removed; nothing produces it. A reply of the wrong length is `Error::InvalidResponseLength` for every inquiry and framing. |
| A `retry_timing` override shorter than the profile's bounds, or a `retry_limit` above 3 | Rejected by `validate_tuning`: retry timing may only lengthen and the retry count may only shrink (#828). |
| `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` over TCP or UDP | Serial only: their manuals document RS-232C/RS-422 VISCA and no IP transport. |
| `Presets::MAX_PRESETS`, `Capabilities::max_presets` (documented as a count, used as the highest index) | `Presets::HIGHEST_PRESET` and `Capabilities::highest_preset`: the highest preset memory number, so presets are `0..=highest_preset` and `0` is one preset (a runtime profile with `has_presets` and `highest_preset: 0` now builds). The registry field is `presets: { highest: N, .. }`. |
| Highest preset number of `SonyEVIH100` 6, `GenericVisca` 6, `NearusBRC300` 16 | 5 for all three: R8 and R21 document `CAM_Memory` p = 0 to 5, and Generic VISCA keeps what R8, R12 and R21 share. Presets 6 and above are refused with `ParameterOutOfRange`. |
| `SonyEVIH100` implemented `HasColorTemperature`, `HasRgbTuning`, `HasImageFlip`, `HasImageMirror` | Removed: R8 has no color-temperature mode or `04 20` command, its `04 43`/`04 44` rows are absolute R/B gain (`HasRgbGain`), and image flip is a rear switch with no `04 61`/`04 66` command. Code bounded on those markers no longer compiles for `SonyEVIH100`; dyn calls return `FeatureNotSupported`. |
| `SonyBRC300`, `NearusBRC300`, `GenericVisca` implemented `HasFocusNearLimitInquiry`; `NearusBRC300` implemented `HasSaturationControl` | Removed: R12 and R21 list no `04 28` focus near-limit command or inquiry, and R21 lists no `04 49` saturation (color gain) command. |
| `SonyFR7` and `SonyBRCH900` accepted typed `ShutterSpeed` positions | Refused before any I/O: their shutter tables could not be read (a known unverified fact, not a claim that the cameras lack shutter control); send the shutter command as a `raw::Plain` request. |
| `ExposureExt::validate_iris` reported a profile without iris as `ValidationError::NotSupported("Iris control")` | `NotSupported("iris")`, the same label request validation uses; a position in the profile's iris table gap is `InvalidValue`. |
| `ProfileGroup::{supports_tcp, supports_udp, supports_serial, uses_sony_encapsulation, inquiry_support}` were declared per group | Derived from the members: a transport or encapsulation is reported only when every member has it, and `inquiry_support` is the weakest member's. `ProfileGroup::GenericVisca` therefore reports no TCP or UDP, because `SonyBRC300`, `SonyEVIH100` and `NearusBRC300` are serial-only; ask the `ProfileId` (`supports_tcp()` and friends) for one profile. |
| Positive tilt degrees were down on `SonyBRC300` and `NearusBRC300` (and documented as down for every profile) | Positive tilt is up for every profile, as positive pan is right. The BRC-300 family's tilt scale is now `+208` units per degree, so raw `493D` (up) is about +90° and the sign of every BRC-300/Nearus tilt in degrees flips: negate stored tilt angles and degree-based limits. Raw `i32` positions and other profiles are unchanged. |
| `SonyBRC300`/`NearusBRC300` optical zoom maximum `0x1068`, no digital zoom, no one-push focus | Optical `0x0000..=0x4000` (x12) and optical-plus-digital `0x4000..=0x7F00` (x4), per R12/R21. Normalized optical zoom `1.0` now sends `0x4000`, so normalized positions computed against the old maximum move further; both profiles implement `HasDigitalZoomToggle`, `HasDigitalZoomRange` and `HasOnePushFocus`. |
| `noise_reduction_2d_mode()` required `HasNoiseReduction2D` and `set_noise_reduction_2d_mode` required `HasNoiseReduction2DControl` | Both require `HasNoiseReduction2DMode` (`TypedSupportSurface::NoiseReduction2DMode`, tag `noise-reduction2-d-mode`); the 2D markers now cover the `04 53` level only. The PTZOptics profiles carry all three; `SonyEVIH100` carries the 2D level pair without the mode. Generic code that calls the mode methods adds the bound; a runtime profile that sets the mode lists the surface. |
| A runtime profile could grant `PtzOpticsPresetRecallSpeed` without a preset speed range and fail every recall-speed request | `ProfileSpecBuilder::build` rejects the grant: the surface's metadata requires `preset_speed_range`. |

Serialization features (`serde`, `schemars`, `ts-rs`) remain opt-in data-shape
features. They do not reopen private modules or create a second semantic
registry. `test-utils` is for deterministic tests, not production construction.

With `serde`, checked data stays checked across the wire boundary:
`CapabilityRange` now requires ordered `min`/`max` endpoints and rejects
`min > max` during deserialization. Validated public scalar wrappers and values
declared by `visca_range_type!` deserialize through their checked `TryFrom`/
`new` paths, so an out-of-range scalar is rejected rather than constructing an
invalid wrapper.

#### Checked newtypes and derive attributes (#824)

`visca_range_type!` and `#[derive(ViscaValue)]` expand through one generator,
so every checked newtype, including the crate's own value types, has the same
constructor, bounds, accessor and error contract:

| Earlier RC | 2.0 |
| --- | --- |
| `visca_range_type!` `MIN`/`MAX` typed as the inner integer | `MIN`/`MAX` are `Self`; read the raw bound with `T::MIN.value()`. |
| `visca_range_type!` `value(&self)` | `pub const fn value(self)`, usable in `const` items and as `T::value` in iterator adapters. |
| An out-of-range `visca_range_type!` value (for example `command::FocusSpeed::new(8)` or `PresetRecallSpeed::new(25)`) returned `Error::InvalidParameter` | Every range-checked newtype returns `Error::ParameterOutOfRange { parameter, value, min, max }`. Values outside a sparse `valid_values` set still return `Error::InvalidParameter`. `GainLimit`, `SharpnessLevel`, `LuminanceLevel`, `ContrastLevel` and `DynamicRangeLevel` are now declared as ranges, so they also report `ParameterOutOfRange`. |
| `types::BrightnessLevel(u16)` with a `0x00..=0x11` domain, `Exposure::BRIGHTNESS_RANGE: Option<CapabilityRange<u16>>`, `Capabilities::exposure_brightness_range: Option<RangeInclusive<u16>>` | `BrightnessLevel(u8)` is the `CAM_Bright` Direct wire byte: `BrightnessLevel::new(u8)` is infallible and `TryFrom<u16>` refuses a value above `0xFF`. The brightness ranges are `u8`, and the selected profile's range is the only admission check (EVI-H100 `0x00..=0x1F` without `0x01..=0x04`, BRC-300 and Nearus BRC-300 `0x00..=0x17`, PTZOptics `0x00..=0x11`). The percentage and `Raw` conversions for brightness are removed: a percentage of the wire byte means nothing for a profile-specific range. |
| `Exposure::IRIS_RANGE: Option<CapabilityRange<u16>>`, `Capabilities::iris_range: Option<RangeInclusive<u16>>` (and the brightness pair) | `Option<CapabilityDomain<u16>>` (`CapabilityDomain<u8>` for brightness): the source table's bounds plus the positions it does not list. Build one with `CapabilityDomain::new(min, max)` or `CapabilityDomain::with_gaps(min, max, &[..])`; read it with `min()`, `max()`, `gaps()`, `bounds()` and `contains(value)`. It serializes as `{min, max, gaps}`. A gap is refused with `Error::InvalidParameter` (EVI-H100 iris and bright `0x01..=0x04`, Generic VISCA iris `0x01..=0x04`). |
| `ExposureCompensationLevel::new`, `Sharpness::SetLevel`, `ZoomPosition::normalize_with_max`, the defog, broadcast-domain, exposure-compensation and color-tuning inquiry decoders, and profile admission of pan/tilt speeds and positions, zoom, focus, iris, exposure, gain, brightness, color and preset values returned `InvalidParameter` | `ParameterOutOfRange` with the offending value and the inclusive bounds (the profile's range for admission checks). A shutter code outside the profile's table remains `InvalidParameter`. `ExposureCompensationLevel::MIN`/`MAX` are `Self`, and it implements `Display`. |
| `Sharpness::SetLevel { value: u8 }` | `Sharpness::SetLevel { value: SharpnessLevel }`; construct the level with `SharpnessLevel::new`. |
| `ParameterOutOfRange` displayed `(valid range: 2..4)` | `(valid range: 2..=4)`, an inclusive range. |
| `Error::Unknown` displayed `0x5` | `0x05`. |
| `visca_range_type!` over any `PartialOrd + Display` inner type | The inner type must widen losslessly into `i32` (`u8`, `u16`, `i8`, `i16`, `i32`), because `ParameterOutOfRange` reports `i32` bounds; a wider type fails to compile at the inner type. |
| `#[derive(ViscaValue)]` without `min`/`max` or `valid_values` produced an unchecked wrapper | Rejected at compile time; declare the domain. A range with `min > max` is also a compile error. |
| Range `new` was a plain `fn` | Range `new` is a `const fn`, so a checked value can initialize a `const`. |
| `display_format = "hex"` printed `0x5` | Hex display pads to two digits: `0x05`. |
| `ViscaInquiry` and `ViscaEnum` accepted a repeated key, keeping the last value | A repeated key is a compile error naming both occurrences, as for `ViscaValue`. |
| `ViscaInquiry` `opcode`/`subcode` accepted only decimal or `0x` literals | Any unsuffixed integer literal in `0..=255`, in any radix (`0b0100_0111`, `0o107`). |
| `ViscaEnum` discriminants accepted suffixed literals and byte literals | Discriminants are unsuffixed integer literals in `0..=255`. |
| `#[visca_enum(exhaustive = ...)]` was parsed and ignored | Rejected as an unknown key; generated conversions always cover every variant. |

#### Persisted `ProfileSpec` values from earlier releases (#795, #807, #808)

`ProfileSpec` deserialization requires every current field and validates the
result; no older shape is upgraded. A spec saved by 2.0.0-rc.3 or earlier
lacks `capabilities.focus_zones` and `capabilities.optical_zoom_ratio`, and its
shutter entries carry a `label` string instead of an `exposure` fraction, so
it fails to deserialize. The error names the first missing field and the fix,
for example:

```text
missing field `exposure` at line 112 column 5; the spec was saved by another release, so regenerate it with `ProfileSpec::from_compile_time::<P>()` for a built-in profile (or rebuild a custom profile with `ProfileSpec::builder`) and persist the result
```

Regenerate each stored built-in spec from the current registry and persist the
new value:

```rust,ignore
use grafton_visca::{profiles::SonyFR7, ProfileSpec};

let spec = ProfileSpec::from_compile_time::<SonyFR7>()?;
let json = serde_json::to_string(&spec)?; // replace the stored value
```

Rebuild a custom runtime profile with `ProfileSpec::builder`. A spec in the
current shape whose `capabilities.profile_id` names a built-in profile must
still match the current registry exactly; a stale typed-support set is refused
with `Error::InvalidRequest` naming the profile, the differing surfaces, and
the `from_compile_time` constructor that regenerates it.

#### Command encoding and inquiry decoding (#809–#812, #828)

Every built-in command and inquiry writes its frame through one frame writer,
and every inquiry reply decodes through the generated table, so the same
malformed reply now fails the same way on every inquiry and profile:

| Earlier RC | 2.0 |
| --- | --- |
| An unknown code byte in an inquiry reply (exposure mode, white-balance mode, focus zone, flip mode, an on/off byte, ...) returned `Error::InvalidParameter` from some inquiries and `Error::InvalidResponse` from others | Always `Error::InvalidResponse { expected, actual }`, with `actual` the offending byte. A camera reply outside its code set is a protocol error, not a caller error. Numeric values outside a value type's range are still `ParameterOutOfRange`. |
| A padded reply (`00 00 0p 0q`, `00 00 00 0p`) with nonzero padding decoded after silently dropping the leading nibbles (a sharpness position of `00 01 00 05` read as `5`) | `Error::InvalidResponseFormat`. Contrast and luminance replies now keep both `0p 0q` digits, as the reference documents. |
| `Nibbles::u8_pair(start)` and `Nibbles::last_nibble()` | `Nibbles::zero_extended_u8()` and `Nibbles::zero_extended_nibble()`, which reject nonzero padding. `#[derive(ViscaInquiry)]` with `parser = LastNibble` or `parser = Nibble` uses them, so a derived inquiry now rejects a reply with nonzero padding as `InvalidResponseFormat`, and `parser = Mode` reports an unknown code as the enum's own `InvalidResponse`. |
| `InquiryData::FlipState { horizontal, vertical }` | `InquiryData::FlipState { state }`, carrying `FlipState` as `Version` and `TallyStatus` do. `FlipState::from(ImageFlipMode)` converts a combined flip mode. |
| `InquiryData::SharpnessPosition { position: u16 }` and `InquiryData::Brightness { position: u16 }` | `position: u8`: both replies are `00 00 0p 0q`. `BrightnessLevel` is unchanged; the typed accessor still returns it. |
| The gain inquiry decoded only the last nibble of `00 00 0p 0q` | Both digits: `InquiryData::Gain::gain` is `pq`, and a value above `GainLevel`'s `0x0F` is the typed accessor's `ParameterOutOfRange` instead of being truncated. |
| A malformed Sony BRC-300 pan/tilt reply returned `Error::DecoderNotFound`, and a profile pairing BRC-300 framing with unsigned-centered coordinates failed each decode with `InvalidRequest` | The wrong length is `Error::InvalidResponseLength`, as for every other framing, and `Error::DecoderNotFound` is removed. The coordinate mismatch can no longer reach a request or a reply: `ProfileSpecBuilder::build`, `ProfileSpec` and `PanTiltCoordinateConversion` deserialization reject it with the profile-field error `InvalidRequest` naming `pan_tilt_coordinates`, and a compile-time profile that declares it fails to build at `PanTiltCoordinateConversion::for_profile`. |
| `AutoWhiteBalanceSensitivity::to_command_byte()` | `u8::from(sensitivity)`; the enum derives `ViscaEnum`, so `TryFrom<u8>` decodes it. `Flip` and `ImageFlipMode` derive `ViscaEnum` too. |
| Unit commands (`PowerOn`, `TallyRedOn`, `SpotlightOn`, ...) had hand-written `new()`/`Default` | `visca_command!` generates `#[derive(Default)]` and a `const fn new()` for every unit command, including commands a downstream crate declares with it; delete a downstream unit command's own `new`/`Default`. |
| `visca_command!` took `bytes = [..]` / `prefix = [..]` literal lists only | Either form takes any `[u8; N]` expression, so a body can be a named constant. Bodies never include the camera address byte. |

### Transport defaults, connect errors and serial startup (#797–#800, #828)

These changes land between 2.0 release candidates; each is breaking for code
that relied on the previous behaviour.

**One set of per-transport defaults.** `TransportConfig::for_tcp()`,
`for_udp()` and `for_serial()` are the defaults every built-in entry point
uses. Start from the matching constructor to change a field:

| Before | Now |
| --- | --- |
| `Transport::tcp()` / `udp()` limited replies to 128 bytes | 256 bytes (TCP) and 1024 bytes (UDP), as every other entry point |
| `CameraConfig::transport_config(c)` replaced a `BufferConfig::default()` buffer with the transport preset | the supplied configuration is used as given; pass `TransportConfig::for_tcp()` (or `for_udp`, `for_serial`) as the base |
| `TransportConfig::for_udp()`-equivalent configs reported TCP nodelay/keepalive | `for_udp()` and `for_serial()` report `None` for TCP-only fields; `tcp_nodelay: None` keeps the OS default |
| `NetTransportBuilder::udp_buffers()`, `raw_ip_buffers()`, `sony_ip_buffers()`, `BufferConfig::for_sony_ip()` | removed; the builder already starts from the transport's defaults, and `recv_buffer_size()` / `max_buffer_size()` set explicit limits |
| `TransportBuilderExt` | removed (its implementors were not nameable); use `Transport::tcp()` / `udp()` |
| `declare_net_transport!` (exported by accident) | removed |

`BufferConfig::recv_buffer_size` is the largest accepted reply frame and the
size of one read, and must be at least `BufferConfig::MIN_RECV_BUFFER_SIZE`
(24 bytes); `max_buffer_size` bounds stream input carried between reads
(see `docs/observability_and_recovery.md`). An unrepresentable timeout is now
`Error::InvalidParameter` naming the field (`"read_timeout"`, ...) both in
validation and at runtime, replacing `InvalidRequest("... exceeds the
monotonic clock range")`.

**Connect errors are the same on every facade.**

| Failure | Before | Now (blocking, Tokio, smol) |
| --- | --- | --- |
| Host name does not resolve | `InvalidAddress` (blocking), `Io` (async) | `InvalidAddress { reason: "Failed to resolve '<host:port>': ..." }` |
| Every resolved address refuses | `Io` | `ConnectionFailed { addr: "<host:port>", .. }` |
| `connect_timeout` expires (DNS included) | blocking DNS was unbounded | `Timeout` (stage `Session`); blocking DNS is bounded by the budget |
| Serial port cannot open | `TransportError` (blocking), `ConnectionFailed` (Tokio) | `ConnectionFailed { addr: "<port>", .. }` |

Each resolved TCP address now receives a share of the remaining connect
budget, so an unroutable first address no longer starves the others.

**Serial ports.** Both serial transports open the device for exclusive
access; on Tokio a second process opening the same port now fails with
`ConnectionFailed` where it previously shared the port. A zero-byte serial
read is `ConnectionClosed { reason: "serial port closed" }` on both facades.

**Serial startup writes nothing by default.** `transport::serial::Config` and
`CameraConfig` serial opens no longer broadcast I/F Clear. Select startup
writes explicitly:

```rust
# #[cfg(feature = "transport-serial")]
# fn example() -> grafton_visca::Result<()> {
use grafton_visca::{camera::CameraConfig, profiles::PtzOpticsG2, transport::serial::Startup};

let session = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9_600)
    .serial_startup(Startup::default().with_address_set(true).with_interface_clear(true))
    .open_serial()?;
# Ok(()) }
```

`serial::Config` replaces `address_set_on_connect` / `if_clear_on_connect`
(fields and builder methods) with `startup: Startup` and
`Config::startup(Startup)`; the unused `camera_address` field and method are
removed, and timeouts and buffer limits default to
`TransportConfig::for_serial()` (5 s) instead of 100 ms. A timed-out or
partial Address Set write now fails startup instead of being resent; a startup
whose Address Set attempts all go unanswered fails with
`Error::MaxRetriesExceeded`; a startup read error is returned unchanged
instead of as `TransportError`. Input received during startup is discarded.

**Addressed cameras are checked on every session open.** A transport that ran
Address Set reports `HasTransportConfig::addressed_bus()` (an
`AddressedBus { port, cameras }`). Every `Session::open`, blocking and async,
including the `CameraConfig` and `Connect` serial paths and caller-built
transports, checks each registered camera against it before any protocol I/O:
a camera the chain did not address fails with
`ConnectionFailed { addr: <port> }` whose `NotFound` source reads
"camera N was not addressed by Address Set (chain reported M)". Extra cameras
on the chain are allowed. A custom transport that wraps another must forward
`addressed_bus()`, as it forwards `transport_config()` and `send_semantics()`;
an unforwarded method silently disables the check.

### Reconfiguring timeouts at runtime

1.2.0's `Camera::set_timeout_config` took a `TimeoutConfig`; 2.0's
`Session::set_tuning` takes an `OperationalTuning`. The historical category
fields map directly to the corresponding tuning methods:

| 1.2.0 `TimeoutConfig` field | 2.0 `OperationalTuning` builder |
| --- | --- |
| `ack_timeout` | `ack_timeout` |
| `quick_timeout` | `quick_timeout` (an individual command-category override) |
| `movement_timeout` | `movement_timeout` |
| `preset_timeout` | `preset_timeout` |
| `long_timeout` | `long_running_timeout` |
| `network_timeout` | `network_timeout` |
| `default_timeout` | No direct counterpart: every 2.0 request selects a `TimeoutClass`; set the corresponding category deadline. |
| *(no 1.x field)* | `settlement_timeout` for the profile-selected protocol-settlement budget; `inquiry_timeout` is the profile inquiry-response deadline (a new dedicated field whose default differs from 1.x — see the note below) |
| `RetryConfig::max_retries`, `base_retry_delay`, `max_retry_duration` | `retry_limit` and `retry_timing` |

Two profile deadlines changed value relative to 1.x, and both are interim
figures pending a hardware-measurement pass:

- **Acknowledgement (`ack_timeout`).** 1.x's `TimeoutConfig` default was 500 ms,
  and that was the deadline every 1.x profile actually scheduled under. The
  2.0-preview built-in profiles briefly tightened this to 100 ms (150 ms and
  200 ms on two Sony profiles); every built-in profile is now restored to the
  1.x **500 ms** default. An operational override may only widen a profile
  deadline, so an `ack_timeout` below 500 ms is now rejected where the tighter
  preview default would have accepted it.
- **Inquiry response (`inquiry_timeout`).** 1.x inquiries had no dedicated
  deadline and used the 5 s `Quick` category budget. 2.0 gives inquiries their
  own deadline, defaulting to **1 s** on every built-in profile — the one
  intentional divergence from 1.x among these defaults. If your cameras or
  network make an inquiry legitimately take longer than 1 s to answer, widen it
  with `OperationalTuning::inquiry_timeout`.

`CommandTimeouts` is the profile's exact command policy. Its default table
preserves the 1.x values (Quick 5 seconds, Movement 30 seconds, Preset 60
seconds, LongRunning 300 seconds, and Network 5 seconds); each built-in
profile records its exact category values explicitly. An operational category
override must be non-zero and cannot undercut that category's profile value.
Inquiries declare `TimeoutClass::Inquiry` and use `inquiry_timeout`; command
Quick and Network values are never inquiry deadlines.
Retry counts remain category-based, but v2 deliberately makes replay depend on
correlation evidence. Backoff begins at 50 ms by default, uses deterministic
equal jitter, and is capped at the larger of 500 ms and the profile busy
timeout. One admission-to-terminal retry budget remains active through every
later noncancelled phase and is the largest of ten seconds, twice the request's
governing deadline, and the profile busy timeout. `retry_timing` may only
lengthen those bounds: `validate_tuning` rejects a first backoff below 50 ms, a
ceiling below the profile's, or a budget below ten seconds (or the busy
timeout), and a request whose own budget is longer than the override keeps it.
`retry_limit` may only lower the default base retry count of 3. Neither can
make an ambiguous raw replay safe.

**Tuning is a one-way ratchet toward *more* conservative (design decision D15).**
`OperationalTuning` may only lengthen a validated profile's timing facts, never
shorten them: an override that would undercut a category completion deadline, the
profile's `ack_timeout` floor (now 500 ms on every built-in profile), a retry
backoff or budget bound, or a pacing minimum is rejected at validation, live and
at construction alike. This is
stricter than issue #542 §102, whose "more conservative only" wording constrains
*pacing* alone; 2.0 consciously extends the same rule to timeout durations. The
practical consequence a 1.x user feels: 1.x let you install an aggressively
*short*, fail-fast timeout to trip a command early, and 2.0 removes that — a
command now waits at least the profile floor before it fails. If you relied on a
sub-floor deadline for fast failure, drive that from the caller side (cancel on
your own deadline) rather than from tuning.

Two differences matter in practice.

**Runtime tuning is replaced whole, not merged.** Any field left unset returns
to the profile default rather than keeping a value an earlier call installed.
Build the complete `OperationalTuning` value each time. Construction-only raw
recovery policy no longer masquerades as one of those mutable fields: configure
it with `SessionConfig::with_strict_unconfirmed_poison` (or the matching
`CameraConfig` builder) before opening. `Session::tuning` reports tuning only.

**In-flight work is not re-timed.** 1.2.0 recomputed deadlines on every
housekeeping pass, so widening `ack_timeout` also rescued a command that was
already waiting for its acknowledgement. 2.0 stamps a request's deadlines once,
at preparation, and the engine derives its absolute phase deadlines from that
stamp, so `set_tuning` governs **every request prepared after it** and leaves a
request already admitted on the deadlines it was admitted with. The owner's
session-wide pacing floor and per-target socket capacity *are* applied at once,
so work still queued behind pacing is released under the new values.

If a command that is already running must move onto a widened deadline, cancel
it and resubmit:

```rust
use std::time::Duration;

use grafton_visca::{
    blocking::{Camera, Operation, Session},
    camera::profiles::PtzOpticsG2,
    completion::AppliedOnly,
    Error, OperationCommand, OperationalTuning,
};

fn resubmit_after_widening<O>(
    session: &Session,
    camera: &Camera<PtzOpticsG2>,
    mut operation: Operation<AppliedOnly>,
    command: O,
) -> Result<Operation<AppliedOnly>, Error>
where
    O: OperationCommand<AppliedOnly>,
{
    session.set_tuning(OperationalTuning::new().ack_timeout(Duration::from_secs(2)))?;
    // `operation` was admitted before the update and keeps its old deadline.
    let _ = operation.cancel_with_timeout(Duration::from_secs(1));
    let operation = camera.submit::<AppliedOnly, _>(&command)?;
    Ok(operation)
}
```

Runtime updates are validated on exactly the grounds a session open validates
its configured tuning on, so tuning that would have been refused at open is
refused here too and leaves the live configuration untouched.

### Built-in command timeout categories

2.0 re-derives the timeout category of several built-in commands from what the
command does, so the deadline a command waits for differs from 1.x for these
rows (the retry class, which decides what may be replayed, is a separate
2.0 fact):

| Commands | 1.x category | 2.0 category | Why |
| --- | --- | --- | --- |
| `PanTiltStop`, `ZoomStop`, `FocusStop`, `FocusOnePush`, `FocusSnap` | Movement | Quick | Urgent stops and applied-only triggers use the quick deadline and keep movement retry/error semantics. |
| `PanTiltLimitSet`, `PanTiltLimitClear`, `FocusAuto`, `FocusManual`, `FocusToggle` | Movement | Quick | Limit-state and focus-mode edits are plain configuration, not actuation. |
| `IrisReset`, `IrisUp`, `IrisDown`, `IrisDirect`, `NdFilterDirect`, `NdFilterStepUp`, `NdFilterStepDown` | Quick | Movement | Targeted iris and ND-filter operations settle through profile-selected protocol inquiries. |
| `Sharpness*`, `Gamma`, `NoiseReduction2d`/`3d` and their `Off` forms, `ImageFlipBoth`, `ImageFlipCombined` | Custom (an uncategorized 60 s fallback) | Quick | Explicit quick configuration writes. |
| All 63 queryable inquiries | Quick | Inquiry | Inquiry response timing is a separate profile fact; see "Inquiry response" above. |

`CommandCancel` stays Quick and is never retried. `PresetSet` and `PresetReset`
keep the Preset deadline and retry class. `NoiseReduction2dMode` (`01 04 50`)
is a new 2.0 control with no 1.x counterpart; it is Quick with the standard
retry class.

## Recovery changes

Do not reconnect by reusing a poisoned owner, old view, subscriber, or operation
handle. Keep `SessionConfig`, open a fresh session, select fresh views, and
expect every cache entry to start `Unknown`. Re-query camera state explicitly;
the owner never automatically resubmits commands. Old handles must report the
old owner's terminal/closed outcome and cannot be rebound to the new owner.

Classify positive session-death evidence with `Error::requires_new_session()`
rather than matching `ErrorKind::IoClosed` or individual variants. Transport
close, explicit shutdown, and poison are deliberately distinct errors that
share one kind, so the kind alone cannot tell a field disconnect apart from a
shutdown this application requested. Correlation uncertainty now has the
dedicated `ErrorKind::Unconfirmed`; it is still not permission to replay. The
canonical event × envelope × transport table lives in the
[`error` module](../src/error.rs); this shorter recovery view highlights the
outcomes most likely to cause an incorrect reconnect or retry loop:

| Condition | Error | `requires_new_session()` | What to do |
| --- | --- | --- | --- |
| The peer closed the connection, including an OS TCP keepalive timeout normalized from `io::ErrorKind::TimedOut` | `ConnectionClosed` | `true` | Open a fresh session and re-query. |
| The stream position became unknowable, or the strict opt-in poisoned the session for an unconfirmable command | `StreamPoisoned` | `true` | Open a fresh session; never blindly replay uncertain work. |
| A typed STOP finds its camera's control reserve and ordinary admission both full | `ControlReserveExhausted` (`is_retryable() == true`) | `false` | Back off briefly; earlier stops for that camera are still pending. Ordinary saturation alone no longer rejects a STOP (D26). |
| Your own wait on an operation, command, or inquiry expires while the request is still running | `ObservationTimeout { operation }` (`is_retryable() == false`) | `false` | Wait again on the handle, or reconcile; never resubmit. |
| An open peer answers no built-in inquiry through its default retry policy (ten-second total-budget floor, approximately 10.05 seconds with the first backoff) | `Timeout` (stage `Terminal`, certainty `FailedConclusively`; `is_retryable() == true`) | `false` | Compare `MetricsSnapshot::received_frames` around bounded heartbeats; replace the session only when the application's silence threshold is met. |
| A sent unsequenced command on a raw-VISCA envelope cannot be correlated, default per-request mode (ACK/completion/cancellation ambiguity or active retry-budget expiry; the review probe reached this in about 2.56 seconds) | `UnsequencedCommandUnconfirmed` (`kind() == Unconfirmed`) | `false` | Reconcile that command's camera effect; do not replay it blindly or infer that the session died. |
| Raw-VISCA over a stream: a camera's earlier inquiry timed out after it was written and its owed reply has not arrived within the profile's ambiguity window | `InquiryCorrelationLost { camera }` (`is_retryable() == false`; `Terminal`/`NotAccepted`, nothing sent) | `false` | The session is live and other cameras work. That camera's inquiries reopen when the owed reply arrives or the camera answers a later command (proving the reply will never come); close and reopen the session to recover them sooner. |
| Raw-VISCA over a stream: a camera's earlier command ended unconfirmed before its ACK and its owed answer has not arrived within that command's ambiguity window | `CommandCorrelationLost { camera }` (`is_retryable() == false`; `Terminal`/`NotAccepted`, nothing sent) | `false` | The session is live; STOPs and inquiries to that camera still work. Ordinary and `NoReply` commands resume when the owed answer arrives or the camera answers a later inquiry or STOP; close and reopen the session to recover them sooner. A `CompletionOnly` command also fails this way behind a `NoReply` command the camera has answered nothing since, past that command's window (#795). |
| The application shut the session down | `RuntimeShutdown` | `false` | Reconnect only if the application intends to start another session. |

`received_frames` is positive evidence, not an automatic failure detector. An
increase proves that a valid VISCA response arrived. No increase proves only
silence during the sampled interval; the application owns the number of
unanswered heartbeats or wall-clock duration that triggers replacement. The
library sends no background heartbeat.

**Behavior change (issue #713).** A partial raw response straddling a
correlation-release boundary no longer consumes an implementation-specific
poll-count budget or poisons merely because its tail is late. Both facades wait
for the same engine-owned grace (at most 100 ms and never longer than the
configured read timeout), then discard and diagnose an orphaned prefix while
leaving the session usable. `StreamPoisoned` remains reserved for an
unrecoverable stream position, such as overflow or a failed discard.

**Behavior change (issue #712).** A matched raw inquiry reply now releases its
single-flight lane immediately; it no longer installs the profile's one-second
pre-ACK ambiguity hold or delays an urgent stop. Only inquiry timeout, terminal
error, or retry release can retain a late reply. That remaining hold uses the
new `ProfileTiming::raw_inquiry_reply_skew` fact (required by
`ProfileTimingBuilder`, defaulted by compile-time profiles to no more than their
minimum inquiry spacing) and blocks only another inquiry for the same target.
ACK-bearing commands remain eligible. An inquiry also no longer starts the
urgent command-spacing clock, while consecutive commands and cancellations
still honor physical command pacing.

**Behavior change (issue #714).** A raw predecessor whose ACK was lost remains
quarantined only to its known ambiguity deadline. Ordinary work queues to that
bounded correlation release on both facades. An intrinsically `Urgent` stop
takes the safety-lane exception immediately (subject to command pacing and
actual socket capacity). While the predecessor and stop are both
open, unsequenced ACK/error traffic is ambiguous and binds to neither. Treat a
later `UnsequencedCommandUnconfirmed` from either handle as uncertainty about
the reply, not evidence that the urgent stop failed to reach the camera.

A fatal receive closure is normalized to `ConnectionClosed`, with the
underlying transport error's text retained in its reason. `StreamPoisoned` is
reserved for a stream whose framing or write position became unknowable (for
example, a failed stream write or unrecoverable framer loss); a failed read
does not by itself establish stream poison because it consumed no bytes.

**Sustained receive faults close the session.** 1.x retried every
non-`ConnectionClosed` read error indefinitely. In 2.0 a run of twelve
consecutive transient receive faults that spans at least one second closes the
session as `ConnectionClosed` with the last cause retained; a successful read
or a five-second gap between faults resets the run, and an idle read does
not reset it.

**Behavior change (issue #671).** In an earlier 2.0 preview an unconfirmable raw
command poisoned the whole session and `UnsequencedCommandUnconfirmed` mapped to
`true`. It now fails only that one command on a still-live session, so it maps to
`false`: reconcile that command's camera effect (never blindly replay it — it may
already have acted on the camera) and keep using the session for unrelated work.
A raw caller should therefore keep the session alive and reconcile before any
deliberate resubmission. If you preferred the old hard-fail behavior, opt into
`SessionConfig::with_strict_unconfirmed_poison(true)` (or the matching
`CameraConfig` builder), which poisons the session and reports
`StreamPoisoned` (`true`) exactly as before.

**Behavior change (issue #675).** `read_timeout` and `write_timeout` now take
effect on async sessions. In an earlier 2.0 preview both knobs were silently
ignored on every async transport (only the blocking sockets applied them), so an
async read or write could block indefinitely. The async owner now bounds each
one: a read that outlasts `read_timeout` is treated as an idle no-data receive
(nothing consumed, no request penalized), and a write that outlasts
`write_timeout` is abandoned as a send failure — a stream write poisons, a
datagram write fails only its own request — so a stalled peer can no longer hang
`close()`. The defaults are unchanged (5 s each). If your async application set
either knob expecting it to matter, it now does; if it relied on async
reads/writes never timing out, set the value you want explicitly. A custom
`AsyncTransport` should be cancellation-safe (a timed-out read/write future is
dropped) — futures built from the standard async socket readers/writers already
are. The same fix bounds a *babbling* peer (one that returns a valid frame on
every poll): it can no longer starve shutdown, `close()`, admission, or an
emergency stop on the async facade.

**`shutdown` vs `close` (design decision D14).** These are two distinct lifecycle
methods, and the difference is load-bearing for orderly teardown. `shutdown` is
the idempotent, non-joining signal: it asks the owner to stop, is safe to call
repeatedly and again after a `RuntimeShutdown` has already been observed, and
performs no second round of I/O or resolution. The consuming `close` is a
deterministic transport-teardown barrier: it takes the session by value and
returns only once the sole owner has finished its boundary drain and dropped its
driver and transport, so on return the socket or serial port is released — though
it does not promise an executor-specific task join. `close` maps an explicit
`RuntimeShutdown` result to `Ok(())` (a shutdown you asked for is not an error),
while a transport or poison cause that won the source ordering is preserved and
returned. Use `shutdown` to signal from a handle or a second task; use `close`
when the next step needs the transport actually torn down (reopening the same
address, or exiting cleanly).

`true` is positive proof that the session is finished; `false` only means the
error alone does not prove it. Ordinary per-request failures — timeouts, busy
states, protocol and parameter errors — are `false`, and so is a raw `Io`
failure, because a datagram write failure is isolated to its own transmission
and a stream failure reaches the caller as `StreamPoisoned`.

1.x's `Error::to_public_error()` is gone and has no replacement. It folded
`NoSocket` into a transport-unavailable session-death error, which in 2.0 would
move the camera's transient `0x05` capacity answer into the set
`requires_new_session()` calls session death — a reconnect where a retry was
correct. Use `Error::kind()` for the coarse category and
`requires_new_session()` for the reconnect decision.

```rust
use grafton_visca::blocking::{Camera, Session};
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::transport::{BlockingTransport, HasTransportConfig};
use grafton_visca::{Error, PlainCommand, SessionConfig};

fn execute_or_reopen<C, T>(
    camera: &Camera<PtzOpticsG2>,
    command: &C,
    config: &SessionConfig,
    new_transport: impl FnOnce() -> Result<T, Error>,
) -> Result<(), Error>
where
    C: PlainCommand + ?Sized,
    T: BlockingTransport + HasTransportConfig + 'static,
{
    match camera.execute(command) {
        Ok(()) => {}
        Err(error) if error.requires_new_session() => {
            // Drop the old session and views, then rebuild from the retained
            // configuration. Re-query state before applying anything new.
            let session = Session::open(new_transport()?, config.clone())?;
            // ...
            session.close()?;
        }
        Err(error) => return Err(error),
    }
    Ok(())
}
```

For the complete construction, transport, target, tuning, noun, and lifecycle
examples, see [`usage_2_0.md`](usage_2_0.md). For bounded state and diagnostic
behavior, see [`observability_and_recovery.md`](observability_and_recovery.md).
