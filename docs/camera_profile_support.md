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
| Zoom | `HasDirectZoom` | Static gate for `camera.zoom().set_position` and `.set_normalized` |
| Zoom | `HasDigitalZoomToggle` | `camera.zoom().set_digital_zoom` |
| Zoom | `HasDigitalZoomRange` | `TypedSupportSet` runtime gate when `.set_normalized(..., ZoomDomain::OpticalPlusDigital)` is chosen; also requires a documented digital maximum |
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
| Color | `HasColorTemperature` | color-temperature mode and setters |
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
| Image | `HasNoiseReduction2D` | 2D noise-reduction level inquiry |
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
| Image | `HasNoiseReduction2DControl` | 2D noise-reduction level control and disable |
| Image | `HasNoiseReduction3DControl` | 3D noise-reduction level control and disable |
| Image | `HasImageFreeze` | image-freeze control |
| Image | `HasDefogLevel` | vendor defog-level inquiry |
| Tally | `HasTallyBrightness` | extended tally-brightness commands |
| Tally | `HasPtzOpticsTally` | PTZOptics packed status, mode, and auto-adjust tally family |
| System | `HasVersionInquiry` | `camera.system().version()` (`09 00 02` Sony-format `CAM_VersionInq` decode) |
| Color | `HasColorTemperatureInquiry` | color-temperature inquiry |
| Image | `HasNoiseReduction2DMode` | 2D noise-reduction auto/manual mode control and inquiry |
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
| `SonyEVIH100` | n/a | n/a | yes | Raw VISCA |
| `SonyBRC300` | n/a | n/a | yes | Raw VISCA |
| `NearusBRC300` | n/a | n/a | yes | Raw VISCA |
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
| `PtzOpticsG2` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasFocusZoneInquiry`<br>`HasUsbAudio`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasColorTemperatureInquiry`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction2DMode`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `PtzOpticsG3` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction2DMode`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `PtzOptics30X` | `HasPtzOpticsAntiFlicker`<br>`HasPtzOpticsSettingsSave`<br>`HasPtzOpticsPresetRecallSpeed`<br>`HasPtzOpticsMulticastStreaming`<br>`HasPtzOpticsNdiQuality`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasFocusLock`<br>`HasDirectZoom`<br>`HasIrisControl`<br>`HasFocusZone`<br>`HasFocusZoneInquiry`<br>`HasUsbAudio`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasColorTemperatureInquiry`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoWhiteBalanceSensitivity`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasCombinedImageFlip`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasLuminanceControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction2DMode`<br>`HasNoiseReduction3D`<br>`HasNoiseReduction2DControl`<br>`HasNoiseReduction3DControl`<br>`HasPictureEffect` |
| `SonyFR7` | `HasSonySpotlight`<br>`HasExposureCompensation`<br>`HasPushAutoFocus`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasRgbGain`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasAutoTrackingWhiteBalance`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasGammaControl`<br>`HasTally`<br>`HasDirectMenuControl`<br>`HasNdFilter`<br>`HasVariableSpeed`<br>`HasVersionInquiry` |
| `SonyBRCH900` | `HasSonySpotlight`<br>`HasExposureMode`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasWideDynamicRange`<br>`HasColorTemperature`<br>`HasRgbTuning`<br>`HasOnePushWhiteBalance`<br>`HasImageFlip`<br>`HasImageMirror`<br>`HasContrastControl`<br>`HasSharpnessControl`<br>`HasSaturationControl`<br>`HasGammaControl`<br>`HasVersionInquiry` |
| `SonyEVIH100` | `HasSonyAutoSlowShutter`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasOnePushFocus`<br>`HasIrisControl`<br>`HasFocusNearLimitInquiry`<br>`HasBacklightCompensation`<br>`HasRgbGain`<br>`HasOnePushWhiteBalance`<br>`HasSaturationControl`<br>`HasHueControl`<br>`HasGammaControl`<br>`HasNoiseReduction2D`<br>`HasNoiseReduction2DControl`<br>`HasVersionInquiry` |
| `SonyBRC300` | `HasSonyAutoSlowShutter`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasOnePushFocus`<br>`HasIrisControl`<br>`HasBacklightCompensation`<br>`HasRgbGain`<br>`HasOnePushWhiteBalance`<br>`HasImageFlip`<br>`HasVersionInquiry` |
| `NearusBRC300` | `HasSonyAutoSlowShutter`<br>`HasExposureMode`<br>`HasExposureCompensation`<br>`HasBrightnessControl`<br>`HasDirectZoom`<br>`HasDigitalZoomToggle`<br>`HasDigitalZoomRange`<br>`HasOnePushFocus`<br>`HasIrisControl`<br>`HasBacklightCompensation`<br>`HasRgbGain`<br>`HasOnePushWhiteBalance`<br>`HasImageFlip`<br>`HasVersionInquiry` |
| `GenericVisca` | `HasExposureMode`<br>`HasIrisControl`<br>`HasOnePushFocus`<br>`HasOnePushWhiteBalance`<br>`HasVersionInquiry` |

Focus-zone selection is source-backed for PtzOptics G2, G3, and 30X, while the
matching inquiry is enabled only for G2 and 30X: the `09 04 AA` inquiry comes
from the archived Gen-2 table, and R14's Queries page (checked 2026-10-05) has
no `09 04 AA` row, so it is not G3 evidence. USB audio control and inquiry
likewise remain limited to the G2 and raw 30X UAC table entries; R14 lists no
UAC row. Picture effect is retained for all three PTZOptics profiles from the G2/G3 references and the
raw 30X Gen-2 table; it is not inferred for Sony FR7 or BRC-H900. The Sony
model lists also do not establish the removed brightness controls.

### Focus zone value `03` and per-profile zone lists (#795)

The PTZOptics sources (the R14/R20 `CAM_AFZone` rows and the G2 user-manual
OSD and web `AF-Zone` options) and the OEM command lists checked for the same
firmware family document only Top (`00`), Center (`01`) and Bottom (`02`). On
the PTZOptics G2 bench (PT30X-NDI, PT20X-NDI and PT12X-NDI G2; firmware ARM
6.3.51THI, 6.3.76THI and 6.4.18SHI; 2026-10-04) every camera answered the
`81 09 04 AA FF` inquiry with `90 50 03 FF`, accepted `81 01 04 AA 03 FF` with
ACK `90 42` and completion `90 52`, and read `03` back; `01` round-trips the
same way. The value therefore behaves as a peer of the documented zones.
`FocusZone::Zone03` represents it with a value-based name because no source
says which image area it weights; it is not focus lock, which is the separate
`81 0A 04 68 02/03 FF` command. `FocusZone` is `#[non_exhaustive]` because its
values are evidence-driven vendor values rather than one fixed protocol table.

Sending a zone is gated per value, following the `exposure_modes` precedent.
`Focus::FOCUS_ZONES` lists the values a profile may send (default: the
documented three) and `Capabilities::focus_zones` mirrors it at runtime; it is
empty exactly when focus-zone selection is unsupported and must be
duplicate-free. `FocusZoneCommand` refuses a value outside the list with
`Error::InvalidParameter { parameter: "focus_zone", .. }` before any I/O, on
the static, dynamic and runtime-profile paths alike.

The table below is checked against the registry by
`camera_profile_support_focus_zone_table_matches_registry`. `Zone03` is
admitted for `PtzOpticsG2` and `PtzOptics30X` from bench evidence; G3 was not
bench-tested.

<!-- BEGIN GENERATED FOCUS ZONES -->
| Profile | `focus_zones` |
| ------- | ------------- |
| `PtzOpticsG2` | `Top`, `Center`, `Bottom`, `Zone03` |
| `PtzOpticsG3` | `Top`, `Center`, `Bottom` |
| `PtzOptics30X` | `Top`, `Center`, `Bottom`, `Zone03` |
| `SonyFR7` | none |
| `SonyBRCH900` | none |
| `SonyEVIH100` | none |
| `SonyBRC300` | none |
| `NearusBRC300` | none |
| `GenericVisca` | none |
<!-- END GENERATED FOCUS ZONES -->

Decoding is not gated: the focus-zone inquiry decodes `03` from any camera,
because reading a reply sends nothing. A runtime profile may add `Zone03` to
its own list when its camera has evidence for it. A persisted `ProfileSpec`
must carry the field; one saved without it is refused on load with an error
that names the regeneration call. See
[VISCA reference §7.13](visca_reference.md) for the source search.

### Color-temperature inquiry (`HasColorTemperatureInquiry`)

`HasColorTemperature` gates the color-temperature mode and the reset, up,
down and direct setters. The `09 04 20` inquiry is gated separately, because
its reply layout is sourced only for the PTZOptics G2 family: the PTZOptics G2
bench (hardware inquiry test, #498) replies with one data byte, `90 50 pq FF`,
where `pq` is the `04 20` direct code ([VISCA reference §7.10](visca_reference.md)).
`PtzOpticsG2` and the legacy G2 `PtzOptics30X` implement
`HasColorTemperatureInquiry`. `PtzOpticsG3` and `SonyBRCH900` keep the
controls, but no source documents their reply, so
`camera.white_balance().color_temperature()` does not compile for them and the
dyn/runtime path refuses it with `FeatureNotSupported` before any I/O; send
`raw::Inquiry` with `81 09 04 20 FF` to read the bytes.

### Version inquiry (`HasVersionInquiry`)

`camera.system().version()` decodes the Sony `CAM_VersionInq` reply,
`y0 50 GG GG HH HH JJ JJ KK FF` (vendor, model, ROM revision, maximum socket).
Each Sony profile's registry evidence cites its model's row: R7 (FR7, model
`051E`), R11 (BRC-H900, `050B`), R8 (EVI-H100, `050E`/`050F`), R12 (BRC-300,
`040F`) and R21 (Nearus BRC-300); `GenericVisca` follows the Sony baseline.
These profiles implement `HasVersionInquiry`. The PTZOptics profiles do not:
on the PTZOptics G2 bench (2026-10-04) the cameras answered `81 09 00 02 FF`
with the 2-byte payload `90 50 00 52 FF`, and no PTZOptics source documents
that reply or its fields. G3 was not bench-tested and is gated under the same
source-evidence rule. Rather than guess at field meanings, the typed inquiry
does not compile for those profiles and the dyn/runtime path refuses it with
`FeatureNotSupported` before any I/O; send `raw::Inquiry` with
`81 09 00 02 FF` to read the bytes.

A custom profile keeps the typed version inquiry only if all three of these
hold:

1. its compile-time profile type implements `HasVersionInquiry`;
2. its `ProfileTypedSupport::TYPED_SUPPORT` includes
   `TypedSupportSurface::VersionInquiry`;
3. every persisted runtime profile lists `"version-inquiry"` in
   `capabilities.typed_support`. A custom runtime profile without the tag
   loads, but `version()` then fails with `FeatureNotSupported`; add the tag
   to the stored JSON (or to the `TypedSupportSet` before building the
   `ProfileSpec`).

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

Accordingly, `HasNoiseReduction2D` and `HasNoiseReduction3D` gate the level
inquiries, `HasNoiseReduction2DControl` and `HasNoiseReduction3DControl` the
level controls, and `HasNoiseReduction2DMode` the `04 50` auto/manual mode
control and inquiry. All five markers are emitted for `PtzOpticsG2`,
`PtzOpticsG3`, and `PtzOptics30X`. Sony EVI-H100 carries only
`HasNoiseReduction2D` and `HasNoiseReduction2DControl`: R8 documents
`CAM_NR` `8x 01 04 53 0p FF` (0 off, levels 1–5) and `CAM_NRInq`
`8x 09 04 53 FF` → `y0 50 0p FF`, which are exactly the 2D level control and
inquiry, and no `04 50` mode or `04 54` 3D level. `PtzOptics30X` names the
legacy PT30X SDI/NDI G2/Gen-2 profile explicitly listed by PTZOptics; it is not
a generic 30X grant and does not cover Move, Link, or newer 30X products. The
R14 portal describes its list as the full G2/G3 VISCA list, so a row R14 itself
lists (here the NR controls and inquiries) is G3 evidence; a row found only in
the archived Gen-2 tables (R1/U1) is not. No profile is admitted by group
membership alone.
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
controls; Sony EVI-H100 (R8), BRC-300 (R12) and Nearus BRC-300 (R21) expose the
fixed `04 5A` automatic slow-shutter controls. None of those three model
sources lists the `04 3A` spotlight family. Other profiles can use the raw
escape hatch where appropriate but do not receive these typed methods.

The shared `04 39` AE-mode command and inquiry use `HasExposureMode`, separate
from broad `HasExposure`. Every built-in except Sony FR7 emits the marker and a
nonempty shared-mode inventory. PtzOptics G2/G3/30X use R1/R10/R14 evidence;
Sony BRC-H900, EVI-H100, BRC-300 and Nearus BRC-300 use the exact command and
inquiry rows of their own model sources (R11, R8, R12 and R21). Generic VISCA
grants only the Sony-standard families that R8, R12 and R21 all document
identically: the `04 39` AE modes, standard iris, and the One Push white
balance mode and trigger.
The inventory and marker are equivalent for built-ins. Custom runtime profiles
must still provide both before a dynamic call can encode.

Sony EVI-H100 (R8), BRC-300 (R12) and Nearus BRC-300 (R21) document the standard
exposure-compensation (`04 3E`/`04 0E`/`04 4E`), Bright (`04 0D`/`04 4D`) and
R/B gain (`04 03`/`04 04`, `04 43`/`04 44`) commands with their inquiries, so
all three carry `HasExposureCompensation`, `HasBrightnessControl` and
`HasRgbGain`; BRC-300 and Nearus also document `CAM_ImgFlip` `04 66` with its
inquiry (`HasImageFlip`, no mirror). `BrightnessLevel` is the `0p 0q` wire byte
of `CAM_Bright` Direct, so each profile's `exposure_brightness_range` is the sole
admission authority: `0x00..=0x1F` without `0x01..=0x04` for EVI-H100 (R8), `0x00..=0x17` for BRC-300
and Nearus BRC-300 (R12, R21), and `0x00..=0x11` for the PTZOptics profiles.
EVI-H100's Wide-D is `CAM_WD` `04 3D`; the typed `WideDynamicRange` surface sends
the different `04 25` command, so EVI-H100 reports the WDR metadata but withholds
the typed surface.

Shutter codes are per-model tables, and a code means a different exposure time
on each table, so each profile carries its own source's table: EVI-H100 `00`
(1/1 s) through `15` (1/10000 s) from R8's 60/30 mode column, and BRC-300 and
Nearus BRC-300 `02` (1/4 s) through `15` from R12/R21. Generic VISCA uses the
codes all three share (`02` through `15`). The FR7 and BRC-H900 command lists
could not be read, so those profiles advertise no shutter codes and a typed
shutter position is refused for them; the raw command path remains.

### Known unverified facts

- **FR7 shutter codes (R7).** The shutter table of the FR7 command list could
  not be read, so `SonyFR7` advertises no shutter codes (`shutter_speeds` is
  empty) and `ShutterSpeed` positions are refused before any I/O.
- **BRC-H900 shutter codes (R11).** The shutter table of the BRC-H900 command
  list could not be read, so `SonyBRCH900` advertises no shutter codes either.
- **Pan/tilt direction of PTZOptics, FR7 and BRC-H900.** Degrees are positive
  right and positive up for every profile. R8 (EVI-H100) documents increasing
  raw pan as right and increasing raw tilt as up, and R12/R21 (BRC-300, Nearus)
  document increasing raw pan as left and increasing raw tilt as up. The
  PTZOptics, FR7 and BRC-H900 sources give degree ranges but no raw direction,
  so those profiles follow the R8 convention. Only PTZOptics G2 tilt has been
  verified on hardware (below); pan direction, `PtzOpticsG3`, FR7 and BRC-H900
  remain unverified. `tests/pan_tilt_polarity.rs` pins the encoded direction of
  +45° for every built-in profile.

The FR7 and BRC-H900 shutter entries are gaps in the evidence, not statements
that the cameras lack shutter control. Send the shutter command as a
`raw::Plain` request until a source-backed table is recorded in
[VISCA reference](visca_reference.md) and added to the profile.

### Facts verified on the PTZOptics G2 bench

These were verified on 2026-10-07 on PT30X-NDI G2, PT20X-NDI G2 and
PT12X-NDI G2 cameras; models, firmware and run evidence are in the
[hardware release checklist](hardware_release_checklist.md) (row HW-05).

- **PTZOptics G2-family shutter codes.** The PTZOptics sources document only
  `pq = Shutter Position`, not a code-to-time table, so `PtzOpticsG2`,
  `PtzOpticsG3` and `PtzOptics30X` ship the table `01` (1/30 s) through `11`
  (1/10000 s). `hw05_shutter_round_trip` (`tests/hardware_wire_rows_test.rs`)
  set codes `01`, `06`, `09` and `11` in shutter-priority mode and read each
  back unchanged on all five cameras, with `PtzOptics30X` on the PT30X-NDI G2
  and `PtzOpticsG2` on the others. The bench did not measure exposure time, so
  the fraction attached to each code is still the shipped table's.
- **PTZOptics G2 tilt polarity.** `hw05_tilt_polarity` passed on all five
  cameras: a typed tilt UP increased raw tilt and tilt degrees, DOWN decreased
  them, and pan did not change. A separate visible ~9° UP move was confirmed
  physically up on every camera by the operator.

Sony EVI-H100 (R8) also documents `CAM_ColorGain` `8x 01 04 49 00 00 00 0p FF`
and `CAM_ColorHue` `8x 01 04 4F 00 00 00 0p FF` (`0h`..`Eh`) with their
`09 04 49`/`09 04 4F` inquiries, byte for byte the typed saturation and hue
controls, so it carries `HasSaturationControl` and `HasHueControl`. R8's
`CAM_Aperture` (`04 02`/`04 42`, level `00`..`0F`) and `CAM_PictureEffect`
(`04 63`) are reported as discovery metadata but their typed surfaces are
withheld: the sharpness surface also sends the `04 05` mode R8 does not list,
and R8's picture-effect inquiry reports Off as `00` and Neg.Art as `02`, which
the typed inquiry would decode as Off.

Sony BRC-300 (R12) and Nearus BRC-300 (R21) document an x12 lens with optical
zoom positions `0000`..`4000` and digital positions `4000`..`7F00` (x4) reached
through `CAM_Zoom` Direct `04 47`, `CAM_DZoom` on/off `04 06 02/03` with
`CAM_DZoomModeInq`, and the One Push AF trigger `04 18 01`, so both carry
`HasDigitalZoomToggle`, `HasDigitalZoomRange` and `HasOnePushFocus`. Generic
VISCA gains `HasOnePushFocus`, the one of these that R8, R12 and R21 document
identically; their digital zoom tables differ.

The EVI-H100, BRC-300 and Nearus BRC-300 sources document VISCA over RS-232C and
RS-422 only, so those profiles support no IP transport. Nearus BRC-300 uses the
BRC-300 one-speed, five-nibble pan/tilt frame and signed limits that R21
documents.

`HasBacklightCompensation` is likewise a paired typed control and inquiry under
`camera.image()`, not a consequence of broad exposure metadata. For Sony
BRC-300, R12 explicitly lists `CAM_BackLight` `8x 01 04 33 02/03 FF` on manual
p. 11 and `CAM_BackLightModeInq` `8x 09 04 33 FF` with `y0 50 02/03 FF` replies
on manual p. 14. Runtime profiles must therefore provide image base support,
the backlight fact, and the typed marker together.

An iris range in runtime metadata is a discovery fact, not a typed-support
grant. `IrisLevel` covers the built-in wire-domain union (`0x00..=0x1E`) and
`GainLevel` covers the VISCA nibble domain (`0x00..=0x0F`); profile ranges make
the model-specific admission decision for either value. `HasIrisControl`
covers standard iris reset/up/down/direct control and the `09 04 4B` position
inquiry. It is enabled for the same eight built-ins:
the three PTZOptics profiles, BRC-H900, EVI-H100, BRC-300, Nearus BRC-300, and
Generic VISCA. R10/R14, R11, R8, R12 and R21 establish the model-specific rows.
Iris and bright positions are `CapabilityDomain` values: a table's bounds minus
the positions it does not list, checked by one admission rule in both the
`*Ext` helpers and request preparation (a gap is `Error::InvalidParameter`, a
value past the bounds `Error::ParameterOutOfRange`). EVI-H100's iris table (R8
p. 44) lists `00` CLOSE and `05`..`11` and its Bright table (p. 45) `00` and
`05`..`1F`, so both domains exclude `01`..`04`. BRC-300 and Nearus BRC-300 list
every iris position `00`..`11` (R12, R21). Generic VISCA admits the iris
positions all three list: `00` and `05`..`11`. The PTZOptics sources (R1, R14)
give no iris or bright tables, so those profiles keep their contiguous ranges.
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

# Blocking/async facade parity: `Session`, `CameraSession`, `Camera` and
# `Operation` expose the same methods in both modes. The noun accessors and
# the dynamic projection need no parity test: one noun-table row generates all
# three surfaces and their capability gates.
cargo test --no-default-features --features blocking,async --lib facade_parity

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
