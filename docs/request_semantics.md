# Built-in request semantics

The private `BuiltinCommand` ledger is authoritative for implementation review.  A wire
opcode, timeout category, or command module is not allowed to infer a request
class.  One `builtin_command_ledger!` list in `src/command/semantics.rs` names
every built-in command together with its classification; the enum, its `ALL`
inventory and `classification()` are expanded from that list, so a command
cannot exist without a reviewed classification.

The rule is intentionally narrow:

1. A request is an operation only when it starts, stops, or retargets physical
   actuation and exact VISCA socket cancellation has useful lifecycle meaning.
2. A targeted operation has a meaningful target state and a profile-selected
   protocol settlement condition.
3. An applied-only operation has actuation but no meaningful target state.
4. Configuration, mode selection, and stored-state edits are plain commands.

## Typed raw escape hatches

Custom wire frames use the single canonical raw namespace. The four request
classes are `raw::PlainRequest`, `raw::InquiryRequest`, `raw::TargetedRequest`, and
`raw::AppliedOnlyRequest`; plain commands use `raw::CommandPolicy`, operations use
`raw::OperationPolicy`, and inquiries use `raw::InquiryPolicy`. Each class
has one primary `new` constructor and one
`with_policy` constructor where a prebuilt policy is useful. Inquiry decoders
use `from_fn` or `with_context`, and operation values expose `bytes()` and
`affected_axes()`. All inquiry convenience constructors select the `Inquiry`
timeout category. Inquiry policies store neither timeout nor command reply
shape. Command and operation policies take `CommandTimeoutClass`, which has no
`Inquiry` variant. Inquiry policies and convenience constructors take
`request::InquiryRetryClass::{Never, Inquiry}`: `Never` disables automatic replay,
and `Inquiry` enables bounded lost-reply recovery. `Standard`, `Movement` and
`Preset` are command families and cannot be constructed or deserialized into an
inquiry policy. Preparation rejects those families on downstream inquiry
`Request` implementations before encoding or I/O. All policies take
`SubmissionClass`, which has no urgent lane.
Policy construction is infallible, and raw request construction validates only
wire bytes.

The raw namespace is the explicit escape hatch for callers that need to supply
wire bytes or a custom response decoder. Public built-in requests live under `command::<noun>::<Request>`, with
`<Property>Inquiry` for inquiries and setters mirroring the noun methods.
Wire encoding alone does not choose a completion class; owner submission uses the typed
`Request`/`Inquiry`/`OperationCommand` contracts.

Raw frames are bounded by the single public `raw::MAX_BYTES` constant; the
inline storage capacity is an implementation detail. Inquiry decoders own the
payload interpretation; a raw reply carries no caller-chosen route identifier.
Affected-axis cardinality is reported by `AffectedAxes::axis_count`.

An inquiry declares its structural reply evidence through `payload_shape()`.
`PayloadShape::Bytes(n)` admits exactly `n` decoded data bytes;
`Nibbles(n)` additionally requires every byte to be at most `0x0F`. The count
excludes the VISCA address, reply header and terminator. Raw inquiries can use
`with_payload_shape` to supply the shape their decoder accepts. Custom inquiries
default to `Unknown`, which overlaps every shape. Known shapes overlap by
length, pairwise: `Bytes(1)` and `Nibbles(4)` each overlap `Unknown` but do not
overlap each other. Decoder value, padding and profile-range checks still apply.

Built-in inquiries and canonical `ViscaInquiry` derives take their shape from
one column beside the decoder in the inquiry table. Pan/tilt position selects
the profile's same per-axis grammar for both decoding and shape: eight nibble
bytes for standard VISCA, nine for BRC-300 and ten for FR7. Explicit custom
parsers and raw derives keep `Unknown`, since their payload structure belongs
to the caller.

A built-in query with a parameter owns its validated argument and is constructed
with `new(argument)`. Both encoding paths append its one-byte protocol value to
the canonical fixed prefix. Generated Source metadata retains that prefix;
framing and decoder inventories construct the table's valid typed sample.

Every operation row contains a non-empty `BuiltinAxisSelection`.  Preset
recall is the one profile-dependent case: preparation must validate and carry
the profile's exact preset axes.  It must not substitute all axes.

## Color-temperature decoding

`ColorTemperatureInquiry.decoder()` and the profile-independent color-temperature
command use the declared PTZOptics step table. They describe that table's wire
dialect. Use `decoder_for_profile(&profile)` and target-bound preparation for a
custom table: the same wire index can select a different Kelvin value there.
Every conversion indexes a table; no arithmetic Kelvin-to-step mapping is used.

The profile-independent `InquiryData::ColorTemperature { temperature }` field
always carries a wire-table index. A profile-bound typed decoder returns `Kelvin`
directly and never stores physical temperature in that raw field. Derived raw
inquiries retain the index under both decoder entry points (#974).

## Ledger decisions

<!-- BEGIN LEDGER DECISIONS -->
| Built-in row | Class and exact axes | Applied-state requirement |
| --- | --- | --- |
| `PanTiltHome` | Targeted — `PanTilt` | — |
| `PanTiltReset` | Targeted — `PanTilt` | — |
| `PanTiltDrive` | AppliedOnly — `PanTilt` | — |
| `PanTiltStop` | AppliedOnly STOP — `PanTilt` | — |
| `PanTiltAbsolute` | Targeted — `PanTilt` | — |
| `PanTiltRelative` | Targeted — `PanTilt` | — |
| `PanTiltLimitSet` | Plain | Set `PanTiltLimit` |
| `PanTiltLimitClear` | Plain | Clear `PanTiltLimit` |
| `ZoomStop` | AppliedOnly STOP — `Zoom` | — |
| `ZoomTele` | AppliedOnly — `Zoom` | — |
| `ZoomWide` | AppliedOnly — `Zoom` | — |
| `ZoomTeleVariable` | AppliedOnly — `Zoom` | — |
| `ZoomWideVariable` | AppliedOnly — `Zoom` | — |
| `ZoomPosition` | Targeted — `Zoom` | — |
| `DigitalZoom` | Plain | Set `ZoomDigitalZoom` |
| `FocusStop` | AppliedOnly STOP — `Focus` | — |
| `FocusFar` | AppliedOnly — `Focus` | — |
| `FocusNear` | AppliedOnly — `Focus` | — |
| `FocusFarVariable` | AppliedOnly — `Focus` | — |
| `FocusNearVariable` | AppliedOnly — `Focus` | — |
| `FocusPosition` | Targeted — `Focus` | — |
| `FocusAuto` | Plain | — |
| `FocusManual` | Plain | — |
| `FocusOnePush` | AppliedOnly — `Focus` | — |
| `FocusInfinity` | Targeted — `Focus` | — |
| `FocusToggle` | Plain | — |
| `FocusSnap` | AppliedOnly — `Focus` | — |
| `FocusZone` | Plain | — |
| `FocusAutoSensitivity` | Plain | — |
| `FocusNearLimit` | Plain | — |
| `FocusLock` | Plain | Set `FocusLock` |
| `PushAfPress` | AppliedOnly — `Focus` | — |
| `PushAfRelease` | AppliedOnly — `Focus` | — |
| `PresetRecall` | Targeted — profile-selected exact axes | — |
| `PresetRecallSpeed` | Plain | Set `PresetsRecallSpeed` |
| `PresetSet` | Plain | — |
| `PresetReset` | Plain | — |
| `PowerOn` | Plain | — |
| `PowerStandby` | Plain | — |
| `ExposureMode` | Plain | — |
| `ExposureCompensationOn` | Plain | — |
| `ExposureCompensationOff` | Plain | — |
| `ExposureCompensationReset` | Plain | — |
| `ExposureCompensationUp` | Plain | — |
| `ExposureCompensationDown` | Plain | — |
| `ExposureCompensationDirect` | Plain | — |
| `DynamicRange` | Plain | — |
| `IrisReset` | Targeted — `Iris` | — |
| `IrisUp` | Targeted — `Iris` | — |
| `IrisDown` | Targeted — `Iris` | — |
| `IrisDirect` | Targeted — `Iris` | — |
| `ShutterReset` | Plain | — |
| `ShutterUp` | Plain | — |
| `ShutterDown` | Plain | — |
| `ShutterDirect` | Plain | — |
| `BrightnessReset` | Plain | — |
| `BrightnessUp` | Plain | — |
| `BrightnessDown` | Plain | — |
| `BrightnessSet` | Plain | — |
| `AntiFlicker` | Plain | — |
| `SpotlightOn` | Plain | Set `ExposureSpotlight` |
| `SpotlightOff` | Plain | Set `ExposureSpotlight` |
| `AutoSlowShutterOn` | Plain | Set `ExposureAutoSlowShutter` |
| `AutoSlowShutterOff` | Plain | Set `ExposureAutoSlowShutter` |
| `GainReset` | Plain | — |
| `GainUp` | Plain | — |
| `GainDown` | Plain | — |
| `GainDirect` | Plain | — |
| `GainLimit` | Plain | — |
| `WhiteBalanceAuto` | Plain | — |
| `WhiteBalanceIndoor` | Plain | — |
| `WhiteBalanceOutdoor` | Plain | — |
| `WhiteBalanceOnePush` | Plain | — |
| `WhiteBalanceAutoTracking` | Plain | — |
| `WhiteBalanceManual` | Plain | — |
| `WhiteBalanceColorTemperature` | Plain | — |
| `AutoWhiteBalanceSensitivity` | Plain | — |
| `OnePushWhiteBalanceTrigger` | Plain | — |
| `RedTuning` | Plain | — |
| `BlueTuning` | Plain | — |
| `Saturation` | Plain | — |
| `Hue` | Plain | — |
| `ColorTemperatureReset` | Plain | — |
| `ColorTemperatureUp` | Plain | — |
| `ColorTemperatureDown` | Plain | — |
| `ColorTemperatureDirect` | Plain | — |
| `RedGainReset` | Plain | — |
| `RedGainUp` | Plain | — |
| `RedGainDown` | Plain | — |
| `RedGainDirect` | Plain | — |
| `BlueGainReset` | Plain | — |
| `BlueGainUp` | Plain | — |
| `BlueGainDown` | Plain | — |
| `BlueGainDirect` | Plain | — |
| `SharpnessMode` | Plain | — |
| `SharpnessReset` | Plain | — |
| `SharpnessUp` | Plain | — |
| `SharpnessDown` | Plain | — |
| `SharpnessDirect` | Plain | — |
| `Luminance` | Plain | — |
| `Contrast` | Plain | — |
| `Gamma` | Plain | — |
| `Backlight` | Plain | — |
| `NoiseReduction2dMode` | Plain | — |
| `NoiseReduction2d` | Plain | — |
| `NoiseReduction2dOff` | Plain | — |
| `NoiseReduction3d` | Plain | — |
| `NoiseReduction3dOff` | Plain | — |
| `ImageFlipOff` | Plain | Invalidate `ImageFlipMode` |
| `ImageFlipHorizontal` | Plain | Invalidate `ImageFlipMode` |
| `ImageFlipHorizontalOff` | Plain | Invalidate `ImageFlipMode` |
| `ImageFlipVertical` | Plain | Invalidate `ImageFlipMode` |
| `ImageFlipBoth` | Plain | Set `ImageFlipMode` |
| `ImageFlipCombined` | Plain | Set `ImageFlipMode` |
| `ImageFreezeOn` | Plain | Set `ImageFreeze` |
| `ImageFreezeOff` | Plain | Set `ImageFreeze` |
| `PictureEffect` | Plain | — |
| `NdFilterMode` | Plain | Set `NdFilterMode` |
| `NdFilterDirect` | Targeted — `NdFilter` | — |
| `NdFilterStepUp` | Targeted — `NdFilter` | — |
| `NdFilterStepDown` | Targeted — `NdFilter` | — |
| `NdFilterAutoOn` | Plain | Set `NdFilterAuto` |
| `NdFilterAutoOff` | Plain | Set `NdFilterAuto` |
| `TallyRedOn` | Plain | — |
| `TallyRedOff` | Plain | — |
| `TallyBrightLow` | Plain | Set `TallyBrightness` |
| `TallyBrightHigh` | Plain | Set `TallyBrightness` |
| `TallyGreenOn` | Plain | — |
| `TallyGreenOff` | Plain | — |
| `TallyFlash` | Plain | Set `TallyMode` |
| `TallyOn` | Plain | Set `TallyMode` |
| `TallyOff` | Plain | Set `TallyMode` |
| `MenuDisplay` | Plain | — |
| `MenuNavigate` | AppliedOnly — `PanTilt` | — |
| `MenuSelect` | Plain | — |
| `MenuCancel` | Plain | — |
| `DirectMenu` | Plain | — |
| `MulticastStreamingOn` | Plain | Set `StreamingMulticast` |
| `MulticastStreamingOff` | Plain | Set `StreamingMulticast` |
| `NdiQuality` | Plain | Set `StreamingNdiQuality` |
| `UsbAudioOn` | Plain | — |
| `UsbAudioOff` | Plain | — |
| `AddressSet` | Plain | — |
| `InterfaceClear` | Plain | — |
| `CommandCancel` | Plain | — |
| `SettingsSave` | Plain | — |
| `MotionSyncMode` | Plain | — |
| `MotionSyncPreset` | Plain | Set `MotionSyncSpeed` |
| `VariableSpeedMode` | Plain | Set `PanTiltSpeedMode` |
| `Fr7AeLevelUp` | Plain | — |
| `Fr7AeLevelDown` | Plain | — |
| `Fr7AutoGain` | Plain | — |
| `Fr7AutoShutter` | Plain | — |
| `Fr7AutoIris` | Plain | — |
| `Fr7IrisUp` | AppliedOnly — `Iris` | — |
| `Fr7IrisDown` | AppliedOnly — `Iris` | — |
| `Fr7WhiteBalanceSet` | Plain | — |
| `Fr7PresetWhiteUp` | Plain | — |
| `Fr7PresetWhiteDown` | Plain | — |
| `Fr7PresetWhiteDirect` | Plain | — |
| `Fr7MemoryColorTemperatureUp` | Plain | — |
| `Fr7MemoryColorTemperatureDown` | Plain | — |
| `Fr7MemoryColorTemperatureDirect` | Plain | — |
| `Fr7MemoryRedGainUp` | Plain | — |
| `Fr7MemoryRedGainDown` | Plain | — |
| `Fr7MemoryRedGainDirect` | Plain | — |
| `Fr7MemoryBlueGainUp` | Plain | — |
| `Fr7MemoryBlueGainDown` | Plain | — |
| `Fr7MemoryBlueGainDirect` | Plain | — |
| `Fr7MemoryTintUp` | Plain | — |
| `Fr7MemoryTintDown` | Plain | — |
| `Fr7MemoryTintDirect` | Plain | — |
| `Fr7AtwColorTemperatureOffsetUp` | Plain | — |
| `Fr7AtwColorTemperatureOffsetDown` | Plain | — |
| `Fr7AtwColorTemperatureOffsetDirect` | Plain | — |
| `Fr7AtwTintOffsetUp` | Plain | — |
| `Fr7AtwTintOffsetDown` | Plain | — |
| `Fr7AtwTintOffsetDirect` | Plain | — |
| `Fr7MasterBlackUp` | Plain | — |
| `Fr7MasterBlackDown` | Plain | — |
| `Fr7MasterBlackDirect` | Plain | — |
| `Fr7RedBlackUp` | Plain | — |
| `Fr7RedBlackDown` | Plain | — |
| `Fr7RedBlackDirect` | Plain | — |
| `Fr7BlueBlackUp` | Plain | — |
| `Fr7BlueBlackDown` | Plain | — |
| `Fr7BlueBlackDirect` | Plain | — |
| `Fr7DetailSetting` | Plain | — |
| `Fr7KneeSetting` | Plain | — |
| `Fr7SetKneeMode` | Plain | — |
| `Fr7SetKneeSlope` | Plain | — |
| `Fr7SetKneePoint` | Plain | — |
| `Fr7ZoomHighResolutionTele` | AppliedOnly — `Zoom` | — |
| `Fr7ZoomHighResolutionWide` | AppliedOnly — `Zoom` | — |
| `Fr7ZoomHighResolutionStop` | AppliedOnly STOP — `Zoom` | — |
| `Fr7SetPresetSpeedSelection` | Plain | — |
| `Fr7SetCommonPresetSpeed` | Plain | — |
| `Fr7SetSeparatePresetMode` | Plain | — |
| `Fr7SetSeparatePresetSpeed` | Plain | — |
| `Fr7SetSeparatePresetDuration` | Plain | — |
| `WhiteBalancePreset` | Plain | — |
<!-- END LEDGER DECISIONS -->

The three owner-only rows are deliberately absent from
`request::builtin`. Address assignment and interface clear require exclusive
ownership of a serial bus before target registration; a target-scoped
`execute` call cannot represent that authority. Socket cancellation is emitted
only from an operation handle, after the owner has correlated that exact
operation to the camera-assigned socket; letting a caller construct a generic
socket cancel could cancel unrelated work. The raw escape hatch enforces this
too — its wire validation rejects the socket-cancel (`8x 2y ff`) and per-camera
interface-clear (`8x 01 00 01 ff`) shapes at construction, and the broadcast
address-set form is refused by the target-address check during preparation — so
no `execute`/`inquire`/`submit` path, typed or raw, can emit an owner-only
primitive.

### Raw reply shape

A raw command declares the reply protocol the camera will use, so the owner does
not assume every command follows the ACK-then-completion shape. The axis is
`raw::RawReplyShape`, carried on `raw::CommandPolicy` and set with
`CommandPolicy::with_reply_shape`. Operations use `raw::OperationPolicy` and
`raw::OperationReplyShape`, which omit `NoReply`:

- `AckThenCompletion` (the default) — the command is acknowledged, assigned a
  socket, then completes. This is the shape of every built-in command, so
  existing raw code is unchanged.
- `CompletionOnly` — the camera answers with a completion (or terminal) frame
  and no acknowledgement. The command earns no socket and never enters the
  unacknowledged-ACK gate, so a missing ACK can neither fail nor poison it; it
  terminates on the completion frame or a bounded completion deadline. Because it
  can never be socket-correlated, a completion-only command holds the target's
  command channel exclusively for ordinary work and starts only when its target
  is idle. An urgent STOP may cross that exclusion on a raw stream, where the
  ledger keeps its first answer distinct (#795). A successful raw datagram
  terminal leaves a bounded `AllResponses` hold before ordinary same-target
  response-bearing work starts; a raw stream accounts for the answer in its
  ledger instead. Cancellation after the write is refused with `CancellationNotSupported`
  because this command can never own a cancellable socket (#835).
- `NoReply` — a plain-command-only fire-and-forget shape. Its `execute()`
  result proves only that the local transport write succeeded; it does **not**
  prove camera acceptance or protocol application, so raw targeted and
  applied-only operations cannot select this shape. To make that later quarantine
  attributable, its write starts only when the target has no live command or
  inquiry and excludes same-target work until the write result lands. A
  successful write leaves the same bounded target response/correlation
  quarantine before same-target
  response-bearing raw command or inquiry work may start. Another `NoReply`
  may write during that hold because it consumes no response identity; a
  successful write extends the same fixed bound. Any delayed command response
  during that interval is ignored rather than guessed onto later work. A STOP
  is never held by it (#795): a socketless error it makes ambiguous binds to
  neither. On a raw byte stream its possible rejection stays owed until a
  later answer from the camera settles it, since a stall can outlast any
  window (see `docs/architecture_2_0.md`).

The shape is a command axis only. An inquiry always awaits its reply, so
`raw::InquiryPolicy` has no command reply-shape field. Shared inquiry
preparation still rejects a non-default shape from a downstream `Request`
implementation. Unconfirmed completion, quarantine, and the strict
opt-in follow the canonical
[raw recovery rule](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery);
this page does not define a second recovery policy.

Reply shape is for *legitimate* custom completion-only or fire-and-forget vendor
frames. It never re-admits the owner-only wire primitives above: a socket cancel
or interface clear stays rejected at construction whatever reply shape is
declared, because those act on another operation's socket or the shared command
buffer, not on the caller's own command.

### Domain rationale for settings and stored state

The exposure row is deliberately broad. Exposure mode and compensation select
how the camera's exposure controller behaves; they do not command a
cancelable lens or axis movement. Dynamic range, anti-flicker, spotlight, and
auto-slow-shutter likewise update an exposure policy or mode register. Their
application is observable as a completed command, but there is no exact
target state for protocol settlement or useful socket cancellation beyond that
application point.

Shutter, brightness, and gain are also plain even where a camera implements
them with electronic or lens hardware. The legacy VISCA commands select or
step a control register and do not expose a position inquiry/settlement plan
that can be used as an independently cancellable movement. `Iris` is the
explicit exception: its direct/step forms have a finite aperture target and an
exact built-in inquiry, so they are targeted on the `Iris` axis. Shutter and
brightness values are not inferred to be target states, and gain is an
electronic exposure value rather than a camera-axis operation.

Preset recall is targeted because a validated profile can provide the exact
axes moved by the stored preset and can settle those axes. Preset set/reset
remain plain: the camera owns an opaque bundle of position and configuration
values, and the command carries no recoverable value that `StateCache` could
write back. Consequently these commands intentionally have no
`AppliedStateEffectRequirement`; inventing a preset cache entry would claim
knowledge the protocol does not provide. The preset contents are outside the
write-only cache by design.

Calibration and configuration commands (one-push white balance, focus/ND
mode selection, motion-sync and variable-speed setup, menu display/selection, settings
save, address/interface setup, and similar vendor controls) are plain. They
may change internal calibration or future command behavior, but they do not
carry a recoverable target state and their protocol completion is the only
stable lifecycle boundary. A later profile may add an inquiry or a typed
state value; that must add an explicit ledger state/effect rather than
silently promoting the existing row to an operation.

### Why the non-PTZ rows are not operations

Power transitions, one-push white balance, spotlight, auto slow shutter,
tally/flash, flip/mirror, menu display/selection, streaming, motion-sync mode,
variable-speed mode, digital-zoom enablement, white-balance, and color/image
controls edit a mode, state register, or output policy.  They do not expose a
cancelable camera-axis movement with a useful exact protocol cancellation, so
they remain plain even when the camera changes hardware or video output while
applying the setting.

Iris and ND filter are different: each command physically repositions a lens
or filter, VISCA cancellation applies to the in-flight actuation, and the
result has a meaningful finite end state.  They are targeted on their own
axes.  Push-AF press/release starts/stops focus actuation but has no requested
focus target, so it is applied-only on `Focus`.

`MenuNavigate` is an exception to menu configuration: with the on-screen menu
closed, its wire command is a continuous physical pan/tilt drive. It is therefore
an applied-only `PAN_TILT` operation, participates in halt fences and motion
tracking, and returns a handle on every facade. Observe it with `applied()`;
end physical movement with a pan/tilt STOP or `stop_all()` (#839).

### Applied-state effects

`AppliedStateEffectRequirement` is closed to `Set`, `Clear`, and `Invalidate`.
The later typed conversion must attach the selected requirement to a prepared
write. The owner applies a typed `KnownState` only after exact application,
independent of observer attachment. Its variant implies the key; values are
never encoded as scalar arrays for the cache:

- `Set` replaces a known state value supplied by the request.
- `Clear` establishes a known absence: a pan/tilt limit clear becomes
  `KnownState::PanTiltLimits(PanTiltLimitUpdate::clear(corner))`. It is retained
  as `StateEntry::Set`, so the corner and its known removal remain observable.
  A deterministic boolean `Off` is likewise `Set` with `false`.
- `Invalidate` marks a state unknown when the command's result cannot be
  represented exactly (for example a single-axis image-flip command cannot establish the
  complete `ImageFlipMode` value).

A write with any state requirement that may have taken effect but terminates
without confirmation invalidates its key. Never-written and conclusively
rejected attempts preserve previous knowledge. The owner commits either
change before resolving the request's terminal outcome.

One `state_key_ledger!` row defines each key, its native value type, its
`KnownState` variant and its typed cache accessor. The fixed table retains one
slot per key without map allocation or eviction.

Current write-only state keys are:

<!-- BEGIN WRITE-ONLY STATE KEYS -->
`PanTiltLimit`, `PresetsRecallSpeed`, `FocusLock`, `ExposureSpotlight`, `ExposureAutoSlowShutter`, `NdFilterMode`, `NdFilterAuto`, `ImageFreeze`, `ZoomDigitalZoom`, `StreamingMulticast`, `StreamingNdiQuality`, `TallyBrightness`, `PanTiltSpeedMode`, `TallyMode`, `ImageFlipMode`, `MotionSyncSpeed`.
<!-- END WRITE-ONLY STATE KEYS -->

USB-audio is intentionally not in this list because the built-in inquiry
inventory has a typed USB-audio status response. A future write-only property
must add a key and explicit ledger rows before it can be exposed by a typed
facade.

## Conversion dependencies

This phase intentionally does not migrate camera facades or every command to
`request::builtin`.  Later conversion must:

- preserve these classes and axes in homogeneous request types;
- add exact profile inquiry facts and settlement plans for `Iris` and
  `NdFilter` before targeted requests can use position polling;
- lower profile-dependent preset axes without an all-axis fallback;
- lower the closed state-effect requirement into the target-local cache; and
- keep system cancellation and raw protocol escapes isolated from ordinary
  operation handles.
