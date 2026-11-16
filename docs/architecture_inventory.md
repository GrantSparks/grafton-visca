# 2.0 preservation inventory

This is the release-candidate preservation baseline. It records behavior that
the 2.0 architecture must retain. The supported-surface inventory test checks
the marked closed lists below against the current registry and production
source. An intentional surface change must update the implementation and this
inventory together.

Caller-visible differences from 1.x are recorded in
[`migration_2_0.md`](migration_2_0.md).

## Profiles, envelopes, and transports

Standard transport support is checked by `TransportCompatibility::supports`, shared by configurations, profile groups and owner preflight.

The built-in profiles are `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`,
`SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, and
`GenericVisca`. The three PTZOptics profiles and `GenericVisca` support raw VISCA over TCP,
UDP, and serial, with default network ports 5678 and 1259. `SonyEviH100`,
`SonyBrc300`, and `NearusBrc300` support raw serial only. `SonyFr7` and
`SonyBrcH900` support Sony-encapsulated VISCA over UDP on port 52381. The
profile-specific capability and transport matrix remains checked in
the profile registry against `camera_profile_support.md`.

<!-- BEGIN BUILTIN PROFILES -->
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
<!-- END BUILTIN PROFILES -->

The transport contract includes TCP, UDP, blocking serial, Tokio serial,
`RawVisca`, `SonyEncapsulated`, `BlockingTransport`, `AsyncTransport`,
`BlockingTransportHandle`, `TransportHandle`, and the required
`HasTransportConfig::identity() -> TransportIdentity<'_>` descriptor. Its
`Tcp`, `Udp`, and `Serial { bus }` variants determine kind, send semantics, and
addressing together; `Custom { semantics, addressing }` declares those facts
for a caller-owned transport. Wrappers forward the identity, including the
serial Address Set report. One adapter topology check reads this addressing
source; `TransportConfig` holds I/O settings only. Transport writes accept the
owner's complete framed bytes, and blocking `send_with_timeout` takes only the
bytes and timeout. Custom transports retain the same framing/correlation
behavior as built-in transports.

## Capability gates

Every compile-time profile supplies `CompileTimeProfile::{TYPED_SUPPORT, ID,
spec}`. Built-ins generate their `ProfileSpec` directly from the registry;
downstream profiles use the same builder and validation path.

Domain gates are `HasPower`, `HasZoom`, `HasFocus`, `HasPanTilt`, `HasPresets`,
`HasExposure`, `HasWhiteBalance`, `HasImage`, and `HasMenu`, emitted from the
corresponding optional domain records. Standard transport gates are
`SupportsTcp`, `SupportsUdp`, and `SupportsSerial`, emitted from the same
transport block that builds the spec. Runtime discovery uses only
`Capabilities::supports(TypedSupportSurface)` and domain presence.

The optional typed gates are exactly the following closed list:

<!-- BEGIN OPTIONAL TYPED GATES -->
`DirectZoom`, `DigitalZoomToggle`, `DigitalZoomRange`, `Iris`, `OnePushFocus`, `SnapFocus`, `FocusLock`, `PushAutoFocus`, `FocusZone`, `AutoFocusSensitivity`, `FocusNearLimit`, `BacklightCompensation`, `WideDynamicRange`, `ExposureCompensation`, `Brightness`, `OnePushWhiteBalance`, `AutoWhiteBalanceSensitivity`, `ColorTemperature`, `RgbGain`, `RgbTuning`, `ImageFlip`, `ImageMirror`, `CombinedImageFlip`, `Contrast`, `Sharpness`, `Saturation`, `Hue`, `Luminance`, `Gamma`, `NoiseReduction2D`, `NoiseReduction3D`, `PictureEffect`, `Tally`, `DirectMenu`, `NdFilter`, `PanTiltSpeedMode`, `MotionSync`, `FocusZoneInquiry`, `UsbAudio`, `AntiFlicker`, `SettingsSave`, `PresetRecallSpeed`, `PresetRecallSpeedInquiry`, `Spotlight`, `AutoSlowShutter`, `MulticastStreaming`, `NdiQuality`, `ExposureMode`, `ImageFreeze`, `TallyBrightness`, `TallyMode`, `VersionInquiry`, `ColorTemperatureInquiry`, `NoiseReduction2DMode`, `AeLevel`, `AutoGain`, `AutoShutter`, `IrisSteps`, `AutoIris`, `PresetWhite`, `MemoryColorTemperature`, `MemoryGain`, `MemoryTint`, `AtwOffsets`, `BlackLevels`, `Detail`, `Knee`, `HighResolutionZoom`, `ClearImageZoom`, `SeparatePresetSpeed`, `SeparatePresetDuration`, `SharpnessMode`, `FocusInfinity`, `PresetMemory`, `Shutter`, `Gain`, `GainLimit`, `MenuDisplay`, `MenuEnterReturn`, `MenuDisplayInquiry`, `DirectShutter`, `PowerStandby`.
<!-- END OPTIONAL TYPED GATES -->

The generated profile registry remains
the single source for marker implementations and runtime discovery facts.

## Static nouns and controls

The noun inventory is:

<!-- BEGIN STATIC NOUNS -->
`PowerAccessor`, `ZoomAccessor`, `SystemAccessor`, `PanTiltAccessor`, `FocusAccessor`, `PresetsAccessor`, `ExposureAccessor`, `WhiteBalanceAccessor`, `ImageAccessor`, `TallyAccessor`, `NdFilterAccessor`, `MotionSyncAccessor`, `MenuAccessor`, `StreamingAccessor`, `MotionAccessor`.
<!-- END STATIC NOUNS -->

Blocking and async use the same
noun names with mode-native return types. The authoritative method inventory is
the closed semantic ledger and the generated static noun surface; profile-gated
methods require their corresponding `Has*` marker. Runtime rows require both a
typed noun-header marker and the row's refining marker, so dynamic tally
refinements cannot bypass `HasTally` (#855).
In particular, `ExposureAccessor::mode` and `ExposureAccessor::set_mode` use
`HasExposureMode` rather than inheriting broad `HasExposure` permission.
Sony FR7 has separate sourced families for relative iris movement, white
balance, image settings, high-resolution zoom and preset speed/duration;
their concept markers describe those sourced contracts and do not grant a
different shared-VISCA contract.
`HasSharpness` covers level/step operations; the independent
`HasSharpnessMode` covers auto/manual mode and its inquiry.

## Runtime-selected profiles

Both facades expose `Camera<profiles::AnyProfile>` through the same inherent
noun methods as compile-time profiles. The profile parameter changes only the
permission check: runtime views validate the registered domains, inventories,
optional permissions and encoding selection before I/O. Concrete request and
`Operation<K>` types retain their completion class. There is one owner, one
transport and one cache; no boxed trait projection or extra feature is needed.

## Connection paths

The supported paths are:

`CameraSession` owns one profile-bound camera; `CameraConfig` holds its reusable
connection settings. `Session` owns a registered target collection.

- `blocking::CameraSession::<P>::connect_tcp`, `connect_udp`, and
  `connect_serial`; the `r#async` counterparts take an explicit runtime last.
  All return the same profile-bound single-camera `CameraSession<P>` shape.
- `CameraSession::connect(&config::CameraConfig<P>)` for reusable settings;
  async construction takes the runtime last. Runtime profiles use
  `CameraConfig<AnyProfile>` with an explicit `ProfileSelection`.
- `Session::connect(&config::SessionConfig)` for a registered multi-target bus.
- `Session::open(transport, &config)` and `CameraSession::open(transport,
  &config)` for caller-owned transports, with an executor last on the async side.
- `Session::camera::<P>(target)` views and the explicit `raw` escape hatches.

Profile/transport incompatibility must still fail before protocol startup.
Blocking, runtime-neutral async, Tokio, smol, runtime coexistence, and the
documented serial feature combinations remain build-matrix requirements.

## Ecosystem, derives, testing, and extensions

The ecosystem features are `blocking`, `async`, `runtime-tokio`, `runtime-smol`,
`transport-serial`, `transport-serial-tokio`, `serde`, `schemars`, `ts-rs`,
and `test-utils`. Serialization, JSON Schema, and TypeScript exports
remain supported for the documented public value/configuration types. A persisted
current `ProfileSpec` must carry every fact key, including `Option` keys:
explicit `null` means absent, while a missing key requires regeneration (#855).
Internal
implementation-only feature flags are omitted from this public inventory.

The supported downstream value attributes are `value` and `value_enum`;
`ViscaInquiry` is the inquiry derive. Stable test utilities are:

<!-- BEGIN STABLE TEST UTILITIES -->
| Public module | Utility | Export condition |
| --- | --- | --- |
| `testing` | `CommandType` | `all(feature = "runtime-tokio", any(test, feature = "test-utils"))` |
| `testing` | `SimulatorBuilder` | `all(feature = "runtime-tokio", any(test, feature = "test-utils"))` |
| `testing` | `SimulatorConfig` | `all(feature = "runtime-tokio", any(test, feature = "test-utils"))` |
| `testing` | `ViscaCameraSimulator` | `all(feature = "runtime-tokio", any(test, feature = "test-utils"))` |
| `testing::testkit` | `Immediate` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `ScriptedTransport` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `ScriptedBlockingTransport` | `all(feature = "test-utils", feature = "blocking")` |
| `testing::testkit` | `ClockJoinHandle` | `all(feature = "test-utils", any(feature = "blocking", feature = "async"))` |
| `testing::testkit` | `ClockParticipant` | `all(feature = "test-utils", any(feature = "blocking", feature = "async"))` |
| `testing::testkit` | `ManualClock` | `all(feature = "test-utils", any(feature = "blocking", feature = "async"))` |
| `testing::testkit` | `LocalExecutor` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `ManualClockExecutor` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `ManualClockTimer` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `TaskJoin` | `all(feature = "test-utils", feature = "async")` |
| `testing::testkit` | `helpers` | `feature = "test-utils"` |
| `testing::testkit` | `DynamicResponseFn` | `feature = "test-utils"` |
| `testing::testkit` | `Step` | `feature = "test-utils"` |
<!-- END STABLE TEST UTILITIES -->

The simulator can be constructed without a runtime; sending and receiving
require Tokio with time enabled. Its fixed-seed timing/loss options schedule
addressed datagram replies, with per-camera socket cancellation and no polling
task retained after the last clone drops (#850).

The preserved extension semantics are custom command encoding, typed and raw
inquiry decoding, caller-owned blocking/async transports, raw/protocol command
and inquiry escape hatches, custom profiles with typed capability gates, and
runtime-neutral executor integration. The final 2.0 generic request surface is
on the profile-bound `Camera<P>` view in either facade: `execute` for plain
commands, `inquire` for inquiries, and
`submit` for typed operations. `with_submission_class` derives one camera view
whose complete request/noun surface uses the selected ordinary-work
`SubmissionClass`. A `CameraSession<P>` is the
single-camera owner wrapper; its `camera()` method hands out that actual
profile-bound `Camera` surface, while its submission-class helpers forward the
default to that camera rather than defining a separate request API. Callers
cannot manufacture or demote the intrinsic urgent safety class:
`SubmissionClass` has no `Urgent` variant, and every raw policy takes that
ordinary-work type. Submission APIs also accept no caller-supplied lifecycle
IDs, target, completion class, retry class, or settlement metadata.

Raw frames use `raw::CommandPolicy` for `raw::PlainRequest`, `raw::OperationPolicy`
for `raw::TargetedRequest` and `raw::AppliedOnlyRequest`, and `raw::InquiryPolicy` for
`raw::InquiryRequest`. Command and operation policies take `CommandTimeoutClass`, whose
five variants exclude `Inquiry`; inquiry timing is fixed by the request kind.
A plain command may select any `raw::RawReplyShape`, while an operation uses
`raw::OperationReplyShape`, which has no `NoReply` variant. Inquiry policy
stores only retry and submission classes. Policy constructors are infallible;
raw request constructors validate wire bytes.

## Allocation baselines

Allocation behavior is characterized at the narrowest stable boundary:

- `tests/issue_548_allocation_baselines.rs` checks zero-allocation built-in
  `Request::write_into` encoding and warmed raw/Sony framing-buffer reuse.
- `command::encode` keeps built-in encoded bytes inline for the current hot
  path; the runtime engine unit test
  `runtime::engine::inert_wire_is_inline_and_reused_across_retry_without_reallocation`
  proves retries reuse the exact encoded `Arc` rather than rebuilding wire
  data, and
  `tests/issue_548_allocation_baselines.rs::warmed_raw_and_sony_retries_reuse_owner_buffers` proves the framing buffers
  are reused with it.
- `tests/issue_561_blocking_queue.rs` and the bounded owner-submission and
  completion-subscriber unit tests in `runtime::owner` establish the current
  lifecycle/storage bounds.
- `tests/dyn_api_integration_test.rs::runtime_profile_future_construction_matches_static_without_allocating`
  establishes the runtime-profile/static future-construction baseline.
- `tests/issue_548_allocation_baselines.rs::{blocking_builtin_camera_and_into_typed_views_do_not_allocate,async_builtin_camera_and_into_typed_views_do_not_allocate}`
  prove typed views reuse validated built-in profile identity without allocation.
- `tests/issue_517_inquiry_encoding.rs` retains the downstream derive encoding baseline.

These are characterization floors, not permission for 2.0 to add per-retry,
per-frame, per-handle, or unbounded lifecycle allocation.
