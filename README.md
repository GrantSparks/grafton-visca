# grafton‑visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](#license)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

A pure Rust library for controlling PTZ cameras via the VISCA protocol. Supports blocking and async APIs with built-in runtime adapters (Tokio and smol), TCP/UDP/serial transports, and type-safe camera profiles.

> **2.0.0** — The owner-backed API keeps one protocol owner per session and
> exposes compile-time and runtime-selected profile views in blocking and async
> over that owner.

---

## 2.0.0 Support Matrix

The 2.0 contract is owner-backed construction, mode-native sessions, typed
profile views, and one request path for each semantic class. Source-backed
profile capability gates and automated protocol tests define software support.

The 2.0 release is a clean break from the pre-2.0 API. Use
`config::SessionConfig` for reusable target/profile registration,
`CameraSession::<P>::connect_tcp` or `connect_udp` for standard connections,
and `Session::open` for caller-owned transports. Reusable single-camera data
lives in `config::CameraConfig<P>`.

### APIs and runtimes

| Area | Supported 2.0 contract | Automated validation |
| ---- | ---------------------- | -------------------- |
| Blocking API | `blocking::Session`, `blocking::Camera<P>`, and blocking noun views; native synchronous I/O with no async runtime/executor dependency | Blocking API contracts plus dependency-graph gate |
| Runtime-neutral async | `r#async::{Session, Camera<P>}` and a caller-provided `runtime::Executor` | Async API contract tests |
| Tokio async | `runtime-tokio`, `TokioRuntime`, and Tokio TCP/UDP adapters | Tokio construction and noun suites |
| smol async | `runtime-smol`, `SmolRuntime`, and smol TCP/UDP adapters | smol construction and noun suites |
| Runtime coexistence | Tokio and smol may be enabled together; each session receives one explicit runtime | Coexistence API contract |
| Runtime-profile API | `Camera<AnyProfile>` and the same concrete `Operation<K>` handles in each facade; runtime capability checks run before I/O | Runtime-profile validation suites |
| Request extension | Typed `Request`, `PlainCommand`, `Inquiry`, and `OperationCommand` contracts; `ResponseParser` handles custom inquiry decoding | Request/API contract tests |

### Transports

| Transport | Supported 2.0 contract | Automated validation |
| --------- | ---------------------- | -------------------- |
| TCP | Blocking, Tokio, and smol owner sessions through `CameraSession::connect` and `config::CameraConfig` | Runtime matrix above |
| UDP | Blocking, Tokio, and smol owner sessions through `CameraSession::connect` and `config::CameraConfig` | Runtime matrix above |
| Blocking serial | RS-232/422 through `transport-serial` and `blocking::Session` | Serial feature matrix |
| Tokio serial | Tokio serial through `transport-serial-tokio` and `Session` | Tokio serial feature matrix |
| Custom transports | Caller-owned `BlockingTransport`/`AsyncTransport` with `Session::open` | Custom transport contract tests |

### Profiles

| Profile family | Protocol | 2.0 support |
| -------------- | -------- | ----------- |
| `GenericVisca` | Raw VISCA | Supported baseline profile with conservative capabilities |
| `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` | Raw VISCA | Supported through profile capability gates and protocol tests |
| `SonyEviH100`, `SonyBrc300`, `NearusBrc300` | Raw VISCA | Supported through profile capability gates and protocol tests |
| `SonyBrcH900`, `SonyFr7` | Sony encapsulation | Supported through Sony encapsulation, profile capability gates, and protocol tests |

Built-in identifiers have explicit persisted spellings:
`ptzoptics_g2`, `ptzoptics_g3`, `ptzoptics_30x`, `sony_fr7`,
`sony_brc_h900`, `sony_evi_h100`, `sony_brc_300`, `nearus_brc_300`, and
`generic_visca`. Resolve one with `profiles::ProfileId::spec()` and inspect its
validated capabilities and transports. Compile-time profiles implement
`profiles::CompileTimeProfile` with the same spec contract; the registry emits
their evidence markers and transport markers from that one declaration.

### Profile-Gated Vendor Controls

Typed vendor-specific controls are exposed only for profiles whose documented
model capabilities support them. Runtime capability metadata remains available for
discovery on every profile, and raw command escape hatches remain available for
custom integrations.

<!-- BEGIN PROFILE SUPPORT SURFACES -->
| Typed surface | Marker | Built-in profiles |
| --- | --- | --- |
| `DirectZoom` | `HasDirectZoom` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `DigitalZoomToggle` | `HasDigitalZoomToggle` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `DigitalZoomRange` | `HasDigitalZoomRange` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `Iris` | `HasIris` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `OnePushFocus` | `HasOnePushFocus` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `SnapFocus` | `HasSnapFocus` | No built-in profile |
| `FocusLock` | `HasFocusLock` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `PushAutoFocus` | `HasPushAutoFocus` | `SonyFr7` |
| `FocusZone` | `HasFocusZone` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `AutoFocusSensitivity` | `HasAutoFocusSensitivity` | No built-in profile |
| `FocusNearLimit` | `HasFocusNearLimit` | `SonyEviH100` |
| `BacklightCompensation` | `HasBacklightCompensation` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `WideDynamicRange` | `HasWideDynamicRange` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `ExposureCompensation` | `HasExposureCompensation` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `Brightness` | `HasBrightness` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `OnePushWhiteBalance` | `HasOnePushWhiteBalance` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `AutoWhiteBalanceSensitivity` | `HasAutoWhiteBalanceSensitivity` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `ColorTemperature` | `HasColorTemperature` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `RgbGain` | `HasRgbGain` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `RgbTuning` | `HasRgbTuning` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `ImageFlip` | `HasImageFlip` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrc300`, `NearusBrc300` |
| `ImageMirror` | `HasImageMirror` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `CombinedImageFlip` | `HasCombinedImageFlip` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `Contrast` | `HasContrast` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `Sharpness` | `HasSharpness` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `Saturation` | `HasSaturation` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `Hue` | `HasHue` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `Luminance` | `HasLuminance` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `Gamma` | `HasGamma` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `NoiseReduction2D` | `HasNoiseReduction2D` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `NoiseReduction3D` | `HasNoiseReduction3D` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `PictureEffect` | `HasPictureEffect` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `Tally` | `HasTally` | `SonyFr7` |
| `DirectMenu` | `HasDirectMenu` | `SonyFr7` |
| `NdFilter` | `HasNdFilter` | `SonyFr7` |
| `PanTiltSpeedMode` | `HasPanTiltSpeedMode` | `SonyFr7` |
| `MotionSync` | `HasMotionSync` | No built-in profile |
| `FocusZoneInquiry` | `HasFocusZoneInquiry` | `PtzOpticsG2`, `PtzOptics30x` |
| `UsbAudio` | `HasUsbAudio` | `PtzOpticsG2`, `PtzOptics30x` |
| `AntiFlicker` | `HasAntiFlicker` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `SettingsSave` | `HasSettingsSave` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `PresetRecallSpeed` | `HasPresetRecallSpeed` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7` |
| `PresetRecallSpeedInquiry` | `HasPresetRecallSpeedInquiry` | `SonyFr7` |
| `Spotlight` | `HasSpotlight` | `SonyFr7` |
| `AutoSlowShutter` | `HasAutoSlowShutter` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `MulticastStreaming` | `HasMulticastStreaming` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `NdiQuality` | `HasNdiQuality` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `ExposureMode` | `HasExposureMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `ImageFreeze` | `HasImageFreeze` | No built-in profile |
| `TallyBrightness` | `HasTallyBrightness` | No built-in profile |
| `TallyMode` | `HasTallyMode` | No built-in profile |
| `VersionInquiry` | `HasVersionInquiry` | `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `ColorTemperatureInquiry` | `HasColorTemperatureInquiry` | `PtzOpticsG2`, `PtzOptics30x` |
| `NoiseReduction2DMode` | `HasNoiseReduction2DMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `AeLevel` | `HasAeLevel` | `SonyFr7` |
| `AutoGain` | `HasAutoGain` | `SonyFr7` |
| `AutoShutter` | `HasAutoShutter` | `SonyFr7` |
| `IrisSteps` | `HasIrisSteps` | `SonyFr7` |
| `AutoIris` | `HasAutoIris` | `SonyFr7` |
| `PresetWhite` | `HasPresetWhite` | `SonyFr7` |
| `MemoryColorTemperature` | `HasMemoryColorTemperature` | `SonyFr7` |
| `MemoryGain` | `HasMemoryGain` | `SonyFr7` |
| `MemoryTint` | `HasMemoryTint` | `SonyFr7` |
| `AtwOffsets` | `HasAtwOffsets` | `SonyFr7` |
| `BlackLevels` | `HasBlackLevels` | `SonyFr7` |
| `Detail` | `HasDetail` | `SonyFr7` |
| `Knee` | `HasKnee` | `SonyFr7` |
| `HighResolutionZoom` | `HasHighResolutionZoom` | `SonyFr7` |
| `ClearImageZoom` | `HasClearImageZoom` | `SonyFr7` |
| `SeparatePresetSpeed` | `HasSeparatePresetSpeed` | `SonyFr7` |
| `SeparatePresetDuration` | `HasSeparatePresetDuration` | `SonyFr7` |
| `SharpnessMode` | `HasSharpnessMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `FocusInfinity` | `HasFocusInfinity` | `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `PresetMemory` | `HasPresetMemory` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `Shutter` | `HasShutter` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `Gain` | `HasGain` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `GainLimit` | `HasGainLimit` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `MenuDisplay` | `HasMenuDisplay` | `SonyFr7` |
| `MenuEnterReturn` | `HasMenuEnterReturn` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `MenuDisplayInquiry` | `HasMenuDisplayInquiry` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900` |
| `DirectShutter` | `HasDirectShutter` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `PowerStandby` | `HasPowerStandby` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100` |
<!-- END PROFILE SUPPORT SURFACES -->

The Sony FR7 tally row refers to the source-backed red/green tally controls and
inquiries. Packed status and auto-adjust inquiries are removed; PTZOptics tally
mode and extended brightness controls require separate markers and have no
built-in profile grant.

FR7 also has model-specific AE-level adjustments, automatic gain/shutter/iris,
counted iris movement, preset white and Memory A white balance, ATW offsets,
black/detail/knee settings, high-resolution zoom, Clear Image Zoom readback,
and preset speed/duration settings. Concept markers such as `HasIrisSteps`,
`HasAutoIris`, `HasPresetWhite`, and `HasSeparatePresetDuration` describe
those contracts. Lens availability, exposure mode and camera runtime state can still
make a documented command temporarily unavailable; see the
[source and profile guidance](docs/camera_profile_support.md).

### Profile-Gated Sub-Capabilities

Broad control families are also decomposed when model support differs. For
example, PTZOptics profiles still expose baseline zoom, exposure, and focus
controls, but do not expose typed VISCA digital zoom, one-push focus, or snap
focus methods.

Runtime-profile callers get the same marker-derived permission model through
`Capabilities::supports(...)`. Sourced facts live in optional domain records:
`caps.zoom.as_ref()` yields the lens maxima and direct-positioning fact, while
`caps.zoom.is_some()` reports domain presence.
Use `camera.capabilities().supports(surface)` to discover optional permissions;
runtime-selected noun calls validate the registered profile before I/O.

Domain support also leaves individual methods gated: H900 exposes shutter steps
and readback without direct positioning, and some power profiles have no standby
method. See the [optional method permissions](docs/migration_2_0.md#optional-method-permissions)
for the method requirements, built-in restrictions, and custom profile opt-in.

| Runtime domain | Owning facts |
| --- | --- |
| `pan_tilt: Option<PanTiltCaps>` | Raw bounds, conversion, nominal endpoint policy, speed ranges and mount-selected tilt limits |
| `zoom: Option<ZoomCaps>` | Optical maximum, optional digital maximum, speed range and lens ratio |
| `focus`, `exposure`, `white_balance`, `image` | Each domain's sourced numeric ranges and inventories |
| `presets: Option<PresetCaps>` | Highest memory number, exact recall axes and optional nonzero name length |
| `power: Option<PowerCaps>` | Power-on timing and optional standby timing |
| `menu: Option<MenuCaps>` | Source-backed menu domain presence |

Profile defaults use `IndividualCameraId` (`1..=7`); broadcast cannot become a
session default. Persisted profiles use the current grouped shape and must be
regenerated when upgrading from the flat capability record.

<!-- BEGIN PROFILE NOUN COVERAGE -->
| Noun | Built-in method availability |
| --- | --- |
| `power()` | `PtzOpticsG2`: 3/3; `PtzOpticsG3`: 3/3; `PtzOptics30x`: 3/3; `SonyFr7`: 3/3; `SonyBrcH900`: 3/3; `SonyEviH100`: 3/3; `SonyBrc300`: 2/3; `NearusBrc300`: 2/3; `GenericVisca`: 2/3 |
| `zoom()` | `PtzOpticsG2`: 9/14; `PtzOpticsG3`: 9/14; `PtzOptics30x`: 9/14; `SonyFr7`: 13/14; `SonyBrcH900`: 6/14; `SonyEviH100`: 10/14; `SonyBrc300`: 10/14; `NearusBrc300`: 10/14; `GenericVisca`: 6/14 |
| `system()` | `PtzOpticsG2`: 1/2; `PtzOpticsG3`: 1/2; `PtzOptics30x`: 1/2; `SonyFr7`: 1/2; `SonyBrcH900`: 1/2; `SonyEviH100`: 1/2; `SonyBrc300`: 1/2; `NearusBrc300`: 1/2; `GenericVisca`: 1/2 |
| `pan_tilt()` | `PtzOpticsG2`: 9/10; `PtzOpticsG3`: 9/10; `PtzOptics30x`: 9/10; `SonyFr7`: 10/10; `SonyBrcH900`: 9/10; `SonyEviH100`: 9/10; `SonyBrc300`: 9/10; `NearusBrc300`: 9/10; `GenericVisca`: 9/10 |
| `focus()` | `PtzOpticsG2`: 13/22; `PtzOpticsG3`: 12/22; `PtzOptics30x`: 13/22; `SonyFr7`: 12/22; `SonyBrcH900`: 11/22; `SonyEviH100`: 14/22; `SonyBrc300`: 12/22; `NearusBrc300`: 12/22; `GenericVisca`: 12/22 |
| `presets()` | `PtzOpticsG2`: 4/13; `PtzOpticsG3`: 4/13; `PtzOptics30x`: 4/13; `SonyFr7`: 13/13; `SonyBrcH900`: 1/13; `SonyEviH100`: 3/13; `SonyBrc300`: 3/13; `NearusBrc300`: 3/13; `GenericVisca`: 3/13 |
| `exposure()` | `PtzOpticsG2`: 36/49; `PtzOpticsG3`: 36/49; `PtzOptics30x`: 36/49; `SonyFr7`: 14/49; `SonyBrcH900`: 16/49; `SonyEviH100`: 32/49; `SonyBrc300`: 32/49; `NearusBrc300`: 32/49; `GenericVisca`: 17/49 |
| `white_balance()` | `PtzOpticsG2`: 24/52; `PtzOpticsG3`: 23/52; `PtzOptics30x`: 24/52; `SonyFr7`: 31/52; `SonyBrcH900`: 2/52; `SonyEviH100`: 13/52; `SonyBrc300`: 13/52; `NearusBrc300`: 13/52; `GenericVisca`: 3/52 |
| `image()` | `PtzOpticsG2`: 30/54; `PtzOpticsG3`: 30/54; `PtzOptics30x`: 30/54; `SonyFr7`: 28/54; `SonyBrcH900`: 0/54; `SonyEviH100`: 13/54; `SonyBrc300`: 2/54; `NearusBrc300`: 2/54; `GenericVisca`: 0/54 |
| `tally()` | `PtzOpticsG2`: 0/6; `PtzOpticsG3`: 0/6; `PtzOptics30x`: 0/6; `SonyFr7`: 4/6; `SonyBrcH900`: 0/6; `SonyEviH100`: 0/6; `SonyBrc300`: 0/6; `NearusBrc300`: 0/6; `GenericVisca`: 0/6 |
| `nd_filter()` | `PtzOpticsG2`: 0/7; `PtzOpticsG3`: 0/7; `PtzOptics30x`: 0/7; `SonyFr7`: 7/7; `SonyBrcH900`: 0/7; `SonyEviH100`: 0/7; `SonyBrc300`: 0/7; `NearusBrc300`: 0/7; `GenericVisca`: 0/7 |
| `motion_sync()` | `PtzOpticsG2`: 0/2; `PtzOpticsG3`: 0/2; `PtzOptics30x`: 0/2; `SonyFr7`: 0/2; `SonyBrcH900`: 0/2; `SonyEviH100`: 0/2; `SonyBrc300`: 0/2; `NearusBrc300`: 0/2; `GenericVisca`: 0/2 |
| `menu()` | `PtzOpticsG2`: 4/7; `PtzOpticsG3`: 4/7; `PtzOptics30x`: 4/7; `SonyFr7`: 5/7; `SonyBrcH900`: 2/7; `SonyEviH100`: 1/7; `SonyBrc300`: 1/7; `NearusBrc300`: 1/7; `GenericVisca`: 1/7 |
| `streaming()` | `PtzOpticsG2`: 4/4; `PtzOpticsG3`: 2/4; `PtzOptics30x`: 4/4; `SonyFr7`: 0/4; `SonyBrcH900`: 0/4; `SonyEviH100`: 0/4; `SonyBrc300`: 0/4; `NearusBrc300`: 0/4; `GenericVisca`: 0/4 |
| `motion()` | `PtzOpticsG2`: 3/3; `PtzOpticsG3`: 3/3; `PtzOptics30x`: 3/3; `SonyFr7`: 3/3; `SonyBrcH900`: 3/3; `SonyEviH100`: 3/3; `SonyBrc300`: 3/3; `NearusBrc300`: 3/3; `GenericVisca`: 3/3 |
<!-- END PROFILE NOUN COVERAGE -->

White-balance mode support comes from `WhiteBalanceCaps::modes`. FR7's
inventory includes `Atw`, `Manual` (Memory A), and `Preset`; mode-setting
requests validate that inventory. Separate standard-mode and ATW markers are
not required.

`HasExposure` remains the broad exposure-domain marker. The shared `04 39`
exposure-mode command and inquiry are separately guarded by `HasExposureMode`;
every built-in profile except `SonyFr7` has a nonempty shared-mode inventory and
grants this typed surface. The PTZOptics profiles use R1/R10/R14 evidence;
`SonyBrcH900`, `SonyEviH100`, `SonyBrc300` and `NearusBrc300` use the command
and inquiry rows of their own model sources (R11, R8, R12 and R21);
`GenericVisca` grants only the Sony-standard families that R8, R12 and R21 all
document. A discovery inventory and the marker agree for every built-in;
custom runtime profiles must still supply both before encoding.

`HasIris` covers standard iris reset/up/down/direct control and the
distinct `09 04 4B` position inquiry exposed as `iris()`. R10 sources that
surface specifically for `PtzOpticsG3`; the current PTZOptics G2/G3 Developer
Portal (R14 in [`docs/visca_reference.md`](docs/visca_reference.md)) sources it
for `PtzOpticsG2` and `PtzOpticsG3`; raw `PtzOptics30x` uses separate
PTZOptics Gen-2/R1 evidence; and the Sony-standard profiles follow the same
R11/R8/R12/R21 model rows described above. The unsourced auto/manual-status
inquiry has no typed built-in model. A caller-owned request and decoder can
represent an extension once its packet and reply are established for that
camera. The Sony FR7 command list instead documents a vendor-relative `7E 04 4B`
iris Up/Down family and `05 34` Auto Iris inquiry, not the shared `04 39`
AE-mode command/inquiry family, standard absolute Iris Direct command, or `09
04 4B` position inquiry. Use `iris_up_by` and `iris_down_by` under `HasIrisSteps`, and
`set_auto_iris` and `auto_iris` under `HasAutoIris`, through `camera.exposure()`; these do not
provide an absolute iris position or its readback.

`IrisLevel` represents the `0x00..=0x1E` union used by the built-in iris
profiles, and `GainLevel` represents the union of built-in gain codes `0x00..=0x19`.
The selected profile's advertised range remains the final admission check, so
constructing a syntactically valid value never grants it to a camera that does
not document it. Coarse `SpeedLevel` position requests similarly clamp to each
profile's documented pan and tilt maxima; `Fastest` therefore means the fastest
valid speed for that selected camera.

Noise-reduction level controls and inquiries share `HasNoiseReduction2D` or
`HasNoiseReduction3D`. The separate `04 50` auto/manual mode
(`noise_reduction_2d_mode`, `set_noise_reduction_2d_mode`) requires
`HasNoiseReduction2DMode`. The
EVI-H100 carries only the 2D level pair, because R8 documents `CAM_NR`
`04 53` and its inquiry but no mode or 3D level. This is exact source-backed scope, not a grant inferred
from a PTZOptics family grouping: `PtzOptics30x` is the explicitly legacy
PT30X SDI/NDI G2/Gen-2 profile, and it does not extend to Move, Link, or newer
30X models. See the separate R14 command-input and query-output domains in
[`docs/visca_reference.md`](docs/visca_reference.md); a successful set is not
promised to round-trip through an inquiry with the same numeric value.
The single grant for each level family records its command and inquiry
evidence together; domain facts do not create a second permission bit.

Contributors adding or changing profile capabilities should follow the
[Camera Profile Support Guide](docs/camera_profile_support.md) and the
[VISCA Protocol Reference](docs/visca_reference.md), which explain how protocol
evidence, runtime metadata, typed support markers, and the profile-first marker
matrix fit together.

### Optional features

| Feature | 2.0 support |
| ------- | ----------- |
| `blocking` | Canonical native blocking facade (enabled by default); it does not enable or link Tokio or another async runtime/executor |
| `async` | Canonical owner-backed async facade; may be enabled with `blocking` |
| `serde` | Stable serialization/deserialization for public value and configuration types |
| `schemars` | Stable JSON Schema generation for serde-backed public types |
| `ts-rs` | Stable TypeScript type generation for supported exported types |
| `test-utils` | Exposes the stable `grafton_visca::testing` module. It enables no facade of its own, so what it exposes depends on the union: `testkit::{Step, helpers}` always, `ScriptedBlockingTransport` with `blocking`, `ManualClock` and `ClockParticipant` with either facade, `ScriptedTransport`, `LocalExecutor` and `ManualClockExecutor` with `async`, and `ViscaCameraSimulator` only with `runtime-tokio`. Neither `runtime-tokio` nor `blocking` exposes any of it on its own. |

### Feature-union checks

Each union below is a CI matrix leg, named by its job title in the feature
matrix of `.github/workflows/ci.yml`.

| Feature union | Scope | Automated validation |
| ------------- | ----- | -------------------- |
| `blocking,async` | Native blocking and runtime-neutral async facades together. | `blocking + runtime-neutral async` matrix leg |
| `runtime-tokio,transport-serial` | Blocking serial plus Tokio async dependency coexistence; use `transport-serial-tokio` for Tokio serial. | `Tokio + blocking serial` matrix leg |
| `runtime-smol` | Async calls with compile-time or runtime-selected profiles and no test helpers. | `smol runtime` matrix leg |
| `test-utils,blocking,runtime-tokio` | The shipped test toolkit driven through a real facade. `test-utils` alone enables neither `blocking` nor a runtime, so the scripted transports, deterministic executor, and camera simulator only *execute* under a union like this one. | `test-utils + blocking + Tokio` matrix leg |

---

## Features

- **Camera-first blocking/async API** — inherent `CameraSession` constructors and reusable `config::CameraConfig`, and shared noun accessors across blocking and async
- **Multi-runtime support** — Pluggable adapters for Tokio and smol
- **Type-safe profiles** — Compile-time protocol selection (raw VISCA vs Sony encapsulation) with capability-based APIs
- **Flexible transports** — TCP, UDP, and serial (RS-232/422) with configurable timeouts, retries, and TCP keepalive
- **Ergonomic API** — One-line connection helpers, checked unit types (`Degrees` for typed pan/tilt input and `UnitInterval` for normalized values), built-in inquiry conversions
- **Optional serialization** — Serde and JSON Schema support for public value and configuration types
- **Zero-allocation hot path** — Stack-allocated command buffers for standard VISCA commands

---

## Quick Start

The quickstart snippets are read-only. They connect to a camera, query state,
and close the session. Movement and configuration changes are shown in focused
examples that opt in to hardware changes explicitly.

Standard TCP and UDP sessions support one camera. `CameraSession::<P>::connect_tcp` and
`CameraSession::<P>::connect_udp` return an owner-backed `CameraSession<P>`: the profile is named
once and bound at compile time, and `session.camera()` hands out the typed
`Camera` view that carries the noun accessors. Multi-target sessions use the
serial or caller-owned transport paths and select each view with
`session.camera::<P>(target)`.

Every Rust snippet in this README is compiled by the crate's own test suite, so
the `#[cfg(feature = "...")]` attributes below are load-bearing: they name the
Cargo feature a snippet needs.

### Blocking

```rust
use grafton_visca::blocking::CameraSession;
use grafton_visca::profiles::PtzOpticsG2;

fn quick_start() -> Result<(), grafton_visca::Error> {
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
    let camera = session.camera();

    let power_status = camera.power().status()?;
    let zoom = camera.zoom().position()?;
    println!("Power: {power_status:?}");
    println!("Zoom position: 0x{:04X}", zoom.value());

    session.close()
}
```

### Async (Tokio)

Enable the `runtime-tokio` feature and call this from inside a Tokio runtime.

```rust
#[cfg(feature = "runtime-tokio")]
async fn quick_start() -> Result<(), grafton_visca::Error> {
    use grafton_visca::profiles::PtzOpticsG2;
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::r#async::CameraSession;

    let runtime = TokioRuntime::from_current()?;
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
    let camera = session.camera();

    let power_status = camera.power().status().await?;
    let zoom = camera.zoom().position().await?;
    println!("Power: {power_status:?}");
    println!("Zoom position: 0x{:04X}", zoom.value());

    session.close().await
}
```

For smol, enable `runtime-smol` and use `SmolRuntime`.

---

## Bounded Movement Operations

Ordinary noun methods remain the simplest command-completion surface. When a
caller needs a per-command deadline, cancellation, or a profile-selected
protocol-settlement wait,
submit a built-in movement command and drive its operation handle explicitly.

`submit` manages lifecycle and runs the request's target validation before
encoding or owner admission. Prefer typed noun controls for
profile-validated ergonomic input. Built-in commands provide exact completion
metadata. Custom operation requests must declare their targeted or applied-only
class explicitly.

```rust
use grafton_visca::blocking::CameraSession;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::command::{pan_tilt, zoom};

fn move_home() -> Result<(), grafton_visca::Error> {
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
    let camera = session.camera();

    // Blocking submit returns after owner admission; waits observe the outcome.
    camera.submit(&pan_tilt::Home)?.settled()?;

    // Applied-only commands have no meaningful settled state.
    camera.submit(&zoom::Stop)?.applied()?;

    session.close()
}
```

The typed noun accessors return the same handles, so
`camera.pan_tilt().home()?.settled()?` and `camera.zoom().stop()?.applied()?`
are the profile-validated equivalents of the two `submit` calls above.

The async form has the same vocabulary and awaits submission and terminal
operations:

```rust
#[cfg(feature = "runtime-tokio")]
async fn move_home() -> Result<(), grafton_visca::Error> {
    use grafton_visca::profiles::PtzOpticsG2;
    use grafton_visca::command::{pan_tilt, zoom};
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::r#async::CameraSession;

    let runtime = TokioRuntime::from_current()?;
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
    let camera = session.camera();

    let mut handle = camera.submit(&pan_tilt::Home).await?;
    handle.applied().await?;
    handle.settled().await?;

    camera.submit(&zoom::Stop).await?.applied().await?;

    session.close().await
}
```

- `applied` means the exact command was accepted and protocol-completed.
- `settled` returns `Settlement` evidence: exact profile-declared completion,
  or stable samples reporting their axes, window, and tolerance. Stable samples
  do not prove arrival at the requested endpoint. A later conflicting admission
  supersedes unfinished polled settlement; previously cached evidence remains
  valid. Settlement describes protocol evidence rather than a guarantee that
  physical motion has ended.
- Waits borrow the handle, and the handle caches what it observes: `applied`
  then `settled` on one handle continues from the cached application, a
  repeated wait answers from the cache, and a wait that times out or loses a
  `select!` ends only that wait.
- `cancel` requests owner-owned, ID/socket-safe cancellation and waits for its
  conclusion: `Cancelled`, or `Completed` when the operation applied first.
  It is idempotent; a handle has one cancellation intent, and a repeated
  `cancel` observes it. Queued work is removable locally; sent work requires
  profile support and otherwise returns `Error::CancellationNotSupported`, leaving the
  operation running and the handle observing it. Success does not prove
  physical motion stopped, so use a bounded STOP for continuous movement.
- Dropping a handle never stops hardware. Drop is `detach`: the submitted
  command remains owner-owned, may still be dispatched and complete, and
  physical movement continues, so an early `?` or a panic leaves the camera
  driving until something ends it. This matches 1.x and is not a 2.0 change.
  To bound movement by a scope, write a guard whose `Drop` submits the typed
  STOP — see the pattern in
  [`docs/migration_2_0.md`](docs/migration_2_0.md#drop-never-stops-hardware).
- `detach` is explicit fire-and-forget, the spelled-out form of what drop
  already does. `#[must_use]` warns only when a returned handle is ignored
  directly.

See the maintained [blocking](examples/operation_handles.rs) and
[Tokio](examples/operation_handles_async.rs) examples for complete programs that
close the session on every path and bound movement with a stop-on-exit guard.

### Checked normalized input

`UnitInterval` is the checked `0.0..=1.0` value the zoom noun accepts directly.
`set_normalized(position, domain)` maps it across the selected `ZoomDomain`:
`Optical` spans the profile's documented optical range, and
`OpticalPlusDigital` is refused on a profile with no documented digital maximum
rather than falling back to the optical range.

```rust
use grafton_visca::blocking::CameraSession;
use grafton_visca::{
    profiles::PtzOpticsG2,
    profiles::SonyFr7,
};
use grafton_visca::{
    types::UnitInterval,
    types::ZoomDomain,
};

fn half_zoom() -> Result<(), grafton_visca::Error> {
    let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
    let camera = session.camera();

    camera
        .zoom()
        .set_normalized(UnitInterval::new(0.5)?, ZoomDomain::Optical)?
        .settled()?;

    session.close()
}

// `set_normalized` requires `HasDirectZoom`; only
// `OpticalPlusDigital` additionally requires `HasDigitalZoomRange` at runtime.
fn full_range_zoom() -> Result<(), grafton_visca::Error> {
    let session = CameraSession::<SonyFr7>::connect_udp("192.168.0.110")?;
    let camera = session.camera();

    camera
        .zoom()
        .set_normalized(UnitInterval::MAX, ZoomDomain::OpticalPlusDigital)?
        .settled()?;

    session.close()
}
```

---

## Public API Boundaries

- Use symmetric `blocking` and `r#async` facade constructors. Standard TCP,
  UDP and single-target serial constructors return `CameraSession<P>`.
  Reusable settings live in `config::CameraConfig<P>`; pass `&config` to
  `CameraSession::connect` or `open`. A multi-target `Session` takes
  `&config::SessionConfig` and selects cameras with `camera::<P>(target)`.
- Use `profiles::AnyProfile` for runtime profile selection. It uses the same
  camera, noun and operation types; `into_any()` retains the owner and
  `into_typed::<P>()` checks complete profile equality.
- Use the 14 inherent noun accessors on `Camera<P>`; profile-gated methods are
  checked by `Has*` marker bounds, the compile-time profile-permission surface,
  and runtime `ProfileSpec` validation. Direct generic `execute`/`inquire`
  intentionally have no request-specific `P` bounds: they are the uniform
  typed/downstream extension boundary. Preparation runs
  `Request::validate_for_target` before encoding, owner admission, or I/O, so
  crate-provided direct requests report `FeatureNotSupported` before writing
  when unsupported. Raw and other downstream requests remain explicit
  low-level extensions and may supply their own validation.
- Custom `Request::policy()` methods return the policy selected by their
  lifecycle: `request::CommandPolicy` for plain commands,
  `request::InquiryPolicy` for inquiries, and `request::OperationPolicy` for
  operations. Raw requests use these same policies. Commands and operations
  choose `request::CommandRetryClass`; inquiries choose
  `request::InquiryRetryClass`.
- Use `UnitInterval::new(value)?` or `UnitInterval::try_from(value)?` for the
  checked `0.0..=1.0` values taken by `camera.zoom().set_normalized(..)` and
  by profile-aware zoom normalization. Other typed control input uses checked
  `Degrees`, `SpeedLevel`, and the profile-checked `types` values.
- Bounded scalar values share fallible `new`, `value()`, `MIN`/`MAX`, and
  `TryFrom`/`From` conversions. `Degrees::new` rejects nonfinite input;
  `Kelvin` expresses physical color temperature, and `ExposureTime` expresses
  rational shutter durations. Use `wire::WireValue` only when encoding or
  decoding protocol values.
- Use `types::IndividualCameraId::new(raw)?` to check a session target, then select it
  with `CameraConfig::with_camera_id`.

---

## Installation

```toml
[dependencies]
grafton-visca = "=2.0.0"
```

### Common configurations

```toml
# Runtime-agnostic async (bring your own executor)
grafton-visca = { version = "=2.0.0", features = ["async"] }

# Async with Tokio
grafton-visca = { version = "=2.0.0", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

# With serialization
grafton-visca = { version = "=2.0.0", features = ["serde"] }

# Serial transport (blocking)
grafton-visca = { version = "=2.0.0", features = ["transport-serial"] }

# Serial transport (Tokio)
grafton-visca = { version = "=2.0.0", features = ["runtime-tokio", "transport-serial-tokio"] }
```

### Configuring Standard Transport Behavior

```rust
use grafton_visca::config::CameraConfig;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::transport::TransportConfig;
use grafton_visca::types::IndividualCameraId;
use std::time::Duration;

let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
    .with_camera_id(IndividualCameraId::CAMERA_1)
    .with_transport_config(
        TransportConfig::for_tcp().with_tcp_keepalive(Some(
            grafton_visca::transport::TcpKeepaliveConfig::new(Duration::from_secs(30)),
        )),
    );
```

TCP keepalive is a socket-level liveness mechanism. It can detect broken peers
and may keep idle network-path state active, but it does not send VISCA commands
or guarantee that camera firmware will retain an idle application session. When
an operation fails, ask `Error::requires_new_session()`: it reports `true` for
terminal connection failures such as `ConnectionClosed` and `StreamPoisoned`; it
reports `false` for a `RuntimeShutdown` this application requested and for the raw
ambiguity error `UnsequencedCommandUnconfirmed`. The canonical raw-correlation
and strict opt-in rules are in the
[architecture guide](docs/architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).
On `true`, discard that session
and establish a replacement from the retained configuration. Its state cache
starts `Unknown`, so re-query and reconcile camera state before any deliberate
resubmission. Never blindly replay an operation whose completion is uncertain.

### Feature flags

| Feature                | Enables                                    |
| ---------------------- | ------------------------------------------ |
| `blocking`             | Canonical owner-backed blocking facade (default) |
| `async`                | Canonical owner-backed async facade        |
| `runtime-tokio`        | Tokio async runtime adapter                |
| `runtime-smol`         | smol runtime adapter                       |
| `transport-serial`     | Blocking serial (RS-232/422)               |
| `transport-serial-tokio` | Async serial (Tokio)                     |
| `serde`                | Serialize/deserialize public value and configuration types |
| `schemars`             | JSON Schema generation                     |
| `ts-rs`                | TypeScript type generation                 |

---

## Camera Profiles

Profiles define protocol format, supported standard transports, and default ports:

| Profile              | Protocol           | TCP Port | UDP Port | Serial |
| -------------------- | ------------------ | -------: | -------: | :----: |
| `GenericVisca`       | Raw VISCA          |     5678 |     1259 |  yes   |
| `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` | Raw VISCA          |     5678 |     1259 |  yes   |
| `SonyEviH100`        | Raw VISCA          |      n/a |      n/a |  yes   |
| `SonyBrc300`         | Raw VISCA          |      n/a |      n/a |  yes   |
| `NearusBrc300`       | Raw VISCA          |      n/a |      n/a |  yes   |
| `SonyBrcH900`        | Sony encapsulation |      n/a |    52381 |   no   |
| `SonyFr7`            | Sony encapsulation |      n/a |    52381 |   no   |

The EVI-H100, BRC-300 and Nearus BRC-300 sources document VISCA over RS-232C
and RS-422 only, so those profiles are serial-only.

Port can be omitted for supported network transports; the profile default for
that transport is used. Unsupported profile/transport pairs are rejected by the
typed constructors at compile time and by deserialized/runtime configurations
before any socket or serial device is opened.

---

## Documentation

- **[API Reference](https://docs.rs/grafton-visca)** — Complete type and method documentation
- **[Examples](examples/)** — Maintained examples for common scenarios
- **[2.0 Example Policy](docs/examples.md)** — Example contract and maintenance rules
- **[2.0 Usage and Construction](docs/usage_2_0.md)** — Sessions, profiles, transports, and noun views
- **[2.0 Architecture](docs/architecture_2_0.md)** — Ownership, target registration, and lifecycle invariants
- **[Preservation Inventory](docs/architecture_inventory.md)** — Closed profile, transport, noun, and extension inventory
- **[Request Semantics](docs/request_semantics.md)** — Plain, inquiry, targeted, and applied-only request classes
- **[Allocation and Validation](docs/allocation_and_validation.md)** — Release gates for bounded work and preflight validation
- **[Observability and Recovery](docs/observability_and_recovery.md)** — Metrics, diagnostics, cache, and recovery behavior
- **[Optional Hardware Smoke Checks](docs/hardware_release_checklist.md)** — Private, optional device checks for maintainers
- **[2.0 Migration Guide](docs/migration_2_0.md)** — Destination APIs for pre-2.0 applications
- **[VISCA Protocol Reference](docs/visca_reference.md)** — Consolidated PTZOptics, Axis, and protocol evidence used for built-in profile decisions
- **[Camera Profile Support Guide](docs/camera_profile_support.md)** — Workflow and testing rules for adding camera capabilities
- **[CHANGELOG](CHANGELOG.md)** — Version history and migration guides

### Examples

| Example | Command |
| ------- | ------- |
| Blocking quickstart | `cargo run --example quickstart -- 192.168.0.110` |
| Blocking quickstart with movement | `cargo run --example quickstart -- 192.168.0.110 --move` |
| Async quickstart (Tokio) | `cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110` |
| Blocking operation handles | `cargo run --example operation_handles -- 192.168.0.110` |
| Async operation handles (Tokio) | `cargo run --example operation_handles_async --features runtime-tokio -- 192.168.0.110` |
| Inquiry quickstart | `cargo run --example inquiry_quickstart -- 192.168.0.110` |
| Configured transport policy | `cargo run --example transport_builder_demo -- 192.168.0.110` |
| Caller-owned transport | `cargo run --example builder_api -- 192.168.0.110:1259` |
| Preset recall | `cargo run --example preset_demo -- 192.168.0.110 recall 1` |
| Error handling | `cargo run --example error_handling --features runtime-tokio -- 192.168.0.110` |
| Serial (async) | `cargo run --example serial_async_demo --features runtime-tokio,transport-serial-tokio -- /dev/ttyUSB0 1` |

---

## Platform and toolchain

- **Rust:** 1.98 (single stable toolchain; `rust-version = "1.98"`)
- **Platforms:** Linux, macOS, Windows

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
