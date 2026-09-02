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
physical-rest observation; choose `settled()` only when the profile supplies
the required position inquiry and that distinction matters.

## Construction and feature selection

| 1.x category | 2.0 destination |
| --- | --- |
| `mode-async` | `async`; add `runtime-tokio` or `runtime-smol` when using a built-in runtime. |
| Generic mode/transport/executor `Camera<Mode, P, T, E>` and `AsyncCamera` | `blocking::Session` or async `Session`, then `Camera<P>` for a target view. |
| `BlockingClient` and direct generic camera constructors | `blocking::Connect`, `blocking::CameraConfig`, or `blocking::Session::open`. |
| 1.2.0's deprecation of the blocking `CameraSession` methods, which pointed at `BlockingClient` | `blocking::CameraSession<P>` again — see [The `BlockingClient` inversion](#the-blockingclient-inversion) below. |
| Legacy `Connect`/`CameraConfig` single-target shortcuts | Keep the convenience path, but use `SessionConfig` when configuration must be cloned, reused, or shared across targets. |
| Direct `Camera::new_*`, `open_*` compatibility constructors | `Connect`/`CameraConfig` for standard transports; `Session::open` for a caller-owned transport. |
| One implicit camera ID or raw camera-ID setters | `CameraId`, `SessionConfig::for_target`, `try_camera_id`, and explicit `camera_for`. |
| `CameraVariant` and root camera-number constants | `CameraId` plus a validated `ProfileSpec`/compile-time profile. |
| `RuntimeHandle` and private scheduler/runtime modules | `TokioRuntime`, `SmolRuntime`, or a coherent public `Executor`; never construct the owner directly. |
| `CameraBuilder` and its `with_executor(...).from_transport(...).profile::<P>().open_async()` chain | `Connect`, `CameraConfig`, or `Connect::builder()` for standard transports; async `Session::open(transport, SessionConfig, executor)` or blocking `blocking::Session::open(transport, SessionConfig)` for a caller-owned one. `camera_id(...)` becomes a `SessionConfig` target (`for_target`/`register_target`) or `CameraConfig::camera_id`; `timeout_config`/`retry_config` become an `OperationalTuning` supplied through `with_tuning`. |
| Implicit Sony sequence synchronization at connection startup | Startup remains write-free by default. Opt in with `SessionConfig::with_sony_sequence_reset_on_connect(true)` or the matching `CameraConfig` builder only for a Sony-encapsulated profile; the RESET is sent before owner work begins. Observe its one-/two-byte reply as `DiagnosticResponse::SonyControl { code }`. |
| `Runtime::connect_tcp` / `connect_udp` and the `TransportHandle` enum | Both remain under `async` as `runtime::{Runtime, TransportHandle}`. Prefer `Connect`/`CameraConfig`; reach for `Session::open(TransportHandle::Tcp(runtime.connect_tcp(addr, cfg).await?), config)` only when you drive the transport yourself. |

`SessionConfig` accepts only individual VISCA IDs 1–7. Broadcast, duplicate
registration, an empty registry, and unsupported profile/transport pairs are
errors before I/O. `camera()` is only for a sole target; multi-target code must
select `camera_for(target)`.

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
| `mode-async` (the only mode toggle) | `async`; add `runtime-tokio` or `runtime-smol` for a built-in runtime |
| blocking XOR async, enforced by `cfg` | `blocking` and `async` are independent and **co-enableable** in one build |
| `--no-default-features` ⇒ blocking crate | `--no-default-features` (no facade) ⇒ pure engine/domain layers only |
| `transport-serial` did not select an explicit blocking feature | `transport-serial` now enables `blocking`; use `transport-serial-tokio` for async Tokio serial |

So one 2.0 build can expose both `grafton_visca::Camera` (async) and
`grafton_visca::blocking::Camera`. If you relied on 1.x's implicit-blocking
default you are unaffected — it is now the explicit default. If you enabled
`mode-async`, switch to `async` (or a `runtime-*` feature) and add `blocking`
only if you also want the blocking facade in the same build.

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

* `BlockingClient` for one camera → `blocking::Connect::open_tcp_camera::<P>`
  or `open_udp_camera::<P>` (configured form:
  `blocking::CameraConfig::<P>::open_camera`), and, under `transport-serial`,
  `blocking::Connect::open_serial_camera::<P>` (configured form:
  `blocking::CameraConfig::<P>::open_serial_camera`). All return
  `blocking::CameraSession<P>`; `session.camera()` is the noun view and takes
  no turbofish.
* `BlockingClient` for several cameras → `blocking::Connect::open_tcp` /
  `open_udp`, which return `blocking::Session`, then
  `session.camera_for::<P>(target)` per target.
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
| 1.x `is_moving` / `stop_all_motion`, and the movement waits `await_idle` / `await_pan_tilt_idle` / `await_zoom_idle` / `await_focus_idle` / `await_axes_idle` (each taking a `Duration`) | `camera.motion().is_moving()` (no argument, samples `AffectedAxes::MOVEMENT`), `is_moving_axes(MotionQuery)` for an explicit axis set, `wait_until_idle(IdleWait)`, and `stop_all_motion()`. There was **no** 1.x `wait_until_idle`; that name is 2.0's. |
| 1.x `AwaitConfig` and `await_with_config(&AwaitConfig)` (`for_pan_tilt`/`for_zoom`/`for_focus`/`for_preset_recall`, `poll_interval`, `tolerance`, `debug`) | `camera::IdleWait` (same `for_*` presets plus `with_interval`/`with_tolerance`/`with_timeout`) passed to `wait_until_idle`, or `camera::MotionQuery` for `is_moving_axes`. The `debug` field has no counterpart — use `tracing`. |
| Noun-specific idle/wait aliases | The separate `motion()` safety/observation view. |
| 1.x `Axes::ALL` as "everything that moves" | `AffectedAxes::MOVEMENT`. The 1.x `Axes` type is renamed `AffectedAxes` **and** `ALL` changed meaning — see [`Axes` → `AffectedAxes`: rename and `ALL` meaning change](#axes--affectedaxes-rename-and-all-meaning-change) below. |
| `DynCameraControl` | `DynSessionCameraControl` plus `DynSessionCameraNouns`. |
| `DynPanTiltControl`, `DynZoomControl`, `DynFocusControl`, `DynPresetsControl`, and `DynMotionControl` | `DynPanTilt`, `DynZoom`, `DynFocus`, `DynPresets`, and `DynMotion`. The final dynamic surface also has `DynPower`, `DynSystem`, `DynExposure`, `DynWhiteBalance`, `DynImage`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`, and `DynAdvanced`. |
| `IntoDynCamera` and wrapper-specific dynamic constructors | Async: `Session::camera_dyn` / `camera_dyn_for`, or `DynSessionCamera::from_session` / `from_session_target`. Blocking runtime profiles: the same session selectors, returning `BlockingDynSessionCamera`. |
| Metadata-only optional control fallback | Static `Has*` marker gates, or dynamic `supports_typed(...)` followed by the matching `Dyn*` noun. |
| Duplicate `NdFilterInquiry` accessor vocabulary | `nd_filter().position()` only. |
| Separate focus `lock()`/`unlock()` twins | One parameterized `focus().set_lock(FocusLock)`. |
| Root `toggle_menu()` (the 1.x `DirectMenuControl` method on the camera) | `menu().toggle_display()` is the direct replacement. Prefer `menu().display(true)` / `menu().display(false)` when the intended state is known; `menu().status()`, `navigate(...)`, and `select()` cover the remaining menu operations. |
| Zoom `set_normalized(UnitInterval)` / `set_normalized_in_domain(UnitInterval, ZoomDomain)` | Same names on `zoom()`: `zoom().set_normalized(UnitInterval)` and `zoom().set_normalized_in_domain(UnitInterval, ZoomDomain)`. They are now **targeted operations** returning an `Operation<Targeted>`; await it with `applied()`/`settled()` instead of getting a bare `Result<()>`. |

Dynamic views remain async and object-safe. They erase profile/request types but
share the static session's owner, timeout, pacing, cancellation, and state
cache. There is no dynamic policy layer that can bypass static preparation.

### Pan/tilt widths and explicit profile gates

Raw pan/tilt coordinates are `i32` in 2.0 so the BRC-300's documented signed
20-bit pan field can be represented without truncation. This changes
`PanTiltPosition`, `PanTiltPositionRaw`, the pan/tilt field of
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
| `HasImageProcessing` arrived through a blanket implementation | Built-in profiles receive an explicit implementation only when at least one source-backed image surface exists. A downstream profile must opt in deliberately. |
| `CapabilityRange` serde accepted `min > max`, and checked scalar wrappers could deserialize invalid values | Deserialization now validates the same invariants as construction; handle the serde error and repair invalid persisted data before retrying. |

### Wire corrections and removed ambiguous inquiries

2.0 deliberately does not preserve several incorrect 1.x byte sequences. The
complete audited delta is recorded in the
[CHANGELOG wire-corrections table](../CHANGELOG.md#changed); the migrations a
caller can observe directly are:

| 1.x assumption | 2.0 destination |
| --- | --- |
| Autofocus sensitivity Low/Normal/High encoded as `00/01/02` | The sourced wire values are `03/02/01`. Keep semantic enum values in application state rather than treating an integer cast as a stable wire code. |
| Bright Direct used `04 0D`, and the 2.0 preview exposed byte-identical `Brightness::SetLevel` / `Brightness::Direct` variants | Use only `Brightness::SetLevel` or the noun method `brightness_set`. They encode the sourced direct register `04 4D`; `Brightness::Direct` and `brightness_direct` are removed, while `04 0D` remains only the reset/up/down family. |
| UpRight limit corner was `03` | It is `01` in limit set and clear frames. |
| Focus-zone inquiry was `09 04 3C` | It now matches the focus-zone register at `09 04 AA`. |
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
`motion().is_moving_axes(MotionQuery::new(AffectedAxes::ALL))`,
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
| `InFlight::await_applied(timeout)` / `BlockingInFlight::await_applied(timeout)`, and `await_settled(timeout)` | `Operation::applied()` / `settled()` for the request's configured deadline, or `applied_with_timeout(timeout)` / `settled_with_timeout(timeout)` for an explicit one. `settled*` exists only on a targeted operation and observes its profile-selected protocol settlement condition; it is not a 2.0.0-rc.1 bench-verified assertion of physical rest. |
| `send_command_with_id(&cmd) -> (CommandId, _)` followed later by `cancel(command_id)` | `camera.submit::<K, _>(&op)` returns a linear `Operation<K>` **handle**; hold it and call `operation.cancel()`. The handle itself is the cancellation authority. |
| `CommandId` used as a cancellation key | `OperationId` (from `operation.id()`) is read-only observability only; it can no longer authorize waiting or cancellation. Cancel through the owning `Operation` / `Cancellation` handle. |
| `cancel_command(ViscaSocket)` and `cancel_socket(ViscaSocket)` (cancel by socket) | There is no public cancel-by-socket call — socket cancellation is owner-only and is driven by cancelling the specific `Operation`. `ViscaSocket` still exists (`grafton_visca::ViscaSocket`) as a value type but is not a cancellation entry point. To force motion to end, submit the typed STOP. |
| `InFlightDyn`/legacy dynamic operation wrappers | `DynTargetedOperation`, `DynAppliedOperation`, and `DynCancellation`. Applied-only handles have no settled operation. |
| Dropping an operation handle | Unchanged from 1.x: drop is `detach` and never stops hardware. See [Drop never stops hardware](#drop-never-stops-hardware) for the scoped stop-on-exit pattern. |
| Raw `command::RawInquiryPayload`/untyped response assumptions | `raw::Plain`, `raw::Inquiry`, `raw::Targeted`, or `raw::AppliedOnly`, with an explicit response parser/spec. |
| `ViscaCommand` response-associated-type extensions | The typed `Request`/`Inquiry`/`OperationCommand` contract and `ResponseParser` for custom decoding. |
| Unvalidated vendor opcodes that 1.x let through because it validated nothing on send (e.g. vendor tally mode `0A 02 02 0p` on a PTZOptics profile) | The typed and `execute` routes validate against profile capability, so a profile without the typed surface refuses the opcode (tally mode needs `HasTally`). Send a firmware-confirmed vendor opcode through `raw::Plain` instead. |
| Plain requests submitted as operations or operations without affected axes | Match the request class exactly: `execute` for plain, `inquire` for inquiry, and `submit` for a typed operation with non-empty affected axes. |
| Caller-selected lifecycle IDs, retry class, target, or settlement metadata | Owner-derived preparation metadata. Callers select a request, a timeout, and a scheduling class; they do not select protocol identity or queue positions. |
| `runtime::Priority` and the `*_priority` camera methods | `SubmissionClass` and the `*_with_submission_class` / `set_submission_class` surface. `ControlClass::Urgent` is now intrinsic safety metadata, not caller QoS. See [Submission priority](#submission-priority). |

The built-in command classification remains one closed semantic ledger. A
custom request must declare its class explicitly; wire opcode or response shape
does not infer lifecycle semantics.

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
safety boundary. On the blocking facade this holds even on a raw profile while a
caller still holds an un-awaited operation handle: the emergency stop's first
write no longer fails `TransportBusy` against the raw single-candidate pre-ACK
gate. Ordinary ACK-bearing work still uses the bounded #673 ACK drain, but an
intrinsically `Urgent` stop bypasses that drain and may create one explicit
two-candidate safety-lane state (#714). An ACK observed while both raw
candidates are open binds to neither; either operation may therefore report
`UnsequencedCommandUnconfirmed` even though the stop bytes reached the camera.
Only genuine socket-capacity contention — every command socket occupied by a
distinct in-flight command — still fails a blocking operation submit fast with
`TransportBusy`.

| 1.x call | 2.0 call |
| --- | --- |
| `camera.set_command_priority(Priority::Low)` | `camera.set_submission_class(Some(SubmissionClass::Background))` |
| `camera.command_priority()` | `camera.submission_class()` — returns `Option<SubmissionClass>`, where `None` means "use each request's intrinsic class" |
| `camera.execute_with_priority(cmd, Priority::High)` | `camera.execute_with_submission_class(&cmd, SubmissionClass::User)` |
| `camera.execute_with_priority(stop, Priority::Critical)` | Submit the typed stop normally; it is intrinsically `ControlClass::Urgent`. |
| — (no 1.x equivalent) | `camera.inquire_with_submission_class(&inquiry, class)` and `camera.submit_with_submission_class::<K, _>(&operation, class)` |
| `BlockingClient` priority methods | The same names on `blocking::Camera`; `blocking::CameraSession` exposes forwarding default helpers and returns that `Camera` from `camera()` |
| — (no 1.x equivalent) | `DynSessionCamera::execute_with_submission_class`, `inquire_with_submission_class`, `submit_targeted_with_submission_class`, `submit_applied_with_submission_class`, and `set_submission_class`; `BlockingDynSessionCamera` has the native blocking `execute_with_submission_class`, `inquire_with_submission_class`, `submit_with_submission_class`, and `set_submission_class` counterparts |

Three behavioural differences are worth reading before porting:

- **No public QoS override can weaken an urgent request.** A handle set to
  `Background`, and even a per-submission
  `SubmissionClass::Background`, still submits `PanTiltStop`, `ZoomStop`,
  `FocusStop`, and owner-issued protocol cancellation as `ControlClass::Urgent`.
  Nor can ordinary work manufacture the urgent lane: `SubmissionClass` has no
  `Urgent` variant, and the raw escape hatch's `raw::Policy` / `raw::Spec` reject
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

### A refused `cancel` hands the handle back

Cancelling a command that has already been written needs profile support for the
standard VISCA socket-cancel command. `PtzOpticsG2` is the one built-in profile
without it, and there the owner refuses with `Error::NotSupported` and
deliberately leaves the original request scheduled and able to complete — the
same per-phase contract 1.x had, where a still-queued command cancels locally on
every profile.

Because the original is still live, `cancel` consumes the handle only when it
succeeds. A refusal returns `CancelRejected<H>`, which carries the handle back:

```rust
#[cfg(feature = "async")]
async fn recover_refused_cancel(
    camera: &grafton_visca::Camera<grafton_visca::camera::profiles::PtzOpticsG2>,
    operation: grafton_visca::Operation<grafton_visca::completion::AppliedOnly>,
) -> Result<grafton_visca::Cancellation, grafton_visca::Error> {
    use grafton_visca::Error;

    let operation = match operation.cancel().await {
        Ok(cancellation) => return Ok(cancellation),
        Err(rejected) => rejected
            .into_operation()
            .ok_or(Error::RuntimeShutdown)?,
    };
    // Still observable, still retryable — and the axis is stopped the usual way.
    camera.zoom().stop().await?.applied().await?;
    operation.detach();
    # Err(Error::NotSupported)
}
```

`?` still works in a function returning `Error`: the `From<CancelRejected<H>>`
conversion keeps the reason and detaches the handle, which is exactly what the
consuming shape did before.

1.x's async and dyn handles borrowed for `cancel` (`InFlight::cancel(&self)` at
`src/camera/inflight.rs:344`, documented at `:337` as "This method borrows the
handle, so its exact response may still be awaited"), so a `NotSupported` there
also left the caller holding the handle. 2.0 keeps that recovery while keeping
its linear consuming terminal methods. 1.x's *blocking* handle consumed on this
path (`src/camera/inflight.rs:615`) and `src/runtime/blocking_runner.rs:413`
discarded the retained result with it; 2.0 does not reproduce that asymmetry —
both facades behave like the 1.x async one.

### Scoped stop-on-exit guard

To bound movement by a scope rather than by an explicit call on every path,
write a guard whose own `Drop` submits the typed STOP. This is caller-owned
code — the library deliberately offers no such type, so the policy, the axes,
and the failure handling stay yours.

Every Rust snippet on this page is compiled by the crate's own test suite, so
the `#[cfg(feature = "...")]` attributes below are load-bearing: they name the
Cargo feature a snippet needs.

```rust
use grafton_visca::blocking::Camera;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::command::PanTiltDirection;
use grafton_visca::types::{PanSpeed, TiltSpeed};
use grafton_visca::Error;

struct StopPanTiltOnExit<'a, 'session> {
    camera: &'a Camera<'session, PtzOpticsG2>,
}

impl Drop for StopPanTiltOnExit<'_, '_> {
    fn drop(&mut self) {
        // `Drop` cannot report a failure and may run while unwinding, so the
        // stop is best effort — as in any scope guard.
        if let Ok(stop) = self.camera.pan_tilt().stop() {
            let _ = stop.applied();
        }
    }
}

fn bounded_drive(
    camera: &Camera<'_, PtzOpticsG2>,
    direction: PanTiltDirection,
    pan: PanSpeed,
    tilt: TiltSpeed,
    do_fallible_work: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    // Hold the guard for the region that must stay bounded.
    let _stop_on_exit = StopPanTiltOnExit { camera };
    let drive = camera.pan_tilt().move_direction(direction, pan, tilt)?;
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
        if let Ok(stop) = camera.pan_tilt().stop().await {
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
| `inquiry_conversions` module (`ZoomDomain`, `ZoomPositionExt`, `PanTiltPositionRaw`, `PanTiltPositionDeg`, `zoom_from_normalized`) | Preserved: the `grafton_visca::inquiry_conversions` module and those exports remain, so raw↔degrees and normalized↔units conversions port unchanged. Prefer the profile-aware noun methods when a profile is in hand. |
| `capabilities::Capabilities` runtime discovery (`from_profile::<P>()`, `supports_typed`, zoom/magnification helpers) | Preserved: `Capabilities` stays at `grafton_visca::capabilities::Capabilities`. Reach it with `camera.capabilities()` (static or dynamic) or `Capabilities::from_profile::<P>()`; build a runtime profile from `Capabilities::runtime_baseline(..)` plus `ProfileSpec::builder`. |
| Model-aware constructors that embed a profile in a value | Plain checked values plus the profile-gated camera noun; profile validation belongs at preparation. |
| Generic optional accessors or unsupported PTZOptics/Sony controls | Compile-time `Has*` gates; dynamic callers inspect capability support. Unsupported controls are not exposed through metadata fallback. |
| Direct PTZOptics ND filter, Motion Sync, variable-speed, Sony color-temperature, or legacy quality controls | The matching supported noun only when its profile marker permits it; otherwise use a raw extension deliberately. |
| Public `camera::*`, `command::*`, `protocol::*`, response, cache, runtime, or transport implementation modules | Supported root/module exports and owner methods. Implementation submodules are not extension points. |
| `diagnostics::Diagnostics`/probe-style compatibility API | `Session::metrics`, async `subscribe_diagnostics`, and blocking `drain_diagnostics`. |
| Legacy mutable `cache::StateCache` | Owner-backed read-only root `StateCache`; use `target()` and `value(StateKey)`. |
| 1.x `Camera::set_timeout_config` / `timeout_config` | `Session::set_tuning` / `tuning` (and the same pair on `CameraSession`), taking an `OperationalTuning`. Standard `CameraConfig` construction uses `with_tuning`. See [Reconfiguring timeouts at runtime](#reconfiguring-timeouts-at-runtime). |

### Removed compatibility and helper vocabulary

Several 1.x names described implementation machinery rather than a stable
camera operation. They have no name-preserving alias in 2.0:

| 1.x API | 2.0 destination |
| --- | --- |
| `mode::{Mode, Blocking, Async}` and the public `mode` module | Select the `blocking` and/or `async` Cargo feature and use `blocking::Camera<P>` or async `Camera<P>`. The two facades can coexist; mode is no longer a camera type parameter. |
| `GenericViscaCam<T>`, `NearusBRC300Cam<T>`, `PtzOptics30XCam<T>`, `PtzOpticsG2Cam<T>`, `PtzOpticsG3Cam<T>`, `SonyBRC300Cam<T>`, `SonyBRCH900Cam<T>`, `SonyEVIH100Cam<T>`, and `SonyFR7Cam<T>` | Spell the profile directly: async `Camera<Profile>` or blocking `blocking::Camera<'session, Profile>`. Construction returns a session; select the view with `session.camera::<Profile>()` or `camera_for::<Profile>(target)`. |
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
| `Error::{CommandTimeout, CameraBusy, CameraMoving, CameraNotReady, CommandRejected, PresetNotFound, NoResponse, ValidationError, UnknownResponseKind}` | Delete explicit arms for these never-produced variants. Deadlines report `Timeout`; VISCA capacity/state failures use `CommandBufferFull`, `NoSocket`, or `CommandNotExecutable`; preset/value validation uses the reachable checked-value errors; capability construction returns `capabilities::ValidationError` directly. Keep a wildcard arm because `Error` remains non-exhaustive. |

Retained public protocol values also changed shape:

| 1.x/earlier RC call | 2.0 call |
| --- | --- |
| `DirectMenuControl::new(control1, control2)` returning the value directly | `DirectMenuControl::new(control1, control2)?`; the constructor rejects a data `FF` followed by an address byte because that would begin a second VISCA frame. |
| `envelope.frame_into(visca, kind, out)` returning framing metadata directly | `envelope.frame_into(visca, kind, out)?`; malformed/non-terminated Sony payloads are validation errors and leave `out` unchanged. |
| Earlier 2.0 RC matching on `FrameSequence::Lower16(value)` | Match `FrameSequence::MaybeTruncated(value)`. A zero upper half does not prove truncation; the new name preserves that uncertainty while `value()` still returns the numeric value. |
| Tuple construction or one-field matching of `ZoomTarget(position)` | `ZoomTarget::new(position)` for a raw target, or `ZoomTarget::from_normalized(value, domain, profile)?` when normalization provenance must survive until profile validation. |
| `<R as RuntimeSerial>::SerialTransport` | `<R as Runtime>::SerialTransport`; `RuntimeSerial` remains the `connect_serial` extension trait, while the associated transport type lives on `Runtime` so `TransportHandle<R>` stays runtime-paired. |

Serialization features (`serde`, `schemars`, `ts-rs`) remain opt-in data-shape
features. They do not reopen private modules or create a second semantic
registry. `test-utils` is for deterministic tests, not production construction.

With `serde`, checked data stays checked across the wire boundary:
`CapabilityRange` now requires ordered `min`/`max` endpoints and rejects
`min > max` during deserialization. Validated public scalar wrappers and values
declared by `visca_range_type!` deserialize through their checked `TryFrom`/
`new` paths, so an out-of-range scalar is rejected rather than constructing an
invalid wrapper.

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
governing deadline, and the profile busy timeout. `retry_timing` may make those
bounds more conservative; it cannot make an ambiguous raw replay safe.

**Tuning is a one-way ratchet toward *more* conservative (design decision D15).**
`OperationalTuning` may only lengthen a validated profile's timing facts, never
shorten them: an override that would undercut a category completion deadline, the
profile's `ack_timeout` floor (now 500 ms on every built-in profile), or a pacing
minimum is rejected at validation, live and at construction alike. This is
stricter than issue #542 §102, whose "more conservative only" wording constrains
*pacing* alone; 2.0 consciously extends the same rule to timeout durations. The
practical consequence a 1.x user feels: 1.x let you install an aggressively
*short*, fail-fast timeout to trip a command early, and 2.0 removes that — a
command now waits at least the profile floor before it fails. If you relied on a
sub-floor deadline for fast failure, drive that from the caller side (cancel on
your own deadline) rather than from tuning.

Two differences matter in practice.

**The runtime-mutable fields are replaced whole, not merged.** Any mutable field
left unset returns to the profile default rather than keeping a value an earlier
call installed. Build the complete set of mutable `OperationalTuning` overrides
each time. `strict_unconfirmed_poison` is the construction-only exception: an
explicit `true` or `false` is rejected by `set_tuning`, while leaving it unset
retains the session's construction-time policy and reports that retained value
through `Session::tuning`.

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

fn resubmit_after_widening<'session, O>(
    session: &Session,
    camera: &Camera<'session, PtzOpticsG2>,
    operation: Operation<'session, AppliedOnly>,
    command: O,
) -> Result<Operation<'session, AppliedOnly>, Error>
where
    O: OperationCommand<AppliedOnly>,
{
    session.set_tuning(OperationalTuning::new().ack_timeout(Duration::from_secs(2)))?;
    // `operation` was admitted before the update and keeps its old deadline.
    let _ = operation.cancel()?.outcome(Duration::from_secs(1));
    let operation = camera.submit::<AppliedOnly, _>(&command)?;
    Ok(operation)
}
```

Runtime updates are validated on exactly the grounds `SessionConfig::with_tuning`
validates on, so tuning that would have been refused at construction is refused
here too and leaves the live configuration untouched.

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
shutdown this application requested. The table also includes the two common
still-live outcomes that otherwise lead migration code into an unbounded retry
loop:

| Condition | Error | `requires_new_session()` | What to do |
| --- | --- | --- | --- |
| The peer closed the connection, including an OS TCP keepalive timeout normalized from `io::ErrorKind::TimedOut` | `ConnectionClosed` | `true` | Open a fresh session and re-query. |
| The stream position became unknowable, or the strict opt-in poisoned the session for an unconfirmable command | `StreamPoisoned` | `true` | Open a fresh session; never blindly replay uncertain work. |
| An open peer answers no built-in inquiry through its default retry policy (ten-second total-budget floor, approximately 10.05 seconds with the first backoff) | `Timeout` (`is_retryable() == true`) | `false` | Compare `MetricsSnapshot::received_frames` around bounded heartbeats; replace the session only when the application's silence threshold is met. |
| A sent unsequenced command on a raw-VISCA envelope cannot be correlated, default per-request mode (ACK/completion/cancellation ambiguity or active retry-budget expiry; the review probe reached this in about 2.56 seconds) | `UnsequencedCommandUnconfirmed` (`kind() == IoClosed`) | `false` | Reconcile that command's camera effect; do not replay it blindly or infer that the session died. |
| The blocking owner is temporarily borrowed/re-entered or otherwise cannot accept this turn | `TransportBusy` | `false` | Serialize or back off the caller; do not reconnect on this error alone. |
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
quarantined only to its known ambiguity deadline. Ordinary blocking operation
submission now waits through that bounded correlation release and writes at the
deadline instead of returning `TransportBusy`; async submission remains queued
to the same release. An intrinsically `Urgent` stop takes the safety-lane
exception immediately (subject to command pacing and actual socket capacity)
and skips the blocking pre-ACK drain. While the predecessor and stop are both
open, unsequenced ACK/error traffic is ambiguous and binds to neither. Treat a
later `UnsequencedCommandUnconfirmed` from either handle as uncertainty about
the reply, not evidence that the urgent stop failed to reach the camera.

A fatal receive closure is normalized to `ConnectionClosed`, with the
underlying transport error's text retained in its reason. `StreamPoisoned` is
reserved for a stream whose framing or write position became unknowable (for
example, a failed stream write or unrecoverable framer loss); a failed read
does not by itself establish stream poison because it consumed no bytes.

**Behavior change (issue #671).** In an earlier 2.0 preview an unconfirmable raw
command poisoned the whole session and `UnsequencedCommandUnconfirmed` mapped to
`true`. It now fails only that one command on a still-live session, so it maps to
`false`: reconcile that command's camera effect (never blindly replay it — it may
already have acted on the camera) and keep using the session for unrelated work.
A raw caller should therefore keep the session alive and reconcile before any
deliberate resubmission. If you preferred the old hard-fail behavior, opt into
`OperationalTuning::strict_unconfirmed_poison(true)`, which poisons the session
and reports `StreamPoisoned` (`true`) exactly as before.

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
    camera: &Camera<'_, PtzOpticsG2>,
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
