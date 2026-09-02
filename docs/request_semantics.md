# Built-in request semantics

`command::semantics::BuiltinCommand` is the authoritative ledger.  A wire
opcode, timeout category, or command module is not allowed to infer a request
class.  Each enum variant has an exhaustive `classification()` arm; adding a
future built-in variant without a reviewed arm fails to compile.

The rule is intentionally narrow:

1. A request is an operation only when it starts, stops, or retargets physical
   actuation and exact VISCA socket cancellation has useful lifecycle meaning.
2. A targeted operation has a meaningful target state and a profile-selected
   protocol settlement condition.
3. An applied-only operation has actuation but no meaningful target state.
4. Configuration, mode selection, and stored-state edits are plain commands.

## Typed raw escape hatches

Custom wire frames use the single canonical raw namespace. The four request
classes are `raw::Plain`, `raw::Inquiry`, `raw::Targeted`, and
`raw::AppliedOnly`; their explicit policy values use `raw::Policy`. Each class
has one primary `new` constructor and one
`with_policy` constructor where a prebuilt policy is useful. Inquiry decoders
use `from_fn` or `with_context`, and request values expose `bytes()` and their
semantic accessors (`route()` or `affected_axes()`).

The raw namespace is the explicit escape hatch for callers that need to supply
wire bytes or a custom response decoder. The low-level `command` module remains
available to the crate's protocol implementation, but wire encoding alone does
not choose a completion class; owner submission uses the typed
`Request`/`Inquiry`/`OperationCommand` contracts.

Raw frames are bounded by the single public `raw::MAX_BYTES` constant; the
inline storage capacity is an implementation detail. Custom inquiry routes
use `InquiryRoute::try_custom`, and affected-axis cardinality is reported by
`AffectedAxes::axis_count`.

Every operation row contains a non-empty `BuiltinAxisSelection`.  Preset
recall is the one profile-dependent case: preparation must validate and carry
the profile's exact preset axes.  It must not substitute all axes.

## Ledger decisions

| Domain | Built-in rows | Class and exact axes | Applied-state requirement |
| --- | --- | --- | --- |
| Pan/tilt | `PanTiltHome`, `PanTiltReset`, `PanTiltAbsolute`, `PanTiltRelative` | Targeted — `PanTilt` | — |
| Pan/tilt | `PanTiltDrive`, `PanTiltStop` | AppliedOnly — `PanTilt` | — |
| Pan/tilt | `PanTiltLimitSet`, `PanTiltLimitClear` | Plain | Set/Clear `PanTiltLimits` |
| Zoom | `ZoomPosition` | Targeted — `Zoom` | — |
| Zoom | `ZoomStop`, `ZoomTele`, `ZoomWide`, `ZoomTeleVariable`, `ZoomWideVariable` | AppliedOnly — `Zoom` | — |
| Zoom | `DigitalZoom` | Plain | Set `DigitalZoomMode` |
| Focus | `FocusPosition`, `FocusInfinity` | Targeted — `Focus` | — |
| Focus | `FocusStop`, `FocusFar`, `FocusNear`, `FocusFarVariable`, `FocusNearVariable`, `FocusOnePush`, `FocusSnap` | AppliedOnly — `Focus` | — |
| Focus | `PushAfPress`, `PushAfRelease` | AppliedOnly — `Focus` | — |
| Focus | `FocusAuto`, `FocusManual`, `FocusToggle`, `FocusZone`, `FocusAutoSensitivity`, `FocusNearLimit` | Plain | — |
| Focus state | `FocusLock` | Plain | Set `FocusLockMode` |
| Presets | `PresetRecall` | Targeted — profile-selected exact axes | — |
| Presets | `PresetRecallSpeed` | Plain | Set `PresetRecallSpeed` |
| Presets | `PresetSet`, `PresetReset` | Plain | — |
| Power | `PowerOn`, `PowerStandby` | Plain | — |
| Exposure | `ExposureMode`, all `ExposureCompensation*`, `DynamicRange`, `Shutter*`, `Brightness*`, `AntiFlicker` | Plain | — |
| Iris | `IrisReset`, `IrisUp`, `IrisDown`, `IrisDirect` | Targeted — `Iris` | — |
| Exposure state | `SpotlightOn`, `SpotlightOff` | Plain | Set `Spotlight` |
| Exposure state | `AutoSlowShutterOn`, `AutoSlowShutterOff` | Plain | Set `AutoSlowShutter` |
| Gain | `GainReset`, `GainUp`, `GainDown`, `GainDirect`, `GainLimit` | Plain | — |
| White balance | all `WhiteBalance*`, `AutoWhiteBalanceSensitivity`, `OnePushWhiteBalanceTrigger` | Plain | — |
| Color | `RedTuning`, `BlueTuning`, `Saturation`, `Hue`, all `ColorTemperature*`, `RedGain*`, `BlueGain*` | Plain | — |
| Image | all `Sharpness*`, `Luminance`, `Contrast`, `Gamma`, `Backlight`, `PictureEffect` | Plain | — |
| Image state | `ImageFlipBoth`, `ImageFlipCombined` | Plain | Set `Flip` |
| Image state | `ImageFlipOff`, `ImageFlipHorizontal`, `ImageFlipHorizontalOff`, `ImageFlipVertical` | Plain | Invalidate `Flip` |
| Image state | `ImageFreezeOn`, `ImageFreezeOff` | Plain | Set `ImageFreeze` |
| ND filter | `NdFilterDirect`, `NdFilterStepUp`, `NdFilterStepDown` | Targeted — `NdFilter` | — |
| ND filter | `NdFilterMode` | Plain | Set `NdFilterMode` |
| ND filter | `NdFilterAutoOn`, `NdFilterAutoOff` | Plain | Set `AutoNdFilter` |
| Tally | `TallyRedOn`, `TallyRedOff`, `TallyGreenOn`, `TallyGreenOff` | Plain | — (status inquiries exist) |
| Tally | `TallyBrightLow`, `TallyBrightHigh` | Plain | Set `TallyBrightness` |
| Tally | `TallyFlash` | Plain | Invalidate `TallyMode` |
| Tally | `TallyOn`, `TallyOff` | Plain | Set `TallyMode` |
| Menu | `MenuDisplay`, `MenuNavigate`, `MenuSelect`, `MenuCancel`, `DirectMenu` | Plain | — |
| Streaming | `MulticastStreamingOn`, `MulticastStreamingOff`, `NdiQuality` | Plain | Set corresponding state |
| Streaming | `UsbAudioOn`, `UsbAudioOff` | Plain | — (USB audio inquiry exists) |
| System | `SettingsSave` | Plain | — |
| Serial setup/recovery | `AddressSet`, `InterfaceClear` wire commands | Owner-only transport handshake | — |
| Cancellation | socket-specific `CommandCancel` wire command | Owner-only operation lifecycle | — |
| Motion configuration | `MotionSyncMode`, `MotionSyncPreset`, `VariableSpeedMode` | Plain | Set `VariableSpeedMode` for the latter |

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
`raw::RawReplyShape`, carried on `raw::Policy` and set with
`Policy::with_reply_shape`:

- `AckThenCompletion` (the default) — the command is acknowledged, assigned a
  socket, then completes. This is the shape of every built-in command, so
  existing raw code is unchanged.
- `CompletionOnly` — the camera answers with a completion (or terminal) frame
  and no acknowledgement. The command earns no socket and never enters the
  unacknowledged-ACK gate, so a missing ACK can neither fail nor poison it; it
  terminates on the completion frame or a bounded completion deadline. Because it
  can never be socket-correlated, a completion-only command holds the target's
  command channel exclusively for its lifetime: nothing else dispatches to the
  target while it is in flight, and it does not start until the target is idle.
  Its successful terminal leaves a bounded target response/correlation
  quarantine before same-target response-bearing raw command or inquiry work
  may start, so a duplicate completion or error cannot bind to that successor.
- `NoReply` — a plain-command-only fire-and-forget shape. Its `execute()`
  result proves only that the local transport write succeeded; it does **not**
  prove camera acceptance or protocol application, so raw targeted and
  applied-only operations reject this shape. To make that later quarantine
  attributable, its write starts only when the target has no live command or
  inquiry and excludes same-target work until the write result lands. A
  successful write leaves the same bounded target response/correlation
  quarantine before same-target
  response-bearing raw command or inquiry work may start. Another `NoReply`
  may write during that hold because it consumes no response identity; a
  successful write extends the same fixed bound. Any delayed command response
  during that interval is ignored rather than guessed onto later work.

The shape is a command axis only. An inquiry always awaits its reply, so
`raw::Inquiry` and shared inquiry preparation reject any non-default shape rather
than silently ignoring it. Unconfirmed completion, quarantine, and the strict
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
mode selection, motion-sync and variable-speed setup, menu actions, settings
save, address/interface setup, and similar vendor controls) are plain. They
may change internal calibration or future command behavior, but they do not
carry a recoverable target state and their protocol completion is the only
stable lifecycle boundary. A later profile may add an inquiry or a typed
state value; that must add an explicit ledger state/effect rather than
silently promoting the existing row to an operation.

### Why the non-PTZ rows are not operations

Power transitions, one-push white balance, spotlight, auto slow shutter,
tally/flash, flip/mirror, menu navigation, streaming, motion-sync mode,
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

### Applied-state effects

`AppliedStateEffectRequirement` is closed to `Set`, `Clear`, and `Invalidate`.
The later typed conversion must attach the selected requirement to a prepared
write.  The owner applies the concrete value only after exact application,
independent of observer attachment:

- `Set` replaces a known state value supplied by the request.
- `Clear` removes a known state entry (for example a pan/tilt limit corner).
  A deterministic boolean `Off` command is still `Set` with `false`, so the
  owner retains the fact that the disabled value was applied.
- `Invalidate` marks a state unknown when the command's result cannot be
  represented exactly (for example vendor tally flash/on/off behavior).

Current write-only state keys are `PanTiltLimits`, `PresetRecallSpeed`,
`FocusLockMode`, `Spotlight`, `AutoSlowShutter`, `NdFilterMode`,
`AutoNdFilter`, `ImageFreeze`, `DigitalZoomMode`, `MulticastStreaming`,
`NdiQuality`, `TallyBrightness`, `VariableSpeedMode`, `TallyMode`, and `Flip`.
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
