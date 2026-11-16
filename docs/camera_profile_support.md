# Camera Profile Support Guide

A profile lets a camera use the existing blocking and async noun API, including
mixed rigs built from `Camera<AnyProfile>`. Start with a JSON `ProfileSpec`, try
it through that API, collect model evidence, and promote the same facts into the
built-in registry.

## Contribution invariant

> A camera contribution adds a profile: one registry row with source-backed
> grants, its evidence entries, and, only when the camera speaks a different
> dialect for an existing concept, an encoding variant with golden bytes. It
> adds no public method, value type, request type, marker or typed surface. A
> genuinely new concept may add those, under a vendor-neutral name and with a
> reviewed contract, in a change separate from the profile that first uses it.

The [pull request checklist](../.github/pull_request_template.md) records this
boundary. An additional model joins the same API; an encoding variant changes
how an existing concept reaches its wire representation.

## Source of truth

Profile support starts from the consolidated
[VISCA protocol reference](visca_reference.md). Add new vendor material or
reproducible hardware validation to that reference and its source register in
the same change. Use this order when sources disagree:

1. Model-specific manuals, datasheets and capability tables.
2. Manufacturer VISCA command lists for the same model and firmware family.
3. Hardware validation logs or reproducible tests with model and firmware noted.
4. Generic VISCA references and command tables.

An opcode in a generic command list does not establish a model-specific grant.
Record the ambiguity and retain conservative facts when a model's support is
unknown. A successful inquiry decode establishes a result for that query and
setup; it does not establish a writable control family, physical motion,
settlement, cancellation or timing guarantees.

## Facts, grants and encodings

`ProfileSpec` is the validated representation for built-in and custom profiles.
A built-in `ProfileId::spec()` resolves the registry row. A custom spec uses the
same builder and deserialization validation. These are its top-level fact keys:

| JSON key | Meaning |
| --- | --- |
| `capabilities` | Identity, optional domain records, ranges, mode inventories and `typed_support` grants |
| `encodings` | Selected variants for granted surfaces with multiple encodings |
| `transports` | `tcp_port`, `udp_port` and `serial` compatibility |
| `envelope` | Raw VISCA or Sony encapsulation |
| `timing` | ACK, inquiry, cancellation, ambiguity, busy, spacing and command-category deadlines |
| `maximum_command_sockets` | Source-backed command capacity |
| `supports_operation_complete` | Whether protocol completion can establish settlement |
| `supports_command_cancel` | Whether the model supports socket cancellation |
| `position_inquiries` | Optional affected-axis set available for settlement queries |

Every current fact key is required, including optional nested keys. Explicit
`null` means an absent fact; omission is not a request for a default. Unknown
keys are rejected. Export again with the current release when an older file
needs regeneration. Durations serialize as `{"secs": ..., "nanos": ...}`.

Domain records describe presence and supported values. For example,
`capabilities.zoom` owns zoom ranges, and `capabilities.focus.zones` lists the
focus zones that may be sent. An absent domain is `null`; empty mode tables and
conservative ranges preserve gaps in the evidence. The corresponding static
domain markers are `HasPower`, `HasZoom`, `HasFocus`, `HasPanTilt`, `HasPresets`,
`HasExposure`, `HasWhiteBalance`, `HasImage` and `HasMenu`.

Optional grants name existing concepts such as `Iris`, `NdFilter` or
`OnePushWhiteBalance`. A registry grant supplies its checked `source!` reference
and generates both the `Has*` marker and the runtime support entry. A domain's
presence does not grant every optional method within it. Getter and setter
support may differ: `ColorTemperatureInquiry` is separate from
`ColorTemperature`, while `NoiseReduction2D` covers its evidenced command and
inquiry family together.

Runtime discovery uses `camera.capabilities().supports(surface)` and the optional
domain records. `Camera<AnyProfile>` exposes all noun methods and checks their
domain, permission, value inventory and selected encoding before I/O. It makes
no static `Has*` claims. Built-in compile-time profiles expose the same methods
with the corresponding marker bounds. Request preparation uses the same facts
for both forms.

### Same concept, different bytes

A surface with one documented encoding declares no family. A multi-encoding
surface declares a closed, `#[non_exhaustive]` `<Surface>Encoding` enum in
`src/capabilities/typed_support_registry.rs`; its registry grant selects a
variant. The current families are:

| Surface | Encoding variants | JSON selection |
| --- | --- | --- |
| `PresetRecallSpeed` | `PtzOptics`, `SonyFr7` | `"preset_recall_speed": "ptzoptics"` or `"sony_fr7"` |
| `OnePushWhiteBalance` | `Standard`, `SonyFr7` | `"one_push_white_balance": "standard"` or `"sony_fr7"` |

For custom builders, `ProfileSpecBuilder::encoding(EncodingSelection::...)`
sets a selection. A granted multi-encoding surface without its selection is
invalid, as is a selection without the corresponding grant. Registry rows keep
one source: `capabilities.typed_support: grants(...)` owns the grants and variant
choices; `encodings: from_grants` derives the map.

An existing concept with another evidenced packet dialect needs a variant,
its `for_target(&TargetRegistration, ...)` encoder arm and independent literal
golden rows. Keep the existing method, request, value and marker. A different
semantic contract requires separate review before a profile can use it.

## Worked example: `acme_cam.json` to a registry row

This walkthrough prepares an unlisted **Acme PTZ** as a local draft. It does not
add a built-in profile or claim that Acme hardware supports any template fact.
The export and validation steps need no camera.

### 1. Export a complete template

From the repository root:

```sh
cargo +1.98.0 run --example profile_spec --no-default-features --features serde -- \
  export generic_visca > acme_cam.json
```

`export` accepts any built-in's canonical id; `--help` lists the available ids.
The file initially retains that built-in's identity and must match its complete
registry spec. Make the identity explicitly custom before editing facts:

```sh
python3 - <<'PY'
import json
from pathlib import Path

path = Path("acme_cam.json")
spec = json.loads(path.read_text())
spec["capabilities"]["profile_id"] = None
spec["capabilities"]["model_name"] = "Acme PTZ"
path.write_text(json.dumps(spec, indent=2) + "\n")
PY
```

Now compare every copied fact with Acme's model and firmware documentation.
Remove unsupported grants and matching encoding selections together; use
absent domain records or narrower inventories where appropriate. Keep every
required key. Template inheritance is not model evidence, including for ports,
coordinate polarity, shutter tables, socket limits, cancellation and timing.
Keep this working JSON under `target/` or outside the repository when preparing
an actual contribution.

### 2. Validate and run through the existing API

```sh
cargo +1.98.0 run --example profile_spec --no-default-features --features serde -- \
  check acme_cam.json
```

`check` deserializes the complete spec and prints its identity, actual granted
surfaces, encoding choices and allowed transports. It performs no camera I/O.
An error must be resolved in the facts before trying a connection.

After reviewing the draft against the camera's evidence, a blocking application
with `blocking,serde` enabled can load it directly:

```rust,no_run
use grafton_visca::{
    blocking::CameraSession,
    config::{CameraConfig, TransportOptions},
    profiles::ProfileSpec,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec: ProfileSpec = serde_json::from_reader(std::fs::File::open("acme_cam.json")?)?;
    let config = CameraConfig::new(spec, TransportOptions::tcp("10.0.0.30"));
    let session = CameraSession::connect(&config)?;
    let camera = session.camera(); // Camera<AnyProfile>
    println!("Power: {:?}", camera.power().status()?);
    session.close()?;
    Ok(())
}
```

The async route uses `r#async::CameraSession::connect(&config, runtime).await`.
The same concrete nouns, values and operation handles serve the custom spec.
An ungranted method fails before a frame is sent. Applications do not need a
separate dynamic API or a new profile-specific control trait.

### 3. Collect read-only evidence

The ignored unit test below probes the built-in inquiry inventory using raw
requests and the built-in decoders, independently of typed grants:

```sh
VISCA_CAMERA_IP=10.0.0.30 VISCA_PROFILE_SPEC=acme_cam.json \
  cargo +1.98.0 test --features serde --lib profile_probe::profile_probe -- \
  --ignored --exact --nocapture
```

For an existing built-in, replace `VISCA_PROFILE_SPEC` with one explicit
`VISCA_PROFILE`, for example `VISCA_PROFILE=generic_visca`. The id is its
canonical `snake_case` string. Set `VISCA_TRANSPORT=tcp` or `udp` to choose a
supported network transport; otherwise the probe chooses the first one allowed
by the spec. `VISCA_CAMERA_IP` accepts a host and the profile's default port, or
an explicit host/port address.

For serial hardware:

```sh
VISCA_SERIAL_PORT=/dev/ttyUSB0 VISCA_PROFILE=sony_evi_h100 \
  cargo +1.98.0 test --features serde,transport-serial --lib profile_probe::profile_probe -- \
  --ignored --exact --nocapture
```

`VISCA_BAUD_RATE` is optional and defaults to 9600. Choose exactly one endpoint,
`VISCA_CAMERA_IP` or `VISCA_SERIAL_PORT`, and exactly one profile selector. Serial
startup stays silent: the probe sends no Address Set or interface-clear command.
The hardware test is refused under `CI`.

Reports are written to `target/profile-probe/<label>.json`; use
`VISCA_PROBE_LABEL` for a safe report filename. They include the full profile,
version result, query/surface identity, request bytes, reply payload, decoder
result or error and latency. Request hex is the complete VISCA frame without a
transport envelope. Reply hex is the VISCA data payload without its response
header, terminator or transport envelope.

Printed candidates are query/decoder observations for review, not installable
grants. Shared bytes may have several interpretations. Decoder acceptance alone
cannot identify the intended semantics or justify a writable control. Attach
the report, exact model, firmware, transport and reproducible setup to the
contribution; record the supported conclusion and its limits in
`docs/visca_reference.md`. Reports and raw logs remain issue or CI artifacts,
while authored evidence entries belong in source.

### 4. Promote the reviewed facts

Once the facts and grants have evidence, add one row to
[`src/camera/profile_registry.rs`](../src/camera/profile_registry.rs). Start from
its complete `GenericVisca` row as a structural template, then transfer the
reviewed Acme values from `acme_cam.json` into its `facts` tree. The fact keys
and nesting are the exported JSON schema, not a second naming convention:

| Reviewed JSON fact | Registry row |
| --- | --- |
| `capabilities.model_name` | Same `model_name` fact in `facts.capabilities` |
| `capabilities.pan_tilt.coordinates` | Same nested coordinate record |
| `capabilities.focus.zones` | Same explicit zone inventory |
| `transports.tcp_port`, `udp_port`, `serial` | Same three facts in `facts.transports` |
| A duration's `secs` and `nanos` | The same duration as a typed Rust `Duration` expression |
| `capabilities.typed_support` | `grants(...)`, with one checked source reference per grant |
| `encodings` | Variant choices on those grants, derived with `from_grants` |
| Custom `capabilities.profile_id: null` | `profile_id: from_id`, derived from the new row id |

Use an `AcmePtz` type and `acme_ptz` id only when actually adding the reviewed
profile. Keep its public documentation, id documentation, vendor, description,
withheld rationale and evidence metadata outside `facts`. The row owns the id
once; `from_id` projects its new built-in identity. Retain every fact key,
including `None` and empty records or inventories. Typed Rust expressions are
the values; key names and record structure stay aligned with JSON.

Each grant's `source!` names the source register id, exact model set and checked
evidence anchor in `docs/visca_reference.md`. A borrowed generic or another
model's source reference is not Acme evidence. Evidence rows name the final
`grant.<Surface>` subject and its exact command/inquiry packet set. New facts
may need command or inquiry evidence as well as a grant row. Keep existing
anchors stable and let the source completeness tests catch omissions or
orphans. An encoding variant additionally needs literal byte cases for every
new encoder branch.

The registry generates the static profile, markers, runtime grants, default
ports and validated spec from that row. Its
`authored_row_fact_keys_match_every_exported_profile` test compares each
row's authored fact keys recursively with its actual serialized spec;
`row_key_alignment_detects_new_missing_and_renamed_record_facts` checks the
tripwire itself. Update the row shape when the JSON schema intentionally changes
instead of adding a separate conversion table or generated row emitter.

After promotion, a rig's `ProfileSelection` value can change from the custom spec
object to the new `"acme_ptz"` string. `Camera<AnyProfile>` callers keep the same
code and noun contracts. Promotion does not change the meaning of an existing
model's grants or claim hardware validation beyond the attached evidence.

## Generated support documentation

The support tables and closed lists are generated from the registry, typed
support vocabulary and noun ledger. Contributors edit facts, grants, evidence
and tests, then regenerate the marked blocks; they never maintain table rows by
hand. Authored source explanations and hardware limits remain authored.

`grafton_visca_test_support::bless_or_assert(path, block, generated)` checks the
exact text between `<!-- BEGIN <block> -->` and `<!-- END <block> -->`. With
`GRAFTON_VISCA_BLESS=1` it writes the block and deliberately fails so the diff
must be reviewed. Blessing is refused under `CI`. Review the changed profile
availability, method permissions, source-key index and literal wire bytes, then
rerun the same tests with the variable unset. The wire golden uses this same
variable.

Regenerate all support-doc blocks from the repository root:

```sh
GRAFTON_VISCA_BLESS=1 cargo +1.98.0 test --all-features --lib \
  generated_docs::generated_documents
```

After reviewing the deliberate-failure run's diff, verify the blocks:

```sh
cargo +1.98.0 test --all-features --lib generated_docs::generated_documents
cargo +1.98.0 test --features serde --lib authored_row_fact_keys_match_every_exported_profile
```

For an intentional encoding change, regenerate the wire fixture separately:

```sh
GRAFTON_VISCA_BLESS=1 cargo +1.98.0 test --lib wire_ledger_matches_literal_golden_inventory
```

Review its literal packets against the model source, then rerun that test with
`GRAFTON_VISCA_BLESS` unset. Generated bytes alone are not independent evidence.

The generated vocabulary and current model boundaries follow. The method table
shows each method's complete gate set and result type, so a profile contribution's
full public impact is visible in the documentation diff.

<!-- BEGIN GENERATED TYPED SUPPORT VOCABULARY -->
| Area | Marker | Typed surface |
| ---- | ------ | ------------- |
| Zoom | `HasDirectZoom` | `zoom().set_position`, `zoom().set_normalized`, `zoom().normalized` |
| Zoom | `HasDigitalZoomToggle` | `zoom().set_digital_zoom` |
| Zoom | `HasDigitalZoomRange` | Runtime-only gate for `zoom().set_normalized(..., ZoomDomain::OpticalPlusDigital)`; also requires a documented digital maximum |
| Exposure | `HasIris` | `exposure().iris`, `exposure().iris_reset`, `exposure().iris_up`, `exposure().iris_down`, `exposure().set_iris` |
| Focus | `HasOnePushFocus` | `focus().one_push` |
| Focus | `HasSnapFocus` | `focus().snap` |
| Focus | `HasFocusLock` | `focus().set_lock` |
| Focus | `HasPushAutoFocus` | `focus().push_auto_focus_press`, `focus().push_auto_focus_release` |
| Focus | `HasFocusZone` | `focus().set_zone` |
| Focus | `HasAutoFocusSensitivity` | `focus().set_sensitivity`, `focus().sensitivity` |
| Focus | `HasFocusNearLimit` | `focus().set_near_limit`, `focus().near_limit` |
| Exposure | `HasBacklightCompensation` | `exposure().backlight_compensation`, `exposure().set_backlight_compensation` |
| Exposure | `HasWideDynamicRange` | `exposure().set_wide_dynamic_range` |
| Exposure | `HasExposureCompensation` | `exposure().compensation`, `exposure().compensation_enabled`, `exposure().set_compensation_enabled`, `exposure().compensation_reset`, `exposure().compensation_up`, `exposure().compensation_down`, `exposure().set_compensation` |
| Exposure | `HasBrightness` | `exposure().brightness`, `exposure().brightness_reset`, `exposure().brightness_up`, `exposure().brightness_down`, `exposure().set_brightness` |
| White balance | `HasOnePushWhiteBalance` | `white_balance().one_push` |
| White balance | `HasAutoWhiteBalanceSensitivity` | `white_balance().set_sensitivity`, `white_balance().sensitivity` |
| Color | `HasColorTemperature` | `white_balance().color_temperature_reset`, `white_balance().color_temperature_up`, `white_balance().color_temperature_down`, `white_balance().set_color_temperature` |
| Color | `HasRgbGain` | `white_balance().red_gain`, `white_balance().red_gain_reset`, `white_balance().red_gain_up`, `white_balance().red_gain_down`, `white_balance().set_red_gain`, `white_balance().blue_gain`, `white_balance().blue_gain_reset`, `white_balance().blue_gain_up`, `white_balance().blue_gain_down`, `white_balance().set_blue_gain` |
| Color | `HasRgbTuning` | `white_balance().set_red_tuning`, `white_balance().set_blue_tuning`, `white_balance().red_tuning`, `white_balance().blue_tuning` |
| Image | `HasImageFlip` | `image().set_flip`, `image().flip` |
| Image | `HasImageMirror` | `image().set_mirror` |
| Image | `HasCombinedImageFlip` | `image().set_flip_mode`, `image().flip_mode` |
| Image | `HasContrast` | `image().contrast`, `image().set_contrast` |
| Image | `HasSharpness` | `image().sharpness`, `image().sharpness_reset`, `image().sharpness_up`, `image().sharpness_down`, `image().set_sharpness` |
| Image | `HasSaturation` | `image().saturation`, `image().set_saturation` |
| Image | `HasHue` | `image().hue`, `image().set_hue` |
| Image | `HasLuminance` | `image().luminance`, `image().set_luminance` |
| Image | `HasGamma` | `image().gamma`, `image().set_gamma` |
| Image | `HasNoiseReduction2D` | `image().noise_reduction_2d`, `image().set_noise_reduction_2d` |
| Image | `HasNoiseReduction3D` | `image().noise_reduction_3d`, `image().set_noise_reduction_3d` |
| Image | `HasPictureEffect` | `image().picture_effect`, `image().set_picture_effect` |
| Tally | `HasTally` | `tally().set_red`, `tally().set_brightness`, `tally().set_green`, `tally().set_mode`, `tally().red`, `tally().green` |
| Menu | `HasDirectMenu` | `menu().direct`, `menu().toggle_visible` |
| ND filter | `HasNdFilter` | `nd_filter().density`, `nd_filter().preset`, `nd_filter().set_mode`, `nd_filter().set_density`, `nd_filter().density_up`, `nd_filter().density_down`, `nd_filter().set_auto` |
| Variable speed | `HasPanTiltSpeedMode` | `pan_tilt().set_speed_mode` |
| Motion Sync | `HasMotionSync` | `motion_sync().enabled`, `motion_sync().set_enabled`, `motion_sync().set_speed` |
| Focus | `HasFocusZoneInquiry` | `focus().zone` |
| Streaming | `HasUsbAudio` | `streaming().usb_audio`, `streaming().set_usb_audio` |
| Exposure | `HasAntiFlicker` | `exposure().set_anti_flicker`, `exposure().anti_flicker` |
| System | `HasSettingsSave` | `system().save_settings` |
| Presets | `HasPresetRecallSpeed` | `presets().set_recall_speed` |
| Presets | `HasPresetRecallSpeedInquiry` | `presets().recall_speed` |
| Exposure | `HasSpotlight` | `exposure().set_spotlight` |
| Exposure | `HasAutoSlowShutter` | `exposure().set_auto_slow_shutter` |
| Streaming | `HasMulticastStreaming` | `streaming().set_multicast` |
| Streaming | `HasNdiQuality` | `streaming().set_ndi_quality` |
| Exposure | `HasExposureMode` | `exposure().mode`, `exposure().set_mode` |
| Image | `HasImageFreeze` | `image().set_freeze` |
| Tally | `HasTallyBrightness` | `tally().set_brightness` |
| Tally | `HasTallyMode` | `tally().set_mode` |
| System | `HasVersionInquiry` | `system().version` |
| Color | `HasColorTemperatureInquiry` | `white_balance().color_temperature` |
| Image | `HasNoiseReduction2DMode` | `image().noise_reduction_2d_mode`, `image().set_noise_reduction_2d_mode` |
| Exposure | `HasAeLevel` | `exposure().ae_level_up`, `exposure().ae_level_down`, `exposure().ae_level` |
| Exposure | `HasAutoGain` | `exposure().set_auto_gain`, `exposure().auto_gain` |
| Exposure | `HasAutoShutter` | `exposure().set_auto_shutter`, `exposure().auto_shutter` |
| Exposure | `HasIrisSteps` | `exposure().iris_up_by`, `exposure().iris_down_by` |
| Exposure | `HasAutoIris` | `exposure().set_auto_iris`, `exposure().auto_iris` |
| WhiteBalance | `HasPresetWhite` | `white_balance().preset_white_up_by`, `white_balance().preset_white_down_by`, `white_balance().set_preset_white`, `white_balance().preset_white` |
| WhiteBalance | `HasMemoryColorTemperature` | `white_balance().memory_color_temperature_up_by`, `white_balance().memory_color_temperature_down_by`, `white_balance().set_memory_color_temperature`, `white_balance().memory_color_temperature` |
| WhiteBalance | `HasMemoryGain` | `white_balance().memory_red_gain_up_by`, `white_balance().memory_red_gain_down_by`, `white_balance().set_memory_red_gain`, `white_balance().memory_red_gain`, `white_balance().memory_blue_gain_up_by`, `white_balance().memory_blue_gain_down_by`, `white_balance().set_memory_blue_gain`, `white_balance().memory_blue_gain` |
| WhiteBalance | `HasMemoryTint` | `white_balance().memory_tint_up_by`, `white_balance().memory_tint_down_by`, `white_balance().set_memory_tint`, `white_balance().memory_tint` |
| WhiteBalance | `HasAtwOffsets` | `white_balance().atw_color_temperature_offset_up_by`, `white_balance().atw_color_temperature_offset_down_by`, `white_balance().set_atw_color_temperature_offset`, `white_balance().atw_color_temperature_offset`, `white_balance().atw_tint_offset_up_by`, `white_balance().atw_tint_offset_down_by`, `white_balance().set_atw_tint_offset`, `white_balance().atw_tint_offset` |
| Image | `HasBlackLevels` | `image().master_black_up_by`, `image().master_black_down_by`, `image().set_master_black`, `image().master_black`, `image().red_black_up_by`, `image().red_black_down_by`, `image().set_red_black`, `image().red_black`, `image().blue_black_up_by`, `image().blue_black_down_by`, `image().set_blue_black`, `image().blue_black` |
| Image | `HasDetail` | `image().set_detail`, `image().detail` |
| Image | `HasKnee` | `image().set_knee`, `image().knee`, `image().set_knee_mode`, `image().knee_mode`, `image().set_knee_slope`, `image().knee_slope`, `image().set_knee_point`, `image().knee_point`, `image().knee_permission` |
| Zoom | `HasHighResolutionZoom` | `zoom().tele_high_resolution`, `zoom().wide_high_resolution`, `zoom().stop_high_resolution` |
| Zoom | `HasClearImageZoom` | `zoom().clear_image_position` |
| Presets | `HasSeparatePresetSpeed` | `presets().set_speed_selection`, `presets().speed_selection`, `presets().set_separate_mode`, `presets().separate_mode`, `presets().set_separate_speed`, `presets().separate_speed` |
| Presets | `HasSeparatePresetDuration` | `presets().set_separate_duration`, `presets().separate_duration` |
| Image | `HasSharpnessMode` | `image().sharpness_mode`, `image().set_sharpness_mode` |
| Focus | `HasFocusInfinity` | `focus().infinity` |
| Presets | `HasPresetMemory` | `presets().store`, `presets().clear` |
| Exposure | `HasShutter` | `exposure().shutter`, `exposure().shutter_reset`, `exposure().shutter_up`, `exposure().shutter_down` |
| Exposure | `HasGain` | `exposure().gain`, `exposure().gain_reset`, `exposure().gain_up`, `exposure().gain_down`, `exposure().set_gain` |
| Exposure | `HasGainLimit` | `exposure().gain_limit`, `exposure().set_gain_limit` |
| Menu | `HasMenuDisplay` | `menu().set_visible` |
| Menu | `HasMenuEnterReturn` | `menu().enter`, `menu().back` |
| Menu | `HasMenuDisplayInquiry` | `menu().visible` |
| Exposure | `HasDirectShutter` | `exposure().set_shutter` |
| Power | `HasPowerStandby` | `power().standby` |
<!-- END GENERATED TYPED SUPPORT VOCABULARY -->

<!-- BEGIN GENERATED TYPED METHODS -->
| Method | Required markers | Result payload | Built-in profiles |
| --- | --- | --- | --- |
| `power().status` | `HasPower` | `types :: PowerStatus` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `power().on` | `HasPower` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `power().standby` | `HasPower`, `HasPowerStandby` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100` |
| `zoom().position` | `HasZoom` | `types :: ZoomPosition` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().tele` | `HasZoom` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().wide` | `HasZoom` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().stop` | `HasZoom` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().tele_variable` | `HasZoom` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().wide_variable` | `HasZoom` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `zoom().set_position` | `HasZoom`, `HasDirectZoom` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `zoom().set_normalized` | `HasZoom`, `HasDirectZoom` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `zoom().set_digital_zoom` | `HasZoom`, `HasDigitalZoomToggle` | `()` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `zoom().tele_high_resolution` | `HasZoom`, `HasHighResolutionZoom` | `Operation<AppliedOnly>` | `SonyFr7` |
| `zoom().wide_high_resolution` | `HasZoom`, `HasHighResolutionZoom` | `Operation<AppliedOnly>` | `SonyFr7` |
| `zoom().stop_high_resolution` | `HasZoom`, `HasHighResolutionZoom` | `Operation<AppliedOnly>` | `SonyFr7` |
| `zoom().clear_image_position` | `HasZoom`, `HasClearImageZoom` | `types :: ClearImageZoomPosition` | `SonyFr7` |
| `zoom().normalized` | `HasZoom`, `HasDirectZoom` | `types :: UnitInterval` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `system().version` | `HasVersionInquiry` | `types :: VersionInfo` | `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `system().save_settings` | `HasSettingsSave` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `pan_tilt().position` | `HasPanTilt` | `types :: PanTiltPosition` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().home` | `HasPanTilt` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().reset` | `HasPanTilt` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().drive` | `HasPanTilt` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().stop` | `HasPanTilt` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().absolute` | `HasPanTilt` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().relative` | `HasPanTilt` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().set_limit` | `HasPanTilt` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().clear_limit` | `HasPanTilt` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `pan_tilt().set_speed_mode` | `HasPanTilt`, `HasPanTiltSpeedMode` | `()` | `SonyFr7` |
| `focus().position` | `HasFocus` | `types :: FocusPosition` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().mode` | `HasFocus` | `types :: FocusMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().far` | `HasFocus` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().near` | `HasFocus` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().far_variable` | `HasFocus` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().near_variable` | `HasFocus` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().stop` | `HasFocus` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().set_position` | `HasFocus` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().one_push` | `HasFocus`, `HasOnePushFocus` | `Operation<AppliedOnly>` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().infinity` | `HasFocus`, `HasFocusInfinity` | `Operation<Targeted>` | `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().toggle_mode` | `HasFocus` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `focus().snap` | `HasFocus`, `HasSnapFocus` | `Operation<AppliedOnly>` | No built-in profile |
| `focus().set_zone` | `HasFocus`, `HasFocusZone` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `focus().set_sensitivity` | `HasFocus`, `HasAutoFocusSensitivity` | `()` | No built-in profile |
| `focus().set_near_limit` | `HasFocus`, `HasFocusNearLimit` | `()` | `SonyEviH100` |
| `focus().set_lock` | `HasFocus`, `HasFocusLock` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `focus().push_auto_focus_press` | `HasFocus`, `HasPushAutoFocus` | `Operation<AppliedOnly>` | `SonyFr7` |
| `focus().push_auto_focus_release` | `HasFocus`, `HasPushAutoFocus` | `Operation<AppliedOnly>` | `SonyFr7` |
| `focus().near_limit` | `HasFocus`, `HasFocusNearLimit` | `types :: FocusPosition` | `SonyEviH100` |
| `focus().zone` | `HasFocus`, `HasFocusZoneInquiry` | `types :: FocusZone` | `PtzOpticsG2`, `PtzOptics30x` |
| `focus().sensitivity` | `HasFocus`, `HasAutoFocusSensitivity` | `types :: AutoFocusSensitivity` | No built-in profile |
| `focus().set_mode` | `HasFocus` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `presets().recall` | `HasPresets` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `presets().set_recall_speed` | `HasPresets`, `HasPresetRecallSpeed` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7` |
| `presets().store` | `HasPresets`, `HasPresetMemory` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `presets().clear` | `HasPresets`, `HasPresetMemory` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `presets().set_speed_selection` | `HasPresets`, `HasSeparatePresetSpeed` | `()` | `SonyFr7` |
| `presets().speed_selection` | `HasPresets`, `HasSeparatePresetSpeed` | `types :: PresetSpeedSelection` | `SonyFr7` |
| `presets().recall_speed` | `HasPresets`, `HasPresetRecallSpeedInquiry` | `types :: PresetSpeed` | `SonyFr7` |
| `presets().set_separate_mode` | `HasPresets`, `HasSeparatePresetSpeed` | `()` | `SonyFr7` |
| `presets().separate_mode` | `HasPresets`, `HasSeparatePresetSpeed` | `types :: PresetSeparateMode` | `SonyFr7` |
| `presets().set_separate_speed` | `HasPresets`, `HasSeparatePresetSpeed` | `()` | `SonyFr7` |
| `presets().separate_speed` | `HasPresets`, `HasSeparatePresetSpeed` | `types :: PresetSpeed` | `SonyFr7` |
| `presets().set_separate_duration` | `HasPresets`, `HasSeparatePresetDuration` | `()` | `SonyFr7` |
| `presets().separate_duration` | `HasPresets`, `HasSeparatePresetDuration` | `std :: time :: Duration` | `SonyFr7` |
| `exposure().mode` | `HasExposure`, `HasExposureMode` | `types :: ExposureMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().set_mode` | `HasExposure`, `HasExposureMode` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().shutter` | `HasExposure`, `HasShutter` | `types :: ShutterSpeed` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().shutter_reset` | `HasExposure`, `HasShutter` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().shutter_up` | `HasExposure`, `HasShutter` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().shutter_down` | `HasExposure`, `HasShutter` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().set_shutter` | `HasExposure`, `HasDirectShutter` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().compensation` | `HasExposure`, `HasExposureCompensation` | `types :: ExposureCompensationLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().compensation_enabled` | `HasExposure`, `HasExposureCompensation` | `bool` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().set_compensation_enabled` | `HasExposure`, `HasExposureCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().compensation_reset` | `HasExposure`, `HasExposureCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().compensation_up` | `HasExposure`, `HasExposureCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().compensation_down` | `HasExposure`, `HasExposureCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().set_compensation` | `HasExposure`, `HasExposureCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().set_wide_dynamic_range` | `HasExposure`, `HasWideDynamicRange` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `exposure().iris` | `HasExposure`, `HasIris` | `types :: IrisLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().iris_reset` | `HasExposure`, `HasIris` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().iris_up` | `HasExposure`, `HasIris` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().iris_down` | `HasExposure`, `HasIris` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().set_iris` | `HasExposure`, `HasIris` | `Operation<Targeted>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().brightness` | `HasExposure`, `HasBrightness` | `types :: BrightnessLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().brightness_reset` | `HasExposure`, `HasBrightness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().brightness_up` | `HasExposure`, `HasBrightness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().brightness_down` | `HasExposure`, `HasBrightness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().set_brightness` | `HasExposure`, `HasBrightness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().gain` | `HasExposure`, `HasGain` | `types :: GainLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().gain_reset` | `HasExposure`, `HasGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().gain_up` | `HasExposure`, `HasGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().gain_down` | `HasExposure`, `HasGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().set_gain` | `HasExposure`, `HasGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `exposure().gain_limit` | `HasExposure`, `HasGainLimit` | `types :: GainLimit` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `exposure().set_gain_limit` | `HasExposure`, `HasGainLimit` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `exposure().set_anti_flicker` | `HasExposure`, `HasAntiFlicker` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `exposure().anti_flicker` | `HasExposure`, `HasAntiFlicker` | `types :: AntiFlickerMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `exposure().set_spotlight` | `HasExposure`, `HasSpotlight` | `()` | `SonyFr7` |
| `exposure().set_auto_slow_shutter` | `HasExposure`, `HasAutoSlowShutter` | `()` | `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().ae_level_up` | `HasExposure`, `HasAeLevel` | `()` | `SonyFr7` |
| `exposure().ae_level_down` | `HasExposure`, `HasAeLevel` | `()` | `SonyFr7` |
| `exposure().ae_level` | `HasExposure`, `HasAeLevel` | `types :: AeLevel` | `SonyFr7` |
| `exposure().set_auto_gain` | `HasExposure`, `HasAutoGain` | `()` | `SonyFr7` |
| `exposure().auto_gain` | `HasExposure`, `HasAutoGain` | `bool` | `SonyFr7` |
| `exposure().set_auto_shutter` | `HasExposure`, `HasAutoShutter` | `()` | `SonyFr7` |
| `exposure().auto_shutter` | `HasExposure`, `HasAutoShutter` | `bool` | `SonyFr7` |
| `exposure().set_auto_iris` | `HasExposure`, `HasAutoIris` | `()` | `SonyFr7` |
| `exposure().auto_iris` | `HasExposure`, `HasAutoIris` | `bool` | `SonyFr7` |
| `exposure().iris_up_by` | `HasExposure`, `HasIrisSteps` | `Operation<AppliedOnly>` | `SonyFr7` |
| `exposure().iris_down_by` | `HasExposure`, `HasIrisSteps` | `Operation<AppliedOnly>` | `SonyFr7` |
| `exposure().backlight_compensation` | `HasExposure`, `HasBacklightCompensation` | `bool` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `exposure().set_backlight_compensation` | `HasExposure`, `HasBacklightCompensation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().mode` | `HasWhiteBalance` | `types :: WhiteBalanceMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `white_balance().set_sensitivity` | `HasWhiteBalance`, `HasAutoWhiteBalanceSensitivity` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().sensitivity` | `HasWhiteBalance`, `HasAutoWhiteBalanceSensitivity` | `types :: AutoWhiteBalanceSensitivity` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().one_push` | `HasWhiteBalance`, `HasOnePushWhiteBalance` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `white_balance().set_red_tuning` | `HasWhiteBalance`, `HasRgbTuning` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().set_blue_tuning` | `HasWhiteBalance`, `HasRgbTuning` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().color_temperature` | `HasWhiteBalance`, `HasColorTemperatureInquiry` | `types :: Kelvin` | `PtzOpticsG2`, `PtzOptics30x` |
| `white_balance().color_temperature_reset` | `HasWhiteBalance`, `HasColorTemperature` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().color_temperature_up` | `HasWhiteBalance`, `HasColorTemperature` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().color_temperature_down` | `HasWhiteBalance`, `HasColorTemperature` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().set_color_temperature` | `HasWhiteBalance`, `HasColorTemperature` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().red_gain` | `HasWhiteBalance`, `HasRgbGain` | `types :: RgbGain` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().red_gain_reset` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().red_gain_up` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().red_gain_down` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().set_red_gain` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().blue_gain` | `HasWhiteBalance`, `HasRgbGain` | `types :: RgbGain` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().blue_gain_reset` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().blue_gain_up` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().blue_gain_down` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().set_blue_gain` | `HasWhiteBalance`, `HasRgbGain` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300` |
| `white_balance().red_tuning` | `HasWhiteBalance`, `HasRgbTuning` | `types :: RgbTuning` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().blue_tuning` | `HasWhiteBalance`, `HasRgbTuning` | `types :: RgbTuning` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `white_balance().preset_white_up_by` | `HasWhiteBalance`, `HasPresetWhite` | `()` | `SonyFr7` |
| `white_balance().preset_white_down_by` | `HasWhiteBalance`, `HasPresetWhite` | `()` | `SonyFr7` |
| `white_balance().set_preset_white` | `HasWhiteBalance`, `HasPresetWhite` | `()` | `SonyFr7` |
| `white_balance().preset_white` | `HasWhiteBalance`, `HasPresetWhite` | `types :: Kelvin` | `SonyFr7` |
| `white_balance().memory_color_temperature_up_by` | `HasWhiteBalance`, `HasMemoryColorTemperature` | `()` | `SonyFr7` |
| `white_balance().memory_color_temperature_down_by` | `HasWhiteBalance`, `HasMemoryColorTemperature` | `()` | `SonyFr7` |
| `white_balance().set_memory_color_temperature` | `HasWhiteBalance`, `HasMemoryColorTemperature` | `()` | `SonyFr7` |
| `white_balance().memory_color_temperature` | `HasWhiteBalance`, `HasMemoryColorTemperature` | `types :: Kelvin` | `SonyFr7` |
| `white_balance().memory_red_gain_up_by` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().memory_red_gain_down_by` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().set_memory_red_gain` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().memory_red_gain` | `HasWhiteBalance`, `HasMemoryGain` | `types :: WhiteBalanceGain` | `SonyFr7` |
| `white_balance().memory_blue_gain_up_by` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().memory_blue_gain_down_by` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().set_memory_blue_gain` | `HasWhiteBalance`, `HasMemoryGain` | `()` | `SonyFr7` |
| `white_balance().memory_blue_gain` | `HasWhiteBalance`, `HasMemoryGain` | `types :: WhiteBalanceGain` | `SonyFr7` |
| `white_balance().memory_tint_up_by` | `HasWhiteBalance`, `HasMemoryTint` | `()` | `SonyFr7` |
| `white_balance().memory_tint_down_by` | `HasWhiteBalance`, `HasMemoryTint` | `()` | `SonyFr7` |
| `white_balance().set_memory_tint` | `HasWhiteBalance`, `HasMemoryTint` | `()` | `SonyFr7` |
| `white_balance().memory_tint` | `HasWhiteBalance`, `HasMemoryTint` | `types :: WhiteBalanceOffset` | `SonyFr7` |
| `white_balance().atw_color_temperature_offset_up_by` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().atw_color_temperature_offset_down_by` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().set_atw_color_temperature_offset` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().atw_color_temperature_offset` | `HasWhiteBalance`, `HasAtwOffsets` | `types :: WhiteBalanceOffset` | `SonyFr7` |
| `white_balance().atw_tint_offset_up_by` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().atw_tint_offset_down_by` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().set_atw_tint_offset` | `HasWhiteBalance`, `HasAtwOffsets` | `()` | `SonyFr7` |
| `white_balance().atw_tint_offset` | `HasWhiteBalance`, `HasAtwOffsets` | `types :: WhiteBalanceOffset` | `SonyFr7` |
| `white_balance().set_mode` | `HasWhiteBalance` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `image().saturation` | `HasImage`, `HasSaturation` | `types :: SaturationLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().set_saturation` | `HasImage`, `HasSaturation` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().hue` | `HasImage`, `HasHue` | `types :: HueLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().set_hue` | `HasImage`, `HasHue` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().luminance` | `HasImage`, `HasLuminance` | `types :: LuminanceLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_luminance` | `HasImage`, `HasLuminance` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().contrast` | `HasImage`, `HasContrast` | `types :: ContrastLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_contrast` | `HasImage`, `HasContrast` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().gamma` | `HasImage`, `HasGamma` | `types :: GammaLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().set_gamma` | `HasImage`, `HasGamma` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().sharpness_mode` | `HasImage`, `HasSharpnessMode` | `types :: SharpnessMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().sharpness` | `HasImage`, `HasSharpness` | `types :: SharpnessLevel` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `image().set_sharpness_mode` | `HasImage`, `HasSharpnessMode` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().sharpness_reset` | `HasImage`, `HasSharpness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `image().sharpness_up` | `HasImage`, `HasSharpness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `image().sharpness_down` | `HasImage`, `HasSharpness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `image().set_sharpness` | `HasImage`, `HasSharpness` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyEviH100` |
| `image().noise_reduction_2d` | `HasImage`, `HasNoiseReduction2D` | `types :: NoiseReduction2D` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().noise_reduction_2d_mode` | `HasImage`, `HasNoiseReduction2DMode` | `types :: NoiseReduction2DMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().noise_reduction_3d` | `HasImage`, `HasNoiseReduction3D` | `types :: NoiseReduction3D` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_noise_reduction_2d_mode` | `HasImage`, `HasNoiseReduction2DMode` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_noise_reduction_2d` | `HasImage`, `HasNoiseReduction2D` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100` |
| `image().set_noise_reduction_3d` | `HasImage`, `HasNoiseReduction3D` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_flip` | `HasImage`, `HasImageFlip` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrc300`, `NearusBrc300` |
| `image().set_mirror` | `HasImage`, `HasImageMirror` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_flip_mode` | `HasImage`, `HasCombinedImageFlip` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_freeze` | `HasImage`, `HasImageFreeze` | `()` | No built-in profile |
| `image().flip` | `HasImage`, `HasImageFlip` | `bool` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyBrc300`, `NearusBrc300` |
| `image().flip_mode` | `HasImage`, `HasCombinedImageFlip` | `types :: ImageFlipMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().picture_effect` | `HasImage`, `HasPictureEffect` | `types :: PictureEffectMode` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().set_picture_effect` | `HasImage`, `HasPictureEffect` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `image().master_black_up_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().master_black_down_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().set_master_black` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().master_black` | `HasImage`, `HasBlackLevels` | `types :: BlackLevel` | `SonyFr7` |
| `image().red_black_up_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().red_black_down_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().set_red_black` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().red_black` | `HasImage`, `HasBlackLevels` | `types :: BlackLevel` | `SonyFr7` |
| `image().blue_black_up_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().blue_black_down_by` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().set_blue_black` | `HasImage`, `HasBlackLevels` | `()` | `SonyFr7` |
| `image().blue_black` | `HasImage`, `HasBlackLevels` | `types :: BlackLevel` | `SonyFr7` |
| `image().set_detail` | `HasImage`, `HasDetail` | `()` | `SonyFr7` |
| `image().detail` | `HasImage`, `HasDetail` | `bool` | `SonyFr7` |
| `image().set_knee` | `HasImage`, `HasKnee` | `()` | `SonyFr7` |
| `image().knee` | `HasImage`, `HasKnee` | `bool` | `SonyFr7` |
| `image().set_knee_mode` | `HasImage`, `HasKnee` | `()` | `SonyFr7` |
| `image().knee_mode` | `HasImage`, `HasKnee` | `types :: KneeMode` | `SonyFr7` |
| `image().set_knee_slope` | `HasImage`, `HasKnee` | `()` | `SonyFr7` |
| `image().knee_slope` | `HasImage`, `HasKnee` | `types :: KneeSlope` | `SonyFr7` |
| `image().set_knee_point` | `HasImage`, `HasKnee` | `()` | `SonyFr7` |
| `image().knee_point` | `HasImage`, `HasKnee` | `types :: KneePoint` | `SonyFr7` |
| `image().knee_permission` | `HasImage`, `HasKnee` | `types :: KneePermission` | `SonyFr7` |
| `tally().set_red` | `HasTally` | `()` | `SonyFr7` |
| `tally().set_brightness` | `HasTally`, `HasTallyBrightness` | `()` | No built-in profile |
| `tally().set_green` | `HasTally` | `()` | `SonyFr7` |
| `tally().set_mode` | `HasTally`, `HasTallyMode` | `()` | No built-in profile |
| `tally().red` | `HasTally` | `bool` | `SonyFr7` |
| `tally().green` | `HasTally` | `bool` | `SonyFr7` |
| `nd_filter().density` | `HasNdFilter` | `types :: NdFilterDensity` | `SonyFr7` |
| `nd_filter().preset` | `HasNdFilter` | `types :: NdFilterPreset` | `SonyFr7` |
| `nd_filter().set_mode` | `HasNdFilter` | `()` | `SonyFr7` |
| `nd_filter().set_density` | `HasNdFilter` | `Operation<Targeted>` | `SonyFr7` |
| `nd_filter().density_up` | `HasNdFilter` | `Operation<Targeted>` | `SonyFr7` |
| `nd_filter().density_down` | `HasNdFilter` | `Operation<Targeted>` | `SonyFr7` |
| `nd_filter().set_auto` | `HasNdFilter` | `()` | `SonyFr7` |
| `motion_sync().enabled` | `HasMotionSync` | `bool` | No built-in profile |
| `motion_sync().set_enabled` | `HasMotionSync` | `()` | No built-in profile |
| `motion_sync().set_speed` | `HasMotionSync` | `()` | No built-in profile |
| `menu().visible` | `HasMenu`, `HasMenuDisplayInquiry` | `bool` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900` |
| `menu().set_visible` | `HasMenu`, `HasMenuDisplay` | `()` | `SonyFr7` |
| `menu().navigate` | `HasMenu` | `Operation<AppliedOnly>` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `menu().enter` | `HasMenu`, `HasMenuEnterReturn` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `menu().back` | `HasMenu`, `HasMenuEnterReturn` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `menu().direct` | `HasMenu`, `HasDirectMenu` | `()` | `SonyFr7` |
| `menu().toggle_visible` | `HasMenu`, `HasDirectMenu` | `()` | `SonyFr7` |
| `streaming().usb_audio` | `HasUsbAudio` | `bool` | `PtzOpticsG2`, `PtzOptics30x` |
| `streaming().set_multicast` | `HasMulticastStreaming` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `streaming().set_ndi_quality` | `HasNdiQuality` | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x` |
| `streaming().set_usb_audio` | `HasUsbAudio` | `()` | `PtzOpticsG2`, `PtzOptics30x` |
| `motion().stop_all` | none | `crate :: operation :: HaltReport` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `motion().is_moving` | none | `bool` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
| `motion().wait_until_idle` | none | `()` | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyFr7`, `SonyBrcH900`, `SonyEviH100`, `SonyBrc300`, `NearusBrc300`, `GenericVisca` |
<!-- END GENERATED TYPED METHODS -->

The image inquiry registers are separate evidence boundaries. `image().flip()`
uses the sourced vertical `09 04 66` register under `HasImageFlip` and returns
a boolean vertical state. `image().flip_mode()` queries
combined `09 04 A4` state as `ImageFlipMode` under `HasCombinedImageFlip`;
only the PTZOptics profiles grant that combined family. A flip setter does not establish a
combined inquiry on another model (#844).

Sony FR7 variable-ND readback uses the sourced `09 7E 04 42` inquiry and
returns `NdFilterDensity` (`00..14` hex), with strict four-nibble zero-padded
replies. It is a variable-density code, distinct from clear/preset selection.
Motion and settlement use that same density readback; R7 v4 pp. 21 and 25
are the packet/value evidence (#844).

Every typed inquiry admitted by a built-in profile must have a source rationale
in the inquiry inventory. The typed surface excludes unsourced focus-range,
night/day, standby, digital-PTZ, auto-trace, focus-unlock, broadcast-domain,
two-tone, digital-mode, and dynamic-range queries. Raw inquiries with explicit
model evidence remain available; the sourced dynamic-range setter is retained.
`FStop` is descriptive, with no universal wire conversion to `IrisLevel`: use
an iris code established by that profile's model source (#844).

Runtime typed noun gates compose with their row refinements. A tally brightness
or PTZOptics tally row requires `Tally` as well as its refining surface, matching
the static `HasTally` header. Granting a refinement alone never permits a frame
through the dynamic API (#855).

Persisted `ProfileSpec` facts require every current key, including optional
facts nested in capabilities, ranges and transport compatibility. Explicit
`null` means an absent fact; omitting the key is refused with the regeneration
instruction. Rebuild built-ins with `ProfileId::spec()` or
custom profiles with `ProfileSpec::builder`, then persist the current result
(#855).

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

## Built-In Profile Marker Matrix

The table below lists optional typed support markers emitted by the registry.
Domain records and their separate domain markers establish which nouns exist;
optional markers establish the supported methods within those nouns.

Do not add a marker to a heterogeneous typed aggregate unless every profile
behind it implements the marker. Runtime-selected callers check
`camera.capabilities().supports(TypedSupportSurface::...)` before selecting an
optional operation.

<!-- BEGIN GENERATED PROFILE MARKERS -->
| Profile | Typed support markers |
| --- | --- |
| `PtzOpticsG2` | `HasDirectZoom`, `HasIris`, `HasFocusLock`, `HasFocusZone`, `HasBacklightCompensation`, `HasWideDynamicRange`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasAutoWhiteBalanceSensitivity`, `HasColorTemperature`, `HasRgbGain`, `HasRgbTuning`, `HasImageFlip`, `HasImageMirror`, `HasCombinedImageFlip`, `HasContrast`, `HasSharpness`, `HasSaturation`, `HasHue`, `HasLuminance`, `HasGamma`, `HasNoiseReduction2D`, `HasNoiseReduction3D`, `HasPictureEffect`, `HasFocusZoneInquiry`, `HasUsbAudio`, `HasAntiFlicker`, `HasSettingsSave`, `HasPresetRecallSpeed`, `HasMulticastStreaming`, `HasNdiQuality`, `HasExposureMode`, `HasColorTemperatureInquiry`, `HasNoiseReduction2DMode`, `HasSharpnessMode`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasGainLimit`, `HasMenuEnterReturn`, `HasMenuDisplayInquiry`, `HasDirectShutter`, `HasPowerStandby` |
| `PtzOpticsG3` | `HasDirectZoom`, `HasIris`, `HasFocusLock`, `HasFocusZone`, `HasBacklightCompensation`, `HasWideDynamicRange`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasAutoWhiteBalanceSensitivity`, `HasColorTemperature`, `HasRgbGain`, `HasRgbTuning`, `HasImageFlip`, `HasImageMirror`, `HasCombinedImageFlip`, `HasContrast`, `HasSharpness`, `HasSaturation`, `HasHue`, `HasLuminance`, `HasGamma`, `HasNoiseReduction2D`, `HasNoiseReduction3D`, `HasPictureEffect`, `HasAntiFlicker`, `HasSettingsSave`, `HasPresetRecallSpeed`, `HasMulticastStreaming`, `HasNdiQuality`, `HasExposureMode`, `HasNoiseReduction2DMode`, `HasSharpnessMode`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasGainLimit`, `HasMenuEnterReturn`, `HasMenuDisplayInquiry`, `HasDirectShutter`, `HasPowerStandby` |
| `PtzOptics30x` | `HasDirectZoom`, `HasIris`, `HasFocusLock`, `HasFocusZone`, `HasBacklightCompensation`, `HasWideDynamicRange`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasAutoWhiteBalanceSensitivity`, `HasColorTemperature`, `HasRgbGain`, `HasRgbTuning`, `HasImageFlip`, `HasImageMirror`, `HasCombinedImageFlip`, `HasContrast`, `HasSharpness`, `HasSaturation`, `HasHue`, `HasLuminance`, `HasGamma`, `HasNoiseReduction2D`, `HasNoiseReduction3D`, `HasPictureEffect`, `HasFocusZoneInquiry`, `HasUsbAudio`, `HasAntiFlicker`, `HasSettingsSave`, `HasPresetRecallSpeed`, `HasMulticastStreaming`, `HasNdiQuality`, `HasExposureMode`, `HasColorTemperatureInquiry`, `HasNoiseReduction2DMode`, `HasSharpnessMode`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasGainLimit`, `HasMenuEnterReturn`, `HasMenuDisplayInquiry`, `HasDirectShutter`, `HasPowerStandby` |
| `SonyFr7` | `HasDirectZoom`, `HasPushAutoFocus`, `HasBacklightCompensation`, `HasOnePushWhiteBalance`, `HasSharpness`, `HasTally`, `HasDirectMenu`, `HasNdFilter`, `HasPanTiltSpeedMode`, `HasPresetRecallSpeed`, `HasPresetRecallSpeedInquiry`, `HasSpotlight`, `HasVersionInquiry`, `HasAeLevel`, `HasAutoGain`, `HasAutoShutter`, `HasIrisSteps`, `HasAutoIris`, `HasPresetWhite`, `HasMemoryColorTemperature`, `HasMemoryGain`, `HasMemoryTint`, `HasAtwOffsets`, `HasBlackLevels`, `HasDetail`, `HasKnee`, `HasHighResolutionZoom`, `HasClearImageZoom`, `HasSeparatePresetSpeed`, `HasSeparatePresetDuration`, `HasPresetMemory`, `HasMenuDisplay`, `HasMenuDisplayInquiry`, `HasPowerStandby` |
| `SonyBrcH900` | `HasIris`, `HasExposureMode`, `HasVersionInquiry`, `HasFocusInfinity`, `HasShutter`, `HasGain`, `HasMenuDisplayInquiry`, `HasPowerStandby` |
| `SonyEviH100` | `HasDirectZoom`, `HasDigitalZoomToggle`, `HasDigitalZoomRange`, `HasIris`, `HasOnePushFocus`, `HasFocusNearLimit`, `HasBacklightCompensation`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasRgbGain`, `HasSharpness`, `HasSaturation`, `HasHue`, `HasGamma`, `HasNoiseReduction2D`, `HasAutoSlowShutter`, `HasExposureMode`, `HasVersionInquiry`, `HasFocusInfinity`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasDirectShutter`, `HasPowerStandby` |
| `SonyBrc300` | `HasDirectZoom`, `HasDigitalZoomToggle`, `HasDigitalZoomRange`, `HasIris`, `HasOnePushFocus`, `HasBacklightCompensation`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasRgbGain`, `HasImageFlip`, `HasAutoSlowShutter`, `HasExposureMode`, `HasVersionInquiry`, `HasFocusInfinity`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasDirectShutter` |
| `NearusBrc300` | `HasDirectZoom`, `HasDigitalZoomToggle`, `HasDigitalZoomRange`, `HasIris`, `HasOnePushFocus`, `HasBacklightCompensation`, `HasExposureCompensation`, `HasBrightness`, `HasOnePushWhiteBalance`, `HasRgbGain`, `HasImageFlip`, `HasAutoSlowShutter`, `HasExposureMode`, `HasVersionInquiry`, `HasFocusInfinity`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasDirectShutter` |
| `GenericVisca` | `HasIris`, `HasOnePushFocus`, `HasOnePushWhiteBalance`, `HasExposureMode`, `HasVersionInquiry`, `HasFocusInfinity`, `HasPresetMemory`, `HasShutter`, `HasGain`, `HasDirectShutter` |
<!-- END GENERATED PROFILE MARKERS -->

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

Sending a zone is gated per value, following the `ExposureCaps::modes` precedent.
`FocusCaps::zones` lists the values a profile may send; it is
empty exactly when focus-zone selection is unsupported and must be
duplicate-free. `command::focus::SetZone` refuses a value outside the list with
`Error::InvalidParameter { parameter: "focus_zone", .. }` before any I/O, on
both compile-time and runtime-selected paths.

The table below is generated from the registry. `Zone03` is
admitted for `PtzOpticsG2` and `PtzOptics30x` from bench evidence; G3 was not
bench-tested.

<!-- BEGIN GENERATED FOCUS ZONES -->
| Profile | `focus_zones` |
| ------- | ------------- |
| `PtzOpticsG2` | `Top`, `Center`, `Bottom`, `Zone03` |
| `PtzOpticsG3` | `Top`, `Center`, `Bottom` |
| `PtzOptics30x` | `Top`, `Center`, `Bottom`, `Zone03` |
| `SonyFr7` | none |
| `SonyBrcH900` | none |
| `SonyEviH100` | none |
| `SonyBrc300` | none |
| `NearusBrc300` | none |
| `GenericVisca` | none |
<!-- END GENERATED FOCUS ZONES -->

Decoding is not gated: the focus-zone inquiry decodes `03` from any camera,
because reading a reply sends nothing. A runtime profile may add `Zone03` to
its own list when its camera has evidence for it. A persisted `ProfileSpec`
must carry the field; one saved without it is refused on load with an error
that names the regeneration call. See
[VISCA reference §7.13](visca_reference.md) for the source search.

### Color-temperature inquiry (`HasColorTemperatureInquiry`)

`HasColorTemperature` gates the color-temperature reset, up, down and direct
setters. White-balance mode selection uses the domain's mode inventory. The `09 04 20` inquiry is gated separately, because
its reply layout is sourced only for the PTZOptics G2 family: the PTZOptics G2
bench (hardware inquiry test, #498) replies with one data byte, `90 50 pq FF`,
where `pq` is the `04 20` direct code ([VISCA reference §7.10](visca_reference.md)).
`PtzOpticsG2` and the legacy G2 `PtzOptics30x` implement
`HasColorTemperatureInquiry`. `PtzOpticsG3` retains the setters; BRC-H900 has no
shared color-temperature grant. Neither has a sourced reply for this inquiry, so
`camera.white_balance().color_temperature()` does not compile for them and the
runtime-selected path refuses it with `FeatureNotSupported` before any I/O; send
`raw::InquiryRequest` with `81 09 04 20 FF` to read the bytes.

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
does not compile for those profiles and the runtime-selected path refuses it with
`FeatureNotSupported` before any I/O; send `raw::InquiryRequest` with
`81 09 00 02 FF` to read the bytes.

A runtime profile grants the version inquiry by including `"version_inquiry"`
in `capabilities.typed_support`. Without it, `version()` fails with
`FeatureNotSupported` before I/O. A registry row's evidenced `VersionInquiry`
grant emits the matching static marker and runtime entry together. The
source-backed reply layout remains necessary for either representation.

The row-specific surfaces `HasImageFreeze`, `HasTallyBrightness`, and
`HasTallyMode` currently have no built-in profile grant. Image freeze
remains firmware/path-specific until its VISCA command path is validated;
extended tally brightness and PTZOptics tally extensions remain
custom/evidenced-profile surfaces. Defog inquiries have no typed built-in
model; a caller-owned request and decoder require model-specific evidence.
Sony FR7's source-backed red/green tally controls remain under `HasTally`.

R14 documents the current G2/G3 noise-reduction command inputs and inquiry
outputs separately. Its `04 50` input selects Auto (`02`) or Manual (`03`),
`04 53` accepts off (`00`) or levels `1–5`, and `04 54` accepts off (`00`) or
levels `1–8`; its `09 04 50`/`53`/`54` inquiry rows report the current mode and
levels `0–5`. Archived R1 is inquiry evidence only: it includes all three
inquiries and records a 3D result of `0–8`; it contains no setter rows. Because
the typed setter admits the full `0–8` public domain and both ranges are
source-backed, the decoder accepts every `NoiseReduction3DLevel` instead of
classifying levels `6–8` as malformed (#717).

Accordingly, `HasNoiseReduction2D` and `HasNoiseReduction3D` each gate their
level controls and inquiries. `HasNoiseReduction2DMode` gates the separate
`04 50` auto/manual mode control and inquiry. These three markers are emitted
for `PtzOpticsG2`, `PtzOpticsG3`, and `PtzOptics30x`. Sony EVI-H100 carries
only `HasNoiseReduction2D`: R8 documents `CAM_NR` `8x 01 04 53 0p FF`
(0 off, levels 1–5) and `CAM_NRInq` `8x 09 04 53 FF` → `y0 50 0p FF`,
which establish the 2D level control and inquiry, with no `04 50` mode or
`04 54` 3D level. `PtzOptics30x` names the
legacy PT30X SDI/NDI G2/Gen-2 profile explicitly listed by PTZOptics; it is not
a generic 30X grant and does not cover Move, Link, or newer 30X products. The
R14 portal describes its list as the full G2/G3 VISCA list, so a row R14 itself
lists (here the NR controls and inquiries) is G3 evidence; a row found only in
the archived Gen-2 tables (R1/U1) is not. No profile is admitted by group
membership alone.
Each 2D/3D permission is one source-backed grant shared by control and
inquiry. A camera without that command-family evidence has no grant; it cannot
advertise an unrelated metadata boolean as a substitute. Raw/custom requests remain the extension path for unsupported profiles and
aggregate vendor dialects.

The typed camera/session decoder and the public
`command::image::NoiseReduction3DInquiry` therefore share one
profile-neutral `0–8` domain. G2, G3, and 30X all decode a well-formed level
`6`, `7`, or `8`; no mutable profile identity or whole-`ProfileSpec` comparison
changes the numeric response domain. Profile markers still decide whether the
inquiry itself is available [R1, R14, R15].

The vendor state commands use their own support markers instead of inferring
permission from broad exposure, preset, or streaming metadata. PTZOptics G2,
G3, and 30X expose anti-flicker, settings save, preset-recall speed, multicast,
and NDI quality. Sony FR7 exposes the fixed `04 3A` spotlight controls; Sony
EVI-H100 (R8), BRC-300 (R12) and Nearus BRC-300 (R21) expose the
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
must still provide both before a runtime-selected call can encode.

Sony EVI-H100 (R8), BRC-300 (R12) and Nearus BRC-300 (R21) document the standard
exposure-compensation (`04 3E`/`04 0E`/`04 4E`), Bright (`04 0D`/`04 4D`) and
R/B gain (`04 03`/`04 04`, `04 43`/`04 44`) commands with their inquiries, so
all three carry `HasExposureCompensation`, `HasBrightness` and
`HasRgbGain`; BRC-300 and Nearus also document `CAM_ImgFlip` `04 66` with its
inquiry (`HasImageFlip`, no mirror). `BrightnessLevel` is the `0p 0q` wire byte
of `CAM_Bright` Direct, so each profile's `ExposureCaps::brightness_range` is the sole
admission authority: `0x00..=0x1F` without `0x01..=0x04` for EVI-H100 (R8), `0x00..=0x17` for BRC-300
and Nearus BRC-300 (R12, R21), and `0x00..=0x11` for the PTZOptics profiles.
EVI-H100's Wide-D is `CAM_WD` `04 3D`; the typed `WideDynamicRange` surface sends
the different `04 25` command, so EVI-H100 reports the WDR metadata but withholds
the typed surface.

Shutter codes are per-model tables, and a code means a different exposure time
on each table, so each profile carries its own source's table: EVI-H100 `00`
(1/1 s) through `15` (1/10000 s) from R8's 60/30 mode column, and BRC-300 and
Nearus BRC-300 `02` (1/4 s) through `15` from R12/R21. Generic VISCA uses the
codes all three share (`02` through `15`). No checked shutter-code inventory has been transcribed for FR7 or BRC-H900,
so those profiles advertise no shutter codes and refuse typed shutter
positions; the raw command path remains.

#### FR7 pan/tilt installation facts

Sony R7 pp. 12, 18 and 22 specify signed twenty-bit pan and tilt words, with
235.9 units/degree on each axis. Positive R7 pan is counterclockwise, so the
library's positive rightward angle uses a negative pan scale. Desktop tilt
uses −30° to +195°; Ceiling uses −210° to +15°. Configure the matching mount
with `SessionConfig::with_mount(target, MountPosition::Ceiling)` before opening
the session. `camera.registration().tilt_range()` reports the effective target
limits; `camera.profile()` remains the canonical source inventory.

The nominal degree endpoints and integer words differ by up to half a unit.
The FR7 converter snaps that endpoint cell to R7's exact word and uses nearest
rounding with halves away from zero in the interior. This is specified packet
support; FR7 movement and polling still need the physical verification recorded
in the hardware release checklist.

## Known unverified facts

- **PTZOptics raw focus orientation.** R1 and the current primary Focus and
  Inquiries pages establish the four-nibble position register but no relation
  between numerical endpoints and the nearest/farthest physical focus. The
  profiles declare `FocusNearEnd::Unknown`. Raw position control remains
  available; normalized near-to-far conversion reports an unsupported feature
  until the orientation is established. The current portal was checked on
  2026-11-15 at `resources/visca-api/focus/` and `resources/visca-api/inquiries/`;
  its separate focus-limit distance selectors do not establish raw direction.

- **BRC-H900 source reference spans (R11 v2.20, p. 37).** The checked
  pan/tilt table labels pan ±169° and records signed raw bounds ±`09BDE`.
  At 236 units/degree those raw endpoints give exact derived bounds of about
  ±169.076°; `EndpointQuantization::Exact` retains them. Tilt spans −30°…+90°
  (`E458`…`52F8`), and increasing pan is left. The focus
  reference table spans `1000` (infinity) through `C000` (0.8 m), with the
  high endpoint nearest. These are the conservative supported reference spans,
  not a claim of measured mechanical stops. Optical zoom is `0000`…`4000`
  (14×); presets are `00`…`0F` (pp. 18, 37, 39). Hardware behavior is unverified.
- **BRC-H900 ambiguous/missing setters.** R11 v2.20 p. 18 prints preset Set
  with selector `00` and Reset with `01`, opposite the other Sony sources.
  The crate withholds these two destructive actions until an erratum or
  hardware result resolves the labels. Its menu row establishes only Off,
  and the extracted spotlight/backlight command rows establish On only;
  inquiry reply polarity is not evidence for an absent setter. Incomplete
  families remain available through explicit caller-owned custom requests.

- **FR7 shutter codes (R7).** No checked R7 shutter-code inventory has
  been transcribed, so `SonyFr7` advertises no shutter codes
  (`ExposureCaps::shutter_speeds` is empty) and `ShutterSpeed` positions are refused before any I/O.
- **BRC-H900 shutter codes (R11).** No checked R11 shutter-code inventory has
  been transcribed, so `SonyBrcH900` advertises no shutter codes either.
  Its sourced reset/step commands and inquiry use `HasShutter`; direct
  positioning requires `HasDirectShutter`, which is withheld until those codes
  have positive evidence.
- **Pan/tilt direction of PTZOptics and BRC-H900.** Degrees are positive
  right and positive up for every profile. R8 (EVI-H100) documents increasing
  raw pan as right and increasing raw tilt as up, and R12/R21 (BRC-300, Nearus)
  document increasing raw pan as left and increasing raw tilt as up. The
  PTZOptics sources give degree ranges but no raw direction, so those profiles
  follow the R8 convention. R11 v2.20 now establishes BRC-H900 pan increasing
  left and tilt increasing up; those source-backed signs are used. Only PTZOptics G2 tilt has been
  verified on hardware (below); pan direction, `PtzOpticsG3` and BRC-H900
  remain unverified. `tests/pan_tilt_polarity.rs` pins the encoded direction of
  +45° for every built-in profile.

The FR7 and BRC-H900 shutter entries are gaps in the evidence, not statements
that the cameras lack shutter control. Send the shutter command as a
`raw::PlainRequest` request until a source-backed table is recorded in
[VISCA reference](visca_reference.md) and added to the profile.

### Facts verified on the PTZOptics G2 bench

These were verified on 2026-10-07 on PT30X-NDI G2, PT20X-NDI G2 and
PT12X-NDI G2 cameras; models, firmware and run evidence are in the
[hardware release checklist](hardware_release_checklist.md) (row HW-05).

- **PTZOptics G2-family shutter codes.** The PTZOptics sources document only
  `pq = Shutter Position`, not a code-to-time table, so `PtzOpticsG2`,
  `PtzOpticsG3` and `PtzOptics30x` ship the table `01` (1/30 s) through `11`
  (1/10000 s). `hw05_shutter_round_trip` (`tests/hardware_wire_rows_test.rs`)
  set codes `01`, `06`, `09` and `11` in shutter-priority mode and read each
  back unchanged on all five cameras, with `PtzOptics30x` on the PT30X-NDI G2
  and `PtzOpticsG2` on the others. The bench did not measure exposure time, so
  the fraction attached to each code is still the shipped table's.
- **PTZOptics G2 tilt polarity.** `hw05_tilt_polarity` passed on all five
  cameras: a typed tilt UP increased raw tilt and tilt degrees, DOWN decreased
  them, and pan did not change. A separate visible ~9° UP move was confirmed
  physically up on every camera by the operator.

Sony EVI-H100 (R8) also documents `CAM_ColorGain` `8x 01 04 49 00 00 00 0p FF`
and `CAM_ColorHue` `8x 01 04 4F 00 00 00 0p FF` (`0h`..`Eh`) with their
`09 04 49`/`09 04 4F` inquiries, byte for byte the typed saturation and hue
controls, so it carries `HasSaturation` and `HasHue`. R8's
`CAM_Aperture` (`04 02`/`04 42`, level `00`..`0F`, and `09 04 42` inquiry)
supports `HasSharpness`. Sharpness auto/manual mode uses the separate
`HasSharpnessMode` marker, granted only to the PTZOptics profiles: R8 and FR7's
R7 do not list the shared `04 05`/`09 04 05` mode family. FR7 detail enablement
uses its own `HasDetail` family. EVI-H100 picture-effect metadata remains
available, but its typed surface is withheld: R8's inquiry reports Off as `00`
and Neg.Art as `02`, which the shared typed inquiry would decode as Off.

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
on manual p. 14. Runtime profiles must therefore provide the image domain, backlight fact and
typed grant together.

An iris range in runtime metadata is a discovery fact, not a typed-support
grant. `IrisLevel` covers the built-in wire-domain union (`0x00..=0x1E`) and
`GainLevel` covers the union of built-in gain codes (`0x00..=0x19`, including
BRC-H900); profile ranges make
the model-specific admission decision for either value. `HasIris`
covers standard iris reset/up/down/direct control and the `09 04 4B` position
inquiry. It is enabled for the same eight built-ins:
the three PTZOptics profiles, BRC-H900, EVI-H100, BRC-300, Nearus BRC-300, and
Generic VISCA. R10/R14, R11, R8, R12 and R21 establish the model-specific rows.
Iris and bright positions are `CapabilityDomain` values: a table's bounds minus
the positions it does not list, checked during request preparation (a gap is `Error::InvalidParameter`, a
value past the bounds `Error::ParameterOutOfRange`). EVI-H100's iris table (R8
p. 44) lists `00` CLOSE and `05`..`11` and its Bright table (p. 45) `00` and
`05`..`1F`, so both domains exclude `01`..`04`. BRC-300 and Nearus BRC-300 list
every iris position `00`..`11` (R12, R21). Generic VISCA admits the iris
positions all three list: `00` and `05`..`11`. The PTZOptics sources (R1, R14)
give no iris or bright tables, so those profiles keep their contiguous ranges.
The unsourced auto/manual status inquiry is removed from the typed surface.
Caller-owned requests and decoders can model an extension once its packet and
reply are established for that camera. The FR7 command list
documents a vendor-relative `7E 04 4B` Up/Down family and `05 34` Auto Iris
control and inquiry. `HasIrisSteps` exposes `exposure().iris_up_by(...)` and
`iris_down_by(...)`; `HasAutoIris` exposes `set_auto_iris(...)` and
`auto_iris()`. The step commands are applied-only operations on Iris; the
auto-iris inquiry reports a mode and supplies no position or settlement probe.
R7 does not establish the shared `04 39` exposure-mode or `09 04 4B` iris-position
families.

R7's recording, audio level and input level, assignable buttons, multi-function
dial and multi selector, colour bars, lens initialization and shooting-mode
families remain outside the typed camera-control domains. Callers can use the
`raw::*` request types or implement their own `Request` for those established
packets. PTZ trace and PTZ auto framing also remain raw/custom extensions:
they start autonomous motion and require an owner motion-safety design covering
halt fencing and motion tracking before they can become typed operations.

Raw/custom request APIs are different. They remain available as escape hatches
for experiments, unsupported firmware variants, and downstream integrations.
Raw request availability must not be used to justify a built-in typed support
marker. Implement `Request` for the wire encoding and semantic class; typed
custom inquiries additionally implement `Inquiry` and return a
`ResponseDecoder` for their response type. Keep routing and profile validation
explicit in those typed request implementations.


## Grouped runtime facts

`Capabilities` owns optional domain records. `None` means the domain is absent;
its bounds, speeds and refinements cannot survive independently. Use validated
`CapabilityRange` values, direction-free `FocusCaps::range` plus a sourced or
`Unknown` `near_end`, and an optional shared gain range for cameras with a
different vendor gain family.
RGB gain and tuning each require both channels in one value.

`FocusCaps::near()` and `far()` return `None` when orientation is unknown.
The PTZOptics profiles retain their validated raw focus positions, while
physical normalized conversion reports unsupported orientation. Sony sources
establish their near endpoint at the numerical maximum. Fixed-lens optical
ratios are separate source facts: for example BRC-H900 has a 14× optical scale;
family profiles that do not identify one installed lens use
`Capabilities::zoom_scale_for_lens`.

Coordinates belong to `PanTiltCaps`; the target registration only selects a
source-declared tilt range. Degree bounds derive from wire bounds and the
conversion. `EndpointQuantization::Exact` uses exact degree bounds;
`WholeDegrees` expresses a source's whole-degree endpoint cells, validates that
each nominal endpoint is within one wire unit, and clamps that final rounding
cell. Values outside the nominal degree range are always refused. Relative
moves use the selected source range's full span and the same conversion rule.

`ProfileSpec::position_inquiries()` returns `Option<types::AffectedAxes>` as
the per-axis settlement matrix; `None` means no position inquiries. Each axis
uses its public inquiry's canonical gate: IRIS polling requires the `Iris`
grant, and ND-filter polling requires `NdFilter`. Validation checks this same
axis-to-inquiry declaration for position inquiries and preset recall axes.
`PresetCaps::recall_axes` describes exact recall effects; completionless
profiles need every corresponding settlement inquiry.

Profile defaults use the validated `IndividualCameraId` and cannot select
broadcast. `SessionConfig::new` registers that proven default directly. Every
optional persisted profile fact remains required (explicit `null` represents absence),
and every record refuses unknown keys; regenerate specs with the current shape.
