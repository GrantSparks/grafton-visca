# Camera Profile Support Guide

This guide is for contributors adding or changing camera profiles and optional
camera capabilities. The goal is that a selected profile means something
concrete: protocol framing, numeric limits, runtime capability metadata, and
typed control surfaces all match the checked-in source documents.

## Source Of Truth

Profile support must start from evidence in the consolidated
[VISCA protocol reference](visca_reference.md). When a profile change depends on
new vendor material or hardware validation, update that reference and its source
register in the same change instead of adding a separate protocol note.

Use this order when sources disagree:

1. Model-specific manuals, datasheets, and capability tables.
2. Manufacturer VISCA command lists for the same model and firmware family.
3. Hardware validation logs or reproducible tests with model and firmware noted.
4. Generic VISCA references and command tables.

A command-list opcode proves that an opcode exists in some command reference. It
does not, by itself, prove that a built-in profile should expose a typed API for
that feature. If model capability docs do not establish support, keep the profile
conservative and document the ambiguity.

## Metadata Versus Typed Support

Optional vendor features and profile-specific sub-capabilities use two layers:

| Layer | Meaning | Example |
| ----- | ------- | ------- |
| Metadata traits and constants | Runtime discovery facts for `Capabilities::from_profile::<P>()`; unsupported defaults are valid. | `NdFilterMetadata`, `DIGITAL_ZOOM_MAX`, `IRIS_RANGE`, `SUPPORTS_ONE_PUSH_FOCUS` |
| Support markers | Compile-time permission for typed control/accessor/inquiry APIs. | `HasNdFilter`, `HasDigitalZoomToggle`, `HasDirectZoom`, `HasIrisControl`, `HasIrisControlInquiry`, `HasOnePushFocus` |

`Profile` requires metadata traits so discovery can report the same fields for
every profile. Do not treat those metadata traits as proof of support. Implement
support markers only when the camera profile's source documents establish that
the feature exists and the crate has a typed implementation for it.

The runtime equivalent of the marker contract is `TypedSupportSet`.
`ProfileTypedSupport::TYPED_SUPPORT` records the optional typed surfaces a
profile exposes, and `Capabilities::supports_typed(...)` is the dyn-api
permission check for those surfaces. Built-in profiles derive marker impls and
`TYPED_SUPPORT` from the same `typed_support: [...]` registry list. Custom
profiles must keep optional marker impls and `TYPED_SUPPORT` in sync; do not use
metadata constants as a fallback permission source.

The two broad baseline markers are intentionally asymmetric. Implementing
`Exposure` establishes a usable baseline exposure domain, so every
`ProfileMetadata + Exposure` type receives `HasExposure`. `ImageProcessing`
must be implemented only because `Profile` needs a uniform discovery shape and
can validly contain no image surface at all; `HasImageProcessing` therefore
requires an explicit, source-backed opt-in. A downstream profile with any typed
image noun must implement that marker itself.

This split also applies inside broad baseline areas. A profile may support zoom
tele/wide movement without supporting direct absolute zoom, digital zoom toggle,
or optical-plus-digital positioning. A profile may support exposure mode and
gain without iris control. A profile may support basic focus without one-push
AF, focus zone, AF sensitivity, or focus near-limit inquiry. Model those
surfaces with precise markers rather than runtime-only guards.

Current built-in sub-capability markers include the generated table below. Its
rows come from `src/capabilities/typed_support_registry.rs`; edit that
declarative vocabulary rather than this marked region.

<!-- BEGIN GENERATED TYPED SUPPORT VOCABULARY -->
| Area | Marker | Typed surface |
| ---- | ------ | ------------- |
| Zoom | `HasDirectZoom` | Static gate for `camera.zoom().set_position`, `.set_normalized`, and `.set_normalized_in_domain` |
| Zoom | `HasDigitalZoomToggle` | `camera.zoom().set_digital_zoom` |
| Zoom | `HasDigitalZoomRange` | `TypedSupportSet` runtime gate when `.set_normalized_in_domain(..., ZoomDomain::OpticalPlusDigital)` is chosen; also requires a documented digital maximum |
| Exposure | `HasIrisControl` | iris reset/up/down/direct control and `IrisInquiryControl` (`09 04 4B` position) |
| Focus | `HasOnePushFocus` | `OnePushFocusControl` |
| Focus | `HasPtzOpticsSnapFocus` | `SnapFocusControl` |
| Focus | `HasFocusLock` | focus-lock control |
| Focus | `HasPushAutoFocus` | Sony Push AF press/release control |
| Focus | `HasFocusZone` | focus-zone selection control |
| Focus | `HasAutoFocusSensitivity` | AF sensitivity controls and inquiry |
| Focus | `HasFocusNearLimitInquiry` | focus near-limit inquiry |
| Exposure | `HasBacklightCompensation` | `BacklightCompensationControl` and backlight inquiry |
| Exposure | `HasWideDynamicRange` | `WideDynamicRangeControl` and dynamic-range inquiry |
| Exposure | `HasExposureCompensation` | exposure-compensation controls and inquiries |
| Exposure | `HasBrightnessControl` | exposure brightness controls and inquiry |
| White balance | `HasOnePushWhiteBalance` | one-push white balance mode and trigger |
| White balance | `HasAutoTrackingWhiteBalance` | ATW white balance mode |
| White balance | `HasAutoWhiteBalanceSensitivity` | AWB sensitivity control |
| Color | `HasColorTemperature` | color-temperature mode, setters, and inquiry |
| Color | `HasRgbGain` | red/blue gain controls and inquiries |
| Color | `HasRgbTuning` | red/blue tuning controls and inquiries |
| Image | `HasImageFlip` | vertical image flip control and inquiry |
| Image | `HasImageMirror` | horizontal mirror control |
| Image | `HasCombinedImageFlip` | combined flip-mode command |
| Image | `HasContrastControl` | contrast control and inquiry |
| Image | `HasSharpnessControl` | sharpness control and inquiry |
| Image | `HasSaturationControl` | saturation control and inquiry |
| Image | `HasHueControl` | hue control and inquiry |
| Image | `HasLuminanceControl` | luminance control and inquiry |
| Image | `HasGammaControl` | gamma control and inquiry |
| Image | `HasNoiseReduction2D` | 2D noise-reduction mode and level inquiries |
| Image | `HasNoiseReduction3D` | 3D noise-reduction level inquiry |
| Image | `HasPictureEffect` | picture-effect control and inquiry |
| Tally | `HasTally` | tally light controls and inquiries |
| Menu | `HasDirectMenuControl` | direct menu controls |
| ND filter | `HasNdFilter` | ND filter controls and inquiries |
| Variable speed | `HasVariableSpeed` | variable speed mode controls |
| Motion Sync | `HasMotionSync` | Motion Sync controls and inquiries |
| Focus | `HasFocusZoneInquiry` | focus-zone inquiry (independent of selection control) |
| Streaming | `HasUsbAudio` | USB audio control and inquiry |
| Exposure | `HasPtzOpticsAntiFlicker` | PTZOptics anti-flicker control and inquiry |
| System | `HasPtzOpticsSettingsSave` | PTZOptics settings-save command |
| Presets | `HasPtzOpticsPresetRecallSpeed` | PTZOptics preset-recall speed control |
| Exposure | `HasSonySpotlight` | Sony spotlight controls |
| Exposure | `HasSonyAutoSlowShutter` | Sony automatic slow-shutter controls |
| Streaming | `HasPtzOpticsMulticastStreaming` | PTZOptics multicast-streaming controls |
| Streaming | `HasPtzOpticsNdiQuality` | PTZOptics NDI-quality control |
| Exposure | `HasExposureMode` | shared `04 39` exposure-mode control and inquiry |
| Exposure | `HasIrisControlInquiry` | `IrisControlInquiryControl` (`09 04 2B` auto/manual status) |
| Image | `HasNoiseReduction2DControl` | 2D noise-reduction mode/level controls and disable |
| Image | `HasNoiseReduction3DControl` | 3D noise-reduction level control and disable |
| Image | `HasImageFreeze` | image-freeze control |
| Image | `HasDefogLevel` | vendor defog-level inquiry |
| Tally | `HasTallyBrightness` | extended tally-brightness commands |
| Tally | `HasPtzOpticsTally` | PTZOptics packed status, mode, and auto-adjust tally family |
<!-- END GENERATED TYPED SUPPORT VOCABULARY -->

## Built-In Transport Matrix

Transport support is registry data, not an inference from the profile-wide
protocol envelope. Standard constructors use `SupportsTcp`, `SupportsUdp`, and
`SupportsSerial` marker traits so unsupported profile/transport pairs do not
compile. Runtime or deserialized `TransportOptions` values are validated against
the same registry before address resolution, socket creation, serial opening, or
protocol startup.

`Session::open` is the advanced path for caller-owned transports. Built-in
TCP/UDP/serial transport handles still expose their standard transport kind and
are validated against the same registry before protocol startup. Custom
transports that do not expose a standard kind remain an intentional unchecked
escape hatch for simulators and downstream integrations.

| Profile | TCP default | UDP default | Serial | Standard envelope |
| ------- | ----------: | ----------: | ------ | ----------------- |
| `PtzOpticsG2` | 5678 | 1259 | yes | Raw VISCA |
| `PtzOpticsG3` | 5678 | 1259 | yes | Raw VISCA |
| `PtzOptics30X` | 5678 | 1259 | yes | Raw VISCA |
| `SonyFR7` | n/a | 52381 | no | Sony encapsulated UDP |
| `SonyBRCH900` | n/a | 52381 | no | Sony encapsulated UDP |
| `SonyEVIH100` | 5678 | 1259 | yes | Raw VISCA |
| `SonyBRC300` | 5678 | 1259 | yes | Raw VISCA |
| `NearusBRC300` | 5678 | 1259 | yes | Raw VISCA |
| `GenericVisca` | 5678 | 1259 | yes | Raw VISCA |

## Built-In Profile Marker Matrix

The table below lists the optional typed support markers emitted for each
built-in profile by the registry. Baseline metadata may still exist for every
profile so runtime discovery can return a complete `Capabilities` value; these
markers are the compile-time contract for typed accessors and control traits.

Do not add one of these markers to a heterogeneous typed aggregate unless every
profile behind that aggregate implements the marker. For `dyn-api` cameras, use
`camera.capabilities().supports_typed(TypedSupportSurface::...)` before calling
optional typed operations when a capability is not universal.

| Profile | Typed support markers |
| ------- | --------------------- |
| `PtzOpticsG2` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasFocusZoneInquiry`<br>`HasUsbAudio`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `PtzOpticsG3` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `PtzOptics30X` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasFocusZoneInquiry`<br>`HasUsbAudio`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `SonyFR7` | `HasSonySpotlight`<br>`HasExposureCompensation`<br>`HasPushAutoFocus`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoTrackingWhiteBalance`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasGammaControl`<br>`HasTally`<br>`HasDirectMenuControl`<br>`HasNdFilter`<br>`HasVariableSpeed` |
| `SonyBRCH900` | `HasSonySpotlight`<br>`HasExposureMode`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasGammaControl` |
| `SonyEVIH100` | `HasSonyAutoSlowShutter`<br>`HasExposureMode`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasColorTemperature`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasGammaControl` |
| `SonyBRC300` | `HasSonyAutoSlowShutter`<br>`HasExposureMode`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation` |
| `NearusBRC300` | `HasExposureMode`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasSaturationControl` |
| `GenericVisca` | `HasExposureMode`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasOnePushWhiteBalance` |

Focus-zone selection is source-backed for PtzOptics G2, G3, and 30X, while the
matching inquiry is enabled only for G2 and 30X because the G3 query response
is not established by its model-specific table. USB audio control and inquiry
likewise remain limited to the G2 and raw 30X UAC table entries. Picture effect
is retained for all three PTZOptics profiles from the G2/G3 references and the
raw 30X Gen-2 table; it is not inferred for Sony FR7 or BRC-H900. The Sony
model lists also do not establish the removed brightness controls.

The row-specific surfaces `HasImageFreeze`, `HasDefogLevel`,
`HasTallyBrightness`, and `HasPtzOpticsTally` currently have no built-in profile
grant. Image freeze remains firmware/path-specific until its VISCA command path
is validated; defog level, extended tally brightness, and PTZOptics tally
extensions likewise remain custom/evidenced-profile surfaces. Sony FR7's
source-backed red/green tally controls remain under `HasTally`.

R14 documents the current G2/G3 noise-reduction command inputs and inquiry
outputs separately. Its `04 50` input selects Auto (`02`) or Manual (`03`),
`04 53` accepts off (`00`) or levels `1–5`, and `04 54` accepts off (`00`) or
levels `1–8`; its `09 04 50`/`53`/`54` inquiry rows report the current mode and
levels `0–5`. Archived R1 is inquiry evidence only: it includes all three
inquiries and records a 3D result of `0–8`; it contains no setter rows. Because
the typed setter admits the full `0–8` public domain and both ranges are
source-backed, the decoder accepts every `NoiseReduction3DLevel` instead of
classifying levels `6–8` as malformed (#717).

Accordingly, `HasNoiseReduction2D` and `HasNoiseReduction3D` remain inquiry
markers, while `HasNoiseReduction2DControl` and
`HasNoiseReduction3DControl` independently gate the control methods. All four
markers and their matching `TypedSupportSurface` entries are emitted exactly
for `PtzOpticsG2`, `PtzOpticsG3`, and `PtzOptics30X`. `PtzOptics30X` names the
legacy PT30X SDI/NDI G2/Gen-2 profile explicitly listed by PTZOptics; it is not
a generic 30X grant and does not cover Move, Link, or newer 30X products. The
R14 portal describes its list as the full G2/G3 VISCA list, which establishes
the G3 inquiry evidence; no profile is admitted by group membership alone.
Runtime NR discovery facts and both inquiry/control typed markers agree for
every built-in profile. Profile validation requires each 2D/3D metadata bit to
equal both halves of its typed pair, so an inquiry-only or control-only runtime
profile cannot claim that NR capability. A model without source-backed shared
NR command-family evidence reports the conservative false metadata rather than
advertising a feature that typed requests must reject. Aggregate aliases remain absent (`HasNoiseReduction`,
aggregate `TypedSupportSurface::NoiseReduction` and its `noise-reduction` serde
tag, `noise_reduction_level`, `noise_reduction_mode`, and aggregate
modes/speeds/strength mappings); raw/custom requests remain the extension path
for unsupported profiles and aggregate vendor dialects.

The typed camera/session decoder and the public
`NoiseReduction3DInquiry::from_response` conversion therefore share one
profile-neutral `0–8` domain. G2, G3, and 30X all decode a well-formed level
`6`, `7`, or `8`; no mutable profile identity or whole-`ProfileSpec` comparison
changes the numeric response domain. Profile markers still decide whether the
inquiry itself is available [R1, R14, R15].

The vendor state commands use their own support markers instead of inferring
permission from broad exposure, preset, or streaming metadata. PTZOptics G2,
G3, and 30X expose anti-flicker, settings save, preset-recall speed, multicast,
and NDI quality. Sony FR7 and BRC-H900 expose the fixed `04 3A` spotlight
controls; Sony EVI-H100 and BRC-300 expose the fixed `04 5A` automatic
slow-shutter controls. Nearus BRC-300 has neither marker until an independent
model source establishes the command family. Other profiles can use the raw
escape hatch where appropriate but do not receive these typed methods.

The shared `04 39` AE-mode command and inquiry use `HasExposureMode`, separate
from broad `HasExposure`. Every built-in except Sony FR7 emits the marker and a
nonempty shared-mode inventory. PtzOptics G2/G3/30X use R1/R10/R14 evidence;
Sony BRC-H900 and BRC-300 use the exact R11/R12 command and inquiry rows;
Nearus BRC-300 follows the BRC-300 compatibility contract; Generic VISCA
deliberately assumes that Sony-standard family; and EVI-H100 retains its 1.2
compatibility breadth pending the direct R8 line-item audit required by #716.
The inventory and marker are equivalent for built-ins. Custom runtime profiles
must still provide both before a dynamic call can encode.

An iris range in runtime metadata is a discovery fact, not a typed-support
grant. `IrisLevel` covers the built-in wire-domain union (`0x00..=0x1E`) and
`GainLevel` covers the VISCA nibble domain (`0x00..=0x0F`); profile ranges make
the model-specific admission decision for either value. `HasIrisControl`
covers standard iris reset/up/down/direct control and the `09 04 4B` position
inquiry. It is enabled for the same eight built-ins:
the three PTZOptics profiles, BRC-H900, EVI-H100, BRC-300, Nearus BRC-300, and
Generic VISCA. R10/R14, R11, and R12 establish the model-specific rows; the
EVI-H100, Nearus, and generic grants follow the compatibility decisions above.
The distinct `09 04 2B` auto/manual status inquiry is instead gated by
`HasIrisControlInquiry`; none of the checked sources establishes that distinct
row, so no built-in profile implements the marker. The FR7 command list
documents a vendor-relative `7E 04 4B` Up/Down family and `05 34` Auto Iris
inquiry; it does not establish the shared `04 39` or `09 04 4B` families.
Until those FR7 families have their own models, they remain available only
through the raw-command escape hatch.

Raw/custom request APIs are different. They remain available as escape hatches
for experiments, unsupported firmware variants, and downstream integrations.
Raw request availability must not be used to justify a built-in typed support
marker. Implement `Request` for the wire encoding and semantic class; typed
custom inquiries additionally implement `Inquiry` and return a
`ResponseDecoder` for their response type. Keep routing and profile validation
explicit in those typed request implementations.

## Adding Or Updating A Profile

1. Add or update the relevant evidence in `docs/visca_reference.md`.
2. Record any model/firmware limitations near the relevant capability section.
3. Update the built-in profile registry in `src/camera/profile_registry.rs` from those sources, including transport support and default ports.
4. Use `false`, `None`, or conservative ranges when the docs do not establish support.
5. Implement optional metadata with supported values only when runtime discovery should report support.
6. Add optional typed support surfaces to the registry entry only when typed APIs are intentionally supported for that profile; the registry emits both marker impls and the runtime `TypedSupportSet`.
7. Add pass API-contract fixtures for newly supported typed surfaces.
8. Add compile-fail fixtures proving unsupported profiles cannot call those typed surfaces.
9. Add or update `Capabilities::from_profile` tests for every affected built-in profile.
10. Update README, examples docs, crate docs, and the Unreleased changelog entry.

## Common Decisions

| Situation | Profile metadata | Support marker | Notes |
| --------- | ---------------- | -------------- | ----- |
| Model docs say the feature exists and typed APIs exist | Supported value | Implement marker and include the matching `TypedSupportSurface` | Add pass and runtime discovery tests. |
| Model docs say the feature does not exist | Unsupported default | No marker | Add compile-fail coverage if the typed API could be accidentally exposed. |
| Command list has an opcode but model capability docs do not list support | Unsupported default | No marker | Document the ambiguity; raw commands remain available. |
| Hardware testing proves support not shown in a manual | Supported value if reproducible | Implement marker only if typed API is intentional | Add model, firmware, test setup, and protocol evidence to docs. |
| Feature exists but the crate lacks a typed control surface | Supported metadata may be OK | No marker yet | Open or reference follow-up work for typed support. |

## Test Expectations

Profile support changes should include focused tests before relying on the full
feature matrix:

```sh
# Root exports and the feature matrix.
cargo test --test api_stability_test --no-default-features
cargo test --test api_stability_test --no-default-features --features async

# Runtime discovery metadata and the typed support markers derived from it.
cargo test --no-default-features --lib capabilities::discovery

# The closed profile, transport, capability-gate, and noun inventories. These
# are the tests that fail when a marker or registry row changes without the
# matching inventory update.
cargo test --no-default-features --test issue_548_supported_surface_inventory
cargo test --no-default-features --test issue_542_semantic_inventory

# Profile/transport pair rejection before any socket work.
cargo test --no-default-features --features blocking --test issue_527_transport_compatibility

# Cross-surface noun parity: async, blocking, and the dynamic projection must
# agree on name, semantic class, and capability bound.
cargo test --no-default-features --features blocking --lib noun_parity

# The compile-fail contract for the typed request and marker gates.
cargo test --no-default-features --test issue_551_compile_contract

cargo clippy --all-targets --all-features -- -D warnings
```

Before merging 2.0 profile-contract work, run the declared matrix:

```sh
bash .github/scripts/test-all-features.sh
```

If a matrix failure is unrelated or flaky, note the exact failing command and
test in the PR so reviewers know what remains unresolved.

## Documentation Checklist

- README support matrix reflects the implemented typed API surface.
- `docs/visca_reference.md` explains protocol, model, firmware, and capability boundaries.
- `examples/type_safe_commands.rs` demonstrates metadata and marker bounds without using unsupported built-in profiles.
- `CHANGELOG.md` describes breaking changes and migration guidance.
- Public examples and snippets use the inherent noun accessors on `Camera<P>`,
  `camera` module re-exports, checked `UnitInterval` values, and checked camera
  ID conversion. Root control traits are 1.x vocabulary and are not 2.0 API.
- Any source conflict is documented near the relevant capability, not only in the PR discussion.
