# 2.0 preservation inventory

This is the release-candidate preservation baseline for issue #542. It records
behavior that the 2.0 architecture must retain. The supported-surface inventory
test checks the closed lists below against the current registry and source; an
intentional change must update the implementation, this inventory, and that
test together.

## Profiles, envelopes, and transports

The built-in profiles are `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`,
`SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, and
`GenericVisca`. The first three and last four support raw VISCA over TCP, UDP,
and serial, with default network ports 5678 and 1259. `SonyFR7` and
`SonyBRCH900` support Sony-encapsulated VISCA over UDP on port 52381. The
profile-specific capability and transport matrix remains checked in
`camera::profile_registry` against `camera_profile_support.md`.

The transport contract includes TCP, UDP, blocking serial, Tokio serial,
`RawVisca`, `SonyEncapsulated`, `BlockingTransport`, `AsyncTransport`,
`BlockingTransportHandle`, and `TransportHandle`. Custom transports retain
send semantics (`Stream` versus `Datagram`), transport configuration, and the
same framing/correlation behavior as built-in transports.

## Capability gates

Every profile retains the baseline profile facets `ProfileMetadata`,
`PanTilt`, `Zoom`, `Focus`, `Exposure`, `WhiteBalance`, `ImageProcessing`,
`Presets`, `Power`, `MenuCapability`, `Tally`, `MotionSyncMetadata`,
`NdFilterMetadata`, `VariableSpeedMetadata`, and `ProfileTypedSupport`.
Standard transport gates are `SupportsTcp`, `SupportsUdp`, and
`SupportsSerial`.

The optional typed gates are exactly: `DirectZoom`, `DigitalZoomToggle`,
`DigitalZoomRange`, `IrisControl`, `OnePushFocus`, `PtzOpticsSnapFocus`,
`FocusLock`, `PushAutoFocus`, `FocusZone`, `AutoFocusSensitivity`,
`FocusNearLimitInquiry`, `BacklightCompensation`, `WideDynamicRange`,
`ExposureCompensation`, `BrightnessControl`, `OnePushWhiteBalance`,
`AutoTrackingWhiteBalance`, `AutoWhiteBalanceSensitivity`, `ColorTemperature`,
`RgbGain`, `RgbTuning`, `ImageFlip`, `ImageMirror`, `CombinedImageFlip`,
`ContrastControl`, `SharpnessControl`, `SaturationControl`, `HueControl`,
`LuminanceControl`, `GammaControl`, `NoiseReduction`, `NoiseReduction2D`,
`NoiseReduction3D`, `PictureEffect`, `Tally`, `DirectMenu`, `NdFilter`,
`VariableSpeed`, and `MotionSync`. The generated profile registry remains the
single source for marker implementations and runtime discovery facts.

## Static nouns and controls

The noun inventory is `PowerAccessor`, `ZoomAccessor`, `SystemAccessor`,
`PanTiltAccessor`, `FocusAccessor`, `ExposureAccessor`,
`WhiteBalanceAccessor`, `ImageAccessor`, `PresetsAccessor`, `TallyAccessor`,
`NdFilterAccessor`, `MotionSyncAccessor`, `MenuAccessor`, and
`AdvancedAccessor`, plus the `MotionAccessor`. Blocking and async use the same
noun names with mode-native return types. The authoritative method inventory is
the closed semantic ledger and the generated static noun surface; profile-gated
methods require their corresponding `Has*` marker.

## Dynamic controls

The dynamic surface is `DynSessionCamera` plus the object-safe
`DynSessionCameraControl`, `DynSessionCameraNouns`, the 14 `Dyn*` noun traits,
and `DynMotion`. It also exposes `DynTargetedOperation`,
`DynAppliedOperation`, and `DynCancellation`. Its checked inventory covers
143 target-facing command methods and 66 typed inquiry methods, with the same
semantic classes as the static surface. Dynamic projection erases profile and
request types only; it does not introduce another runtime, owner, cancellation,
deadline, outcome, or settling policy.

## Connection paths

The supported paths are:

- `Connect` convenience construction for blocking TCP/UDP/serial and async
  TCP/UDP/Tokio-serial, including its typed connection builders.
- `CameraConfig` construction for the same supported profile/transport pairs,
  with runtime-selected Tokio or smol async execution.
- `blocking::Session::open` or async `Session::open` construction from a
  caller-owned transport and shared `SessionConfig`.
- `Session` camera views and the explicit `raw` request escape hatches.

Profile/transport incompatibility must still fail before protocol startup.
Blocking, runtime-neutral async, Tokio, smol, runtime coexistence, and the
documented serial feature combinations remain build-matrix requirements.

## Ecosystem, derives, testing, and extensions

The ecosystem features are `blocking`, `async`, `runtime-tokio`, `runtime-smol`,
`transport-serial`, `transport-serial-tokio`, `serde`, `schemars`, `ts-rs`,
`dyn-api`, and `test-utils`. Serialization, JSON Schema, and TypeScript exports
remain supported for the documented public value/configuration types. Internal
implementation-only feature flags are omitted from this public inventory.

The supported downstream derives are `ViscaInquiry`, `ViscaEnum`, and
`ViscaValue`. Stable test utilities are `Step`, `helpers`,
`ScriptedBlockingTransport`, `ScriptedTransport`, `DeterministicExecutor`,
`DeterministicClock`, `DeterministicExecutorExt`, `TestExecutorSelector`,
`TestExecutorType`, `TestExecutors`, and the Tokio `ViscaCameraSimulator`.

The preserved extension semantics are custom command encoding, typed and raw
inquiry decoding, caller-owned blocking/async transports, raw/protocol command
and inquiry escape hatches, custom profiles with typed capability gates, and
runtime-neutral executor integration. The final 2.0 generic request surface is
only `execute` for plain commands, `inquire` for inquiries, and `submit` for
typed operations; callers cannot inject lifecycle IDs, priority, target,
completion class, retry class, or settlement metadata at submission time.

## Allocation baselines

Allocation behavior is characterized at the narrowest stable boundary:

- `issue_548_allocation_baselines` checks zero-allocation built-in
  `write_into` encoding and warmed raw/Sony framing-buffer reuse.
- `command::encode` keeps built-in encoded bytes inline for the current hot
  path; the runtime engine retry unit test
  `test_command_kind_preserved_through_retries`
  proves retries reuse the exact encoded `Arc` rather than rebuilding wire data.
- The bounded owner-submission and completion-subscriber unit tests establish
  the current lifecycle/storage bounds.
- `dyn_api_integration_test::test_dyn_direct_forwarding_future_construction_allocations_match_static`
  establishes the dyn/static future-construction baseline.
- `issue_517_inquiry_encoding` retains the downstream derive encoding baseline.

These are characterization floors, not permission for 2.0 to add per-retry,
per-frame, per-handle, or unbounded lifecycle allocation.
