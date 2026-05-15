# Camera Profile Support Guide

This guide is for contributors adding or changing camera profiles and optional
camera capabilities. The goal is that a selected profile means something
concrete: protocol framing, numeric limits, runtime capability metadata, and
typed control surfaces all match the checked-in source documents.

## Source Of Truth

Profile support must start from evidence in `docs/` or from a new source
document added with the change.

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
| Support markers | Compile-time permission for typed control/accessor/inquiry APIs. | `HasNdFilter`, `HasDigitalZoomToggle`, `HasDirectZoom`, `HasIrisControl`, `HasOnePushFocus` |

`Profile` requires metadata traits so discovery can report the same fields for
every profile. Do not treat those metadata traits as proof of support. Implement
support markers only when the camera profile's source documents establish that
the feature exists and the crate has a typed implementation for it.

This split also applies inside broad baseline areas. A profile may support zoom
tele/wide movement without supporting direct absolute zoom, digital zoom toggle,
or optical-plus-digital positioning. A profile may support exposure mode and
gain without iris control. A profile may support basic focus without one-push
AF, focus zone, AF sensitivity, or focus near-limit inquiry. Model those
surfaces with precise markers rather than runtime-only guards.

Current built-in sub-capability markers include:

| Area | Marker | Typed surface |
| ---- | ------ | ------------- |
| Zoom | `HasDirectZoom` | `DirectZoomControl::set_zoom` and noun/direct blocking equivalents |
| Zoom | `HasDigitalZoomToggle` | `DigitalZoomControl::set_digital_zoom` |
| Zoom | `HasDigitalZoomRange` | `DigitalZoomRangeControl::zoom_absolute_normalized` for digital domains |
| Exposure | `HasIrisControl` | `IrisControl` and `IrisInquiryControl` |
| Exposure | `HasBacklightCompensation` | `BacklightCompensationControl` and backlight inquiry |
| Exposure | `HasWideDynamicRange` | `WideDynamicRangeControl` and dynamic-range inquiry |
| Exposure | `HasExposureCompensation` | exposure-compensation controls and inquiries |
| White balance | `HasOnePushWhiteBalance` | one-push white balance mode and trigger |
| White balance | `HasAutoTrackingWhiteBalance` | ATW white balance mode |
| White balance | `HasAutoWhiteBalanceSensitivity` | AWB sensitivity control |
| Color | `HasColorTemperature` | color-temperature mode, setters, and inquiry |
| Color | `HasRgbGain` | red/blue gain controls and inquiries |
| Color | `HasRgbTuning` | red/blue tuning controls and inquiries |
| Focus | `HasOnePushFocus` | `OnePushFocusControl` |
| Focus | `HasPtzOpticsSnapFocus` | `SnapFocusControl` |
| Focus | `HasFocusZone` | focus zone controls and inquiry |
| Focus | `HasAutoFocusSensitivity` | AF sensitivity controls and inquiry |
| Focus | `HasFocusNearLimitInquiry` | focus near-limit inquiry |
| Image | `HasImageFlip` | vertical image flip control and inquiry |
| Image | `HasImageMirror` | horizontal mirror control |
| Image | `HasCombinedImageFlip` | combined flip-mode command |
| Image | `HasSaturationControl` | saturation control and inquiry |
| Image | `HasHueControl` | hue control and inquiry |
| Image | `HasLuminanceControl` | luminance control and inquiry |
| Image | `HasGammaControl` | gamma control and inquiry |
| Image | `HasNoiseReduction` | aggregate noise-reduction inquiries |
| Image | `HasNoiseReduction2D` | 2D noise-reduction control and inquiry |
| Image | `HasNoiseReduction3D` | 3D noise-reduction control and inquiry |
| Image | `HasPictureEffect` | picture-effect control and inquiry |
| Tally | `HasTally` | tally light controls and inquiries |

Raw/custom VISCA command APIs are different. They remain available as escape
hatches for experiments, unsupported firmware variants, and downstream
integrations. Raw command availability must not be used to justify a built-in
typed support marker.

## Adding Or Updating A Profile

1. Add or update the source documents in `docs/`.
2. Record any model/firmware limitations near the relevant capability section.
3. Update the built-in profile registry in `src/camera/profile_registry.rs` from those sources.
4. Use `false`, `None`, or conservative ranges when the docs do not establish support.
5. Implement optional metadata with supported values only when runtime discovery should report support.
6. Add optional typed support surfaces to the registry entry only when typed APIs are intentionally supported for that profile.
7. Add pass API-contract fixtures for newly supported typed surfaces.
8. Add compile-fail fixtures proving unsupported profiles cannot call those typed surfaces.
9. Add or update `Capabilities::from_profile` tests for every affected built-in profile.
10. Update README, examples docs, crate docs, and the Unreleased changelog entry.

## Common Decisions

| Situation | Profile metadata | Support marker | Notes |
| --------- | ---------------- | -------------- | ----- |
| Model docs say the feature exists and typed APIs exist | Supported value | Implement marker | Add pass and runtime discovery tests. |
| Model docs say the feature does not exist | Unsupported default | No marker | Add compile-fail coverage if the typed API could be accidentally exposed. |
| Command list has an opcode but model capability docs do not list support | Unsupported default | No marker | Document the ambiguity; raw commands remain available. |
| Hardware testing proves support not shown in a manual | Supported value if reproducible | Implement marker only if typed API is intentional | Add model, firmware, test setup, and protocol evidence to docs. |
| Feature exists but the crate lacks a typed control surface | Supported metadata may be OK | No marker yet | Open or reference follow-up work for typed support. |

## Test Expectations

Profile support changes should include focused tests before relying on the full
feature matrix:

```sh
cargo test --test api_stability_test --no-default-features
cargo test --test api_stability_test --no-default-features --features mode-async
cargo test --no-default-features --lib capabilities::discovery
cargo test --no-default-features --test feature_detection_tests --test camera_profile_tests --test simple_compile_test
cargo test --no-default-features --features test-utils --test compile_time_safety_test
cargo clippy --all-targets --all-features -- -D warnings
```

Before merging 1.0 contract work, run the declared matrix:

```sh
bash .github/scripts/test-all-features.sh
```

If a matrix failure is unrelated or flaky, note the exact failing command and
test in the PR so reviewers know what remains unresolved.

## Documentation Checklist

- README support matrix reflects the implemented typed API surface.
- `docs/visca_unified_reference.md` and model-specific docs explain capability boundaries.
- `examples/type_safe_commands.rs` demonstrates metadata and marker bounds without using unsupported built-in profiles.
- `CHANGELOG.md` describes breaking changes and migration guidance.
- Any source conflict is documented near the relevant capability, not only in the PR discussion.
