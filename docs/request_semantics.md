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
`raw::AppliedOnlyRequest`. Plain commands use `request::CommandPolicy`, operations
use `request::OperationPolicy`, and inquiries use `request::InquiryPolicy`. Each
class has one primary `new` constructor and one `with_policy` constructor where
a prebuilt policy is useful. Inquiry decoders use `from_fn` or `with_context`,
and operation values expose `bytes()` and `affected_axes()`.

The sealed `Request::Class` selects the same policy type for custom `Request`
implementations. A plain request's `policy()` returns `CommandPolicy`; an
operation returns `OperationPolicy`; an inquiry returns `InquiryPolicy`.
Command and operation policies take `profiles::CommandTimeoutClass`, whose five
variants exclude inquiry timing, and `request::CommandRetryClass::{Never,
Standard, Movement, Preset}`. An inquiry's timeout and reply protocol are fixed
by its class, so its policy stores neither a timeout nor a command reply shape.
Its retry family is `request::InquiryRetryClass::{Never, Inquiry}`: `Never`
disables automatic replay, and `Inquiry` enables bounded lost-reply recovery.
Command retry families cannot be constructed or deserialized into an inquiry
policy, and the inquiry retry family cannot be used on commands or operations.
All policies take `SubmissionClass`, which has no urgent lane. These restrictions
are checked by the policy types before submission. Policy construction is
infallible. Raw constructors and target-bound encoding validate wire bytes
against the declared class. The standard command category (`0x01`) requires a
command or operation class, and the standard inquiry category (`0x09`) requires
the inquiry class. A conflict returns `Error::InvalidRequest` before admission.
Other categories, including vendor extensions, use the caller’s declared class
and its policy.

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
`PayloadShape::Bytes(n)` admits exactly `n` decoded data bytes.
`PayloadShape::Nibbles(n)` also requires every byte to be at most `0x0F`.
Both variants take `NonZeroUsize` and exclude the VISCA address, reply header
and terminator from the count. On a raw transport, `90 50 12 FF 90 41 FF`
contains an inquiry reply with one data byte and a separate ACK. The first
`FF` ends the reply even with a declared length of two bytes. Sony IP reads
the frame length from the envelope, so a VISCA data byte within that frame
can be `FF`. Use `with_payload_shape` to declare the shape a
raw inquiry's decoder accepts, for example
`with_payload_shape(PayloadShape::Bytes(NonZeroUsize::MIN))` for one byte.
The raw owner rejects an exact count greater than `recv_buffer_size - 3`
with `Error::InvalidParameter` before it admits or writes the request.
Custom inquiries default to `Unknown`, which overlaps every shape. Known
shapes of equal length overlap, and different lengths can run independently.
Decoder value, padding and profile-range checks still apply.

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
| `DynamicRange` | Plain | Set `ExposureWideDynamicRange` |
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
| `ImageFlipOff` | Plain | — |
| `ImageFlipHorizontal` | Plain | — |
| `ImageFlipHorizontalOff` | Plain | — |
| `ImageFlipVertical` | Plain | — |
| `ImageFlipBoth` | Plain | — |
| `ImageFlipCombined` | Plain | — |
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
| `AddressSet` | OwnerOnly | — |
| `InterfaceClear` | OwnerOnly | — |
| `CommandCancel` | OwnerOnly | — |
| `SettingsSave` | Plain | — |
| `MotionSyncMode` | Plain | Set `MotionSyncEnabled` |
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

The ledger classifies Address Set, I/F Clear and Command Cancel as `OwnerOnly`.
These rows generate no `Request` or noun method. The serial handshake issues
Address Set and I/F Clear while it owns the bus exclusively. The session owner
correlates an operation to its camera-assigned socket before sending a cancel.

Raw construction requires an individual address (`0x81..=0x87`), a message
body, and the `0xff` terminator. It refuses broadcast frames, socket cancel
(`8x 2y ff`), and per-camera I/F Clear (`8x 01 00 01 ff`). Preparation checks
the same grammar for every request and verifies the selected target address.

### Raw reply shape

A raw command declares the reply protocol the camera will use, so the owner does
not assume every command follows the ACK-then-completion shape. The axis is
`raw::RawReplyShape`, carried on `request::CommandPolicy` and set with
`CommandPolicy::with_reply_shape`. Operations use `request::OperationPolicy` and
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
`request::InquiryPolicy` has no command reply-shape field. The inquiry class
requires this policy type on downstream `Request` implementations too, so no
command reply shape can be declared. Unconfirmed completion, quarantine, and
the strict opt-in follow the canonical
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

`AppliedStateEffectRequirement` has two verbs, `Set` and `Clear`.
Preparation attaches the typed effect from the command's
[`BuiltinStateProjection`](../src/request/builtin_contract.rs). The owner
applies a typed `KnownState` only after exact application, independent of
observer attachment. Its variant identifies the cache key.

- `Set` replaces a known state value supplied by the request. A pan/tilt limit
  records the corner and the applied `PanTiltPosition` decoded from the units
  written to the camera.
- `Clear` establishes a known absence: a pan/tilt limit clear becomes
  `KnownState::PanTiltLimit(PanTiltLimitUpdate::clear(corner))`. It is retained
  as `StateEntry::Set`, so the corner and its known removal remain observable.
  A deterministic boolean `Off` is likewise `Set` with `false`.

A write with any state requirement that may have taken effect but terminates
without confirmation invalidates its key. Never-written and conclusively
rejected attempts preserve previous knowledge. The owner commits either
change before resolving the request's terminal outcome.

The scheduler orders pan/tilt speed-mode setters by admission on each target.
On profiles that require `Fine50` for speed codes above 24, an extended-speed
write waits for earlier setters and for any setter already dispatched on that
target. Immediately before the initial write or a retry, the owner checks the
confirmed mode. Standard-speed commands and STOPs follow their usual scheduling
rules.

One `state_key_ledger!` row defines each key, its native value type, its
`KnownState` variant and its typed cache accessor. The fixed table retains one
slot per key without map allocation or eviction.

Current write-only state keys are:

<!-- BEGIN WRITE-ONLY STATE KEYS -->
`PanTiltLimit`, `PresetsRecallSpeed`, `FocusLock`, `ExposureSpotlight`, `ExposureAutoSlowShutter`, `NdFilterMode`, `NdFilterAuto`, `ImageFreeze`, `ZoomDigitalZoom`, `StreamingMulticast`, `StreamingNdiQuality`, `TallyBrightness`, `PanTiltSpeedMode`, `TallyMode`, `MotionSyncSpeed`, `ExposureWideDynamicRange`, `MotionSyncEnabled`.
<!-- END WRITE-ONLY STATE KEYS -->

Use the typed inquiries for USB-audio status and combined image orientation.
A future write-only property must add a key and explicit ledger rows before it
can be exposed by a typed facade.

## Extending built-in requests

Start with the [command ledger](../src/command/semantics.rs) to declare the
request class, affected axes and any applied-state requirement. Follow
preparation and its tests in [`prepared.rs`](../src/prepared.rs) for the selected
class.

- Iris and ND-filter setters declare their individual axes in the command
  ledger. [`settlement_inquiries!`](../src/position_inquiries.rs) maps each axis
  to its typed position inquiry. Preparation uses this table and the target's
  inquiry support to build the position query plan.
- Preset recall gets its affected axes from
  [`ProfileSpec::preset_recall_axes`](../src/profile.rs). Those same axes feed
  motion ordering and settlement preparation.
- A write-only setter such as a pan/tilt limit update projects its typed
  value through [`BuiltinStateProjection`](../src/request/builtin_contract.rs).
  The [owner](../src/runtime/owner/mod.rs) commits the prepared effect to
  the target's cache when it publishes the terminal outcome.
