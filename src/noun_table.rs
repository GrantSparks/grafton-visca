//! The typed noun registry behind the static surface metadata and all three
//! noun facades.
//!
//! Every noun method is written here once — name, arguments, capability
//! gate, return class, request and rustdoc — and everything else is expanded
//! from it:
//!
//! * the blocking and async accessors and methods
//!   ([`crate::blocking::blocking_nouns`] and [`crate::async_nouns`], both
//!   through `crate::noun_facade`), the `Dyn*`
//!   traits ([`crate::dynapi::nouns`]) and the re-export lists
//!   ([`reexport_nouns!`]);
//! * [`crate::command::surface`]: `StaticNoun`, the exhaustive
//!   `surface_entry` match and the runtime gate of every built-in inquiry
//!   (`BuiltinInquiryGate`);
//! * the typed-request inventory in [`crate::request::builtin`].
//!
//! Adding a built-in command is described in `CONTRIBUTING.md` ("Adding a
//! Built-in Command"): one row here is the only surface edit it needs.
//!
//! [`noun_table!`] is a continuation-passing macro: it takes the name of a
//! consumer macro, plus optional fixed-shape leading arguments, and hands
//! that consumer every noun header and row in one invocation
//! (`noun_table!(static_noun_facade, [async], [.await]);`). The consumers own
//! the *differences* between the facades (async `fn` plus `.await`, blocking
//! `fn`, object-safe `dyn` variants, different receiver and session plumbing,
//! different cfg features), while everything that must agree lives here.
//!
//! # Row grammar
//!
//! Every row has one shape, and every consumer matches it in one
//! non-recursive repetition:
//!
//! ```text
//! <kind> [<BuiltinCommand>?] <method>(<arg>: <type>, ...) -> <type>
//!     [where <Marker> [+ <Marker>]] = [<request>];
//! ```
//!
//! * `<kind>` is `inquiry`, `plain`, `applied` or `targeted` — the semantic
//!   return class the ledger records for the row.
//! * The brackets hold the one canonical [`BuiltinCommand`] variant a command
//!   row sends. They are empty (`[]`) for an inquiry row and for a convenience
//!   row that builds a canonical row's request and therefore owns no command
//!   ID.
//! * `-> <type>` is the request type of a command row and the decoded response
//!   of an inquiry row; it is always written.
//! * `where <Marker>` is the row's typed-support gate.  It is a bare marker
//!   name: the static facades bound their profile parameter with
//!   `$crate::capabilities::<Marker>`, so the compiler resolves it, and the
//!   surface registry maps it to its `TypedSupportSurface` with
//!   `typed_surface!`, which has no arm for a marker without a typed-support
//!   registry row.  The erased facade carries no bounds; it enforces the same
//!   gate at runtime through `validate_for_target` (a command through the
//!   ledger row its request serves, an inquiry through its
//!   `BuiltinInquiryGate`).
//! * `<request>` is the command or inquiry value to send.  The forms are:
//!   and only `noun_request!` (in `crate::noun_facade`) parses them:
//!   * `<expr>` — an infallible constructor.
//!   * `by_value <const expr>` — a constant of a request type whose values
//!     select different ledger rows (a `builtin_request!` entry with `rows`).
//!     Every row sending such a type uses this form, and the typed-request
//!     inventory checks at compile time that the constant selects the row's
//!     command.
//!   * `by_value [<const witness>] <expr>` — an argument-bearing constructor
//!     whose constant witness selects the same ledger row. Generated row tests
//!     check the evaluated request as well.
//!   * `checked <expr>` — a fallible constructor returning `Result<_>`.
//!   * `with_profile |<name>| <expr>` — a fallible constructor that needs the
//!     session's profile; the binder names it.
//!
//! The table ends with the three protocol exceptions, which are never noun
//! methods: `<broadcast | internal> [<BuiltinCommand>] <name> -> <request
//! type>;`.
//!
//! Rustdoc is a per-row attribute, so the three surfaces cannot document the
//! same method differently.
//!
//! # Noun headers
//!
//! Each noun's rows follow one header that declares every per-noun fact once:
//!
//! ```text
//! @noun <Noun> {
//!     accessor: <static accessor type>,
//!     getter: <camera getter>,
//!     dyn_trait: <object-safe trait>,
//!     gate: [always] | [domain <ProfileMarker>] | [typed <TypedMarker>],
//!     doc: "<rustdoc shared by every surface of the noun>",
//! };
//! ```
//!
//! `gate` is the noun's base capability gate: `[always]` needs none,
//! `[domain M]` names a profile domain marker and `[typed M]` a typed-support
//! marker.  A `[typed M]` gate bounds the static accessor itself; a
//! `[domain M]` gate bounds only its getter.  A row's runtime gate is its
//! `where` marker, else its header gate; the surface registry derives it once
//! per row (`RowGate`).  On the static facades a `where` clause bounds only
//! its own method, while the accessor or getter keeps the header gate, so the
//! static bound of a row with a `where` clause is the header gate and the
//! `where` marker together.  A `[domain M]` gate that some row inherits needs
//! a `DomainGate` with its runtime check.  The surface registry, the static
//! facades and the dynamic facade all read the headers: the accessor, getter
//! and `Dyn*` trait names and the one doc of every surface of the noun come
//! from here.
//!
//! # The motion view
//!
//! `MotionAccessor`/`DynMotion` is a safety and observation view rather than a
//! ledger noun: its three methods — `stop_all_motion`, `is_moving` and
//! `wait_until_idle` — reach the owner core directly and share no shape with
//! a command row. [`motion_table!`] lists them, with their one rustdoc, for the
//! same per-facade consumers (#816).
//! The rows follow an `@motion { .. };` header in the noun-header shape,
//! without a gate: the view needs no capability.

/// Hands the whole noun table to a consumer macro.
///
/// See the module documentation for the row grammar.  Invoke it as
/// `noun_table!(my_consumer);`, or with fixed-shape leading arguments
/// (`noun_table!(my_consumer, [async], [.await]);`), which the consumer
/// receives as single token trees before the first `@noun` header.
macro_rules! noun_table {
    ($consumer:ident $(, $arg:tt)*) => {
        $consumer! {
            $($arg)*

            @noun Power {
                accessor: PowerAccessor,
                getter: power,
                dyn_trait: DynPower,
                gate: [domain HasPower],
                doc: "Power commands and the power-state inquiry.",
            };
            /// Returns the camera's power status, including an internal circuit error.
            /// Use [`PowerStatus::is_on`](crate::PowerStatus::is_on) for a boolean view.
            inquiry [] state() -> command::PowerStatus = [command::PowerInquiry];
            /// Powers the camera on.
            plain [PowerOn] on() -> command::PowerOn = [command::PowerOn::new()];
            /// Places the camera in standby.
            plain [PowerStandby] off() -> command::PowerStandby = [command::PowerStandby::new()];

            @noun Zoom {
                accessor: ZoomAccessor,
                getter: zoom,
                dyn_trait: DynZoom,
                gate: [domain HasZoom],
                doc: "Optical and digital zoom commands and the zoom-position inquiry.",
            };
            /// Returns the current optical/digital zoom position.
            inquiry [] position() -> types::ZoomPosition
                = [command::ZoomPositionInquiry];
            /// Drives toward telephoto at standard speed.
            applied [ZoomTele] tele() -> builtin::ZoomDrive = [builtin::ZoomDrive::Tele];
            /// Drives toward wide angle at standard speed.
            applied [ZoomWide] wide() -> builtin::ZoomDrive = [builtin::ZoomDrive::Wide];
            /// Stops zoom movement.
            applied [ZoomStop] stop() -> builtin::ZoomStop = [builtin::ZoomStop];
            /// Drives toward telephoto at a validated variable speed.
            applied [ZoomTeleVariable] tele_variable(speed: types::ZoomSpeed)
                -> builtin::ZoomDrive
                = [builtin::ZoomDrive::TeleVariable(speed)];
            /// Drives toward wide angle at a validated variable speed.
            applied [ZoomWideVariable] wide_variable(speed: types::ZoomSpeed)
                -> builtin::ZoomDrive
                = [builtin::ZoomDrive::WideVariable(speed)];
            /// Moves to an absolute zoom position.
            targeted [ZoomPosition] set_position(position: types::ZoomPosition)
                -> builtin::ZoomTarget where HasDirectZoom
                = [builtin::ZoomTarget::new(position)];
            /// Moves to a normalized position across a documented zoom domain.
            ///
            /// `0.0` is the wide end and `1.0` the telephoto end of the selected
            /// domain. [`ZoomDomain::Optical`] spans the profile's documented
            /// optical range; [`ZoomDomain::OpticalPlusDigital`] requires the
            /// profile to document a digital maximum and never falls back to the
            /// optical range.
            targeted [] set_normalized(position: UnitInterval, domain: ZoomDomain)
                -> builtin::ZoomTarget where HasDirectZoom
                = [with_profile |profile| builtin::ZoomTarget::from_normalized(
                    position,
                    domain,
                    profile,
                )];
            /// Enables or disables digital zoom.
            plain [DigitalZoom] set_digital_zoom(enabled: bool) -> command::DigitalZoom
                where HasDigitalZoomToggle
                = [command::DigitalZoom::new(enabled)];


            /// Sony FR7 tele high resolution (R7).
            applied [Fr7ZoomHighResolutionTele] tele_high_resolution(speed: types::HighResolutionZoomSpeed) -> command::Fr7ZoomHighResolution
                where HasSonyFr7HighResolutionZoom
                = [command::Fr7ZoomHighResolution::Tele(speed)];
            /// Sony FR7 wide high resolution (R7).
            applied [Fr7ZoomHighResolutionWide] wide_high_resolution(speed: types::HighResolutionZoomSpeed) -> command::Fr7ZoomHighResolution
                where HasSonyFr7HighResolutionZoom
                = [command::Fr7ZoomHighResolution::Wide(speed)];
            /// Sony FR7 stop high resolution (R7).
            applied [Fr7ZoomHighResolutionStop] stop_high_resolution() -> command::Fr7ZoomHighResolutionStop
                where HasSonyFr7HighResolutionZoom
                = [command::Fr7ZoomHighResolutionStop];
            /// Reads Sony FR7 clear image position (R7).
            inquiry [] clear_image_position() -> types::ClearImageZoomPosition
                where HasSonyFr7ClearImageZoom
                = [command::Fr7ClearImageZoomPositionInquiry];

            @noun System {
                accessor: SystemAccessor,
                getter: system,
                dyn_trait: DynSystem,
                gate: [always],
                doc: "Settings persistence and the version inquiry.",
            };
            /// Returns the camera's Sony-format `CAM_VersionInq` reply: vendor,
            /// model, ROM revision and maximum socket.
            ///
            /// Gated by `HasVersionInquiry`: profiles whose cameras reply in
            /// another, unsourced layout (PTZOptics) do not expose it. Send the
            /// raw `81 09 00 02 FF` inquiry there instead.
            inquiry [] version() -> command::VersionInfo
                where HasVersionInquiry
                = [command::VersionInquiry];
            /// Saves the camera's current settings to non-volatile storage.
            plain [SettingsSave] save_settings() -> command::SettingsSaveCommand
                where HasPtzOpticsSettingsSave
                = [command::SettingsSaveCommand::new()];

            @noun PanTilt {
                accessor: PanTiltAccessor,
                getter: pan_tilt,
                dyn_trait: DynPanTilt,
                gate: [domain HasPanTilt],
                doc: "Pan/tilt movement, limits, and the position inquiry.",
            };
            /// Returns the current pan/tilt position.
            inquiry [] position() -> crate::camera::PanTiltPosition
                = [command::PanTiltPositionInquiry];
            /// Moves the pan/tilt mechanism to its home position.
            targeted [PanTiltHome] home() -> builtin::PanTiltHome = [builtin::PanTiltHome];
            /// Resets the pan/tilt mechanism.
            targeted [PanTiltReset] reset() -> builtin::PanTiltReset = [builtin::PanTiltReset];
            /// Starts a directional pan/tilt drive.
            applied [PanTiltDrive] move_direction(
                direction: command::PanTiltDirection,
                pan_speed: types::PanSpeed,
                tilt_speed: types::TiltSpeed
            ) -> builtin::PanTiltDrive = [checked builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed)];
            /// Stops pan/tilt movement using profile-safe stop speeds.
            applied [PanTiltStop] stop() -> builtin::PanTiltStop
                = [with_profile |profile| crate::stop_request::pan_tilt_stop_request(profile)];
            /// Moves to an absolute degree position at the selected speed.
            targeted [PanTiltAbsolute] absolute(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                -> builtin::PanTiltAbsolute
                = [with_target |target| builtin::PanTiltAbsolute::for_target_speed_level(
                    pan, tilt, speed, target,
                )];
            /// Moves by a relative degree offset at the selected speed. Each axis accepts
            /// the full span between the profile's absolute limits in either direction.
            targeted [PanTiltRelative] relative(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                -> builtin::PanTiltRelative
                = [with_target |target| builtin::PanTiltRelative::for_target_speed_level(
                    pan, tilt, speed, target,
                )];
            /// Sets one pan/tilt movement-limit corner.
            plain [PanTiltLimitSet] limit_set(
                corner: command::PanTiltLimitCorner,
                pan: Degrees<f32>,
                tilt: Degrees<f32>
            ) -> builtin::PanTiltLimitSet = [with_target |target| builtin::PanTiltLimitSet::for_target(
                corner,
                pan,
                tilt,
                target,
            )];
            /// Clears one pan/tilt movement-limit corner.
            plain [PanTiltLimitClear] limit_clear(corner: command::PanTiltLimitCorner)
                -> builtin::PanTiltLimitClear
                = [with_profile |profile| builtin::PanTiltLimitClear::for_profile(corner, profile)];

            @noun Focus {
                accessor: FocusAccessor,
                getter: focus,
                dyn_trait: DynFocus,
                gate: [domain HasFocus],
                doc: "Focus movement, modes, and focus inquiries.",
            };
            /// Returns the current focus position.
            inquiry [] position() -> types::FocusPosition
                = [command::FocusPositionInquiry];
            /// Returns the current focus mode.
            inquiry [] mode() -> command::FocusMode
                = [command::FocusModeInquiry];
            /// Drives focus farther at standard speed.
            applied [FocusFar] far() -> builtin::FocusDrive = [builtin::FocusDrive::Far];
            /// Drives focus nearer at standard speed.
            applied [FocusNear] near() -> builtin::FocusDrive = [builtin::FocusDrive::Near];
            /// Drives focus farther at a variable speed.
            applied [FocusFarVariable] far_variable(speed: types::FocusSpeed)
                -> builtin::FocusDrive
                = [builtin::FocusDrive::FarVariable(speed)];
            /// Drives focus nearer at a variable speed.
            applied [FocusNearVariable] near_variable(speed: types::FocusSpeed)
                -> builtin::FocusDrive
                = [builtin::FocusDrive::NearVariable(speed)];
            /// Stops focus movement.
            applied [FocusStop] stop() -> builtin::FocusStop = [builtin::FocusStop];
            /// Moves focus to an absolute position.
            targeted [FocusPosition] set_position(position: types::FocusPosition)
                -> builtin::FocusTarget
                = [builtin::FocusTarget::new(position)];
            /// Enables automatic focus mode.
            plain [FocusAuto] auto() -> builtin::FocusModeCommand
                = [builtin::FocusModeCommand::Auto];
            /// Enables manual focus mode.
            plain [FocusManual] manual() -> builtin::FocusModeCommand
                = [builtin::FocusModeCommand::Manual];
            /// Triggers one-push autofocus.
            applied [FocusOnePush] one_push() -> builtin::FocusTrigger where HasOnePushFocus
                = [by_value builtin::FocusTrigger::OnePush];
            /// Moves focus to infinity.
            targeted [FocusInfinity] infinity() -> builtin::FocusInfinity
                = [builtin::FocusInfinity];
            /// Toggles automatic/manual focus mode.
            plain [FocusToggle] toggle() -> builtin::FocusModeCommand
                = [builtin::FocusModeCommand::Toggle];
            /// Triggers vendor snap focus.
            applied [FocusSnap] snap() -> builtin::FocusTrigger where HasPtzOpticsSnapFocus
                = [by_value builtin::FocusTrigger::Snap];
            /// Selects a focus zone.
            plain [FocusZone] set_zone(zone: command::FocusZone)
                -> command::FocusZoneCommand where HasFocusZone
                = [command::FocusZoneCommand::new(zone)];
            /// Sets the autofocus sensitivity.
            plain [FocusAutoSensitivity] set_sensitivity(sensitivity: command::AutoFocusSensitivity)
                -> command::AutoFocusSensitivityCommand where HasAutoFocusSensitivity
                = [command::AutoFocusSensitivityCommand::new(sensitivity)];
            /// Sets the minimum focus distance.
            plain [FocusNearLimit] set_near_limit(position: types::FocusPosition)
                -> command::FocusNearLimitCommand where HasFocusNearLimitInquiry
                = [command::FocusNearLimitCommand::new(position)];
            /// Sets the focus-lock mode.
            plain [FocusLock] set_lock(mode: command::FocusLock) -> command::FocusLock
                where HasFocusLock = [mode];
            /// Presses the vendor Push-AF control.
            applied [PushAfPress] push_af_press() -> builtin::PushAfPress where HasPushAutoFocus
                = [builtin::PushAfPress::new()];
            /// Releases the vendor Push-AF control.
            applied [PushAfRelease] push_af_release() -> builtin::PushAfRelease
                where HasPushAutoFocus = [builtin::PushAfRelease::new()];
            /// Returns the configured focus near limit.
            inquiry [] near_limit() -> types::FocusPosition
                where HasFocusNearLimitInquiry
                = [command::FocusNearLimitInquiry];
            /// Returns the configured focus zone.
            inquiry [] zone() -> command::FocusZone where HasFocusZoneInquiry
                = [command::FocusZoneInquiry];
            /// Returns the autofocus sensitivity.
            inquiry [] sensitivity()
                -> command::AutoFocusSensitivity where HasAutoFocusSensitivity
                = [command::AutoFocusSensitivityInquiry];


            @noun Presets {
                accessor: PresetsAccessor,
                getter: presets,
                dyn_trait: DynPresets,
                gate: [domain HasPresets],
                doc: "Preset recall, store, clear, and recall-speed commands.",
            };
            /// Recalls a stored preset as a targeted operation.
            targeted [PresetRecall] recall(preset: command::PresetNumber)
                -> builtin::PresetRecall
                = [with_profile |profile| builtin::PresetRecall::for_profile(preset, profile)];
            /// Sets the preset-recall speed.
            plain [PresetRecallSpeed] set_recall_speed(speed: command::PresetRecallSpeed)
                -> command::PresetRecallSpeedCommand
                where HasPtzOpticsPresetRecallSpeed
                = [command::PresetRecallSpeedCommand::new(speed)];
            /// Stores the current camera state in a preset.
            plain [PresetSet] set(preset: command::PresetNumber) -> builtin::PresetSet
                = [builtin::PresetSet::new(preset)];
            /// Clears a stored preset.
            plain [PresetReset] reset(preset: command::PresetNumber) -> builtin::PresetReset
                = [builtin::PresetReset::new(preset)];


            /// Sony FR7 select speed (R7).
            plain [Fr7SetPresetSpeedSelection] select_speed(mode: command::Fr7PresetSpeedSelection) -> command::Fr7SetPresetSpeedSelection
                where HasSonyFr7PresetSpeed
                = [command::Fr7SetPresetSpeedSelection::new(mode)];
            /// Reads Sony FR7 speed selection (R7).
            inquiry [] speed_selection() -> command::Fr7PresetSpeedSelection
                where HasSonyFr7PresetSpeed
                = [command::Fr7PresetSpeedSelectionInquiry];
            /// Sony FR7 set common speed (R7).
            plain [Fr7SetCommonPresetSpeed] set_common_speed(speed: types::PresetSpeed) -> command::Fr7SetCommonPresetSpeed
                where HasSonyFr7PresetSpeed
                = [command::Fr7SetCommonPresetSpeed::new(speed)];
            /// Reads Sony FR7 common speed (R7).
            inquiry [] common_speed() -> types::PresetSpeed
                where HasSonyFr7PresetSpeed
                = [command::Fr7CommonPresetSpeedInquiry];
            /// Sony FR7 set separate mode (R7).
            plain [Fr7SetSeparatePresetMode] set_separate_mode(preset: types::Fr7PresetNumber, mode: command::Fr7PresetSeparateMode) -> command::Fr7SetSeparatePresetMode
                where HasSonyFr7PresetSpeed
                = [command::Fr7SetSeparatePresetMode::new(preset, mode)];
            /// Reads Sony FR7 separate mode (R7).
            inquiry [] separate_mode(preset: types::Fr7PresetNumber) -> command::Fr7PresetSeparateMode
                where HasSonyFr7PresetSpeed
                = [command::Fr7SeparatePresetModeInquiry::new(preset)];
            /// Sony FR7 set separate speed (R7).
            plain [Fr7SetSeparatePresetSpeed] set_separate_speed(preset: types::Fr7PresetNumber, speed: types::PresetSpeed) -> command::Fr7SetSeparatePresetSpeed
                where HasSonyFr7PresetSpeed
                = [command::Fr7SetSeparatePresetSpeed::new(preset, speed)];
            /// Reads Sony FR7 separate speed (R7).
            inquiry [] separate_speed(preset: types::Fr7PresetNumber) -> types::PresetSpeed
                where HasSonyFr7PresetSpeed
                = [command::Fr7SeparatePresetSpeedInquiry::new(preset)];
            /// Sony FR7 set separate duration (R7).
            plain [Fr7SetSeparatePresetDuration] set_separate_duration(preset: types::Fr7PresetNumber, duration: types::PresetDuration) -> command::Fr7SetSeparatePresetDuration
                where HasSonyFr7PresetDuration
                = [command::Fr7SetSeparatePresetDuration::new(preset, duration)];
            /// Reads Sony FR7 separate duration (R7).
            inquiry [] separate_duration(preset: types::Fr7PresetNumber) -> types::PresetDuration
                where HasSonyFr7PresetDuration
                = [command::Fr7SeparatePresetDurationInquiry::new(preset)];

            @noun Exposure {
                accessor: ExposureAccessor,
                getter: exposure,
                dyn_trait: DynExposure,
                gate: [domain HasExposure],
                doc: "Exposure, iris, shutter, brightness, and gain commands and inquiries.",
            };
            /// Returns the active exposure mode.
            inquiry [] mode() -> command::ExposureMode
                where HasExposureMode
                = [command::ExposureModeInquiry];
            /// Sets the exposure mode.
            plain [ExposureMode] set_mode(mode: command::ExposureMode)
                -> command::ExposureCommand where HasExposureMode
                = [command::ExposureCommand::new(mode)];
            /// Returns the shutter speed.
            inquiry [] shutter() -> types::ShutterSpeed
                = [command::ShutterInquiry];
            /// Restores the camera's shutter default.
            plain [ShutterReset] shutter_reset() -> command::Shutter = [command::Shutter::Reset];
            /// Increases shutter speed by one camera-defined step.
            plain [ShutterUp] shutter_up() -> command::Shutter = [command::Shutter::Up];
            /// Decreases shutter speed by one camera-defined step.
            plain [ShutterDown] shutter_down() -> command::Shutter = [command::Shutter::Down];
            /// Sets an explicit shutter speed.
            plain [ShutterDirect] shutter_direct(speed: types::ShutterSpeed)
                -> command::Shutter = [command::Shutter::SetSpeed(speed)];
            /// Returns the exposure compensation value.
            inquiry [] compensation()
                -> types::ExposureCompensationLevel
                where HasExposureCompensation
                = [command::ExposureCompensationInquiry];
            /// Returns whether exposure compensation is enabled.
            inquiry [] compensation_enabled() -> bool
                where HasExposureCompensation
                = [command::ExposureCompensationModeInquiry];
            /// Returns the camera's exposure compensation position.
            inquiry [] compensation_position()
                -> types::ExposureCompensationPosition
                where HasExposureCompensation
                = [command::ExposureCompensationPositionInquiry];
            /// Enables exposure compensation.
            plain [ExposureCompensationOn] compensation_on() -> command::ExposureCompensation
                where HasExposureCompensation
                = [command::ExposureCompensation::On];
            /// Disables exposure compensation.
            plain [ExposureCompensationOff] compensation_off() -> command::ExposureCompensation
                where HasExposureCompensation
                = [command::ExposureCompensation::Off];
            /// Resets exposure compensation.
            plain [ExposureCompensationReset] compensation_reset()
                -> command::ExposureCompensation where HasExposureCompensation
                = [command::ExposureCompensation::Reset];
            /// Increases exposure compensation by one step.
            plain [ExposureCompensationUp] compensation_up() -> command::ExposureCompensation
                where HasExposureCompensation
                = [command::ExposureCompensation::Up];
            /// Decreases exposure compensation by one step.
            plain [ExposureCompensationDown] compensation_down()
                -> command::ExposureCompensation where HasExposureCompensation
                = [command::ExposureCompensation::Down];
            /// Sets direct exposure compensation.
            plain [ExposureCompensationDirect] compensation_direct(
                level: types::ExposureCompensationLevel
            ) -> command::ExposureCompensation
                where HasExposureCompensation
                = [command::ExposureCompensation::SetLevel(level)];

            /// Sets the wide-dynamic-range level.
            plain [DynamicRange] set_dynamic_range(level: types::DynamicRangeLevel)
                -> command::DynamicRange where HasWideDynamicRange
                = [command::DynamicRange::new(level)];
            /// Returns the iris level.
            inquiry [] iris() -> types::IrisLevel where HasIrisControl
                = [command::IrisInquiry];
            /// Resets the iris.
            targeted [IrisReset] iris_reset() -> builtin::IrisReset where HasIrisControl
                = [builtin::IrisReset::new()];
            /// Increases the iris by one step.
            targeted [IrisUp] iris_up() -> builtin::IrisUp where HasIrisControl
                = [builtin::IrisUp::new()];
            /// Decreases the iris by one step.
            targeted [IrisDown] iris_down() -> builtin::IrisDown where HasIrisControl
                = [builtin::IrisDown::new()];
            /// Sets a direct iris level.
            targeted [IrisDirect] iris_direct(level: types::IrisLevel)
                -> builtin::IrisDirect where HasIrisControl
                = [builtin::IrisDirect::new(level)];
            /// Returns exposure brightness.
            inquiry [] brightness() -> types::BrightnessLevel
                where HasBrightnessControl
                = [command::BrightnessInquiry];
            /// Resets exposure brightness.
            plain [BrightnessReset] brightness_reset() -> command::Brightness
                where HasBrightnessControl = [command::Brightness::Reset];
            /// Increases exposure brightness.
            plain [BrightnessUp] brightness_up() -> command::Brightness
                where HasBrightnessControl = [command::Brightness::Up];
            /// Decreases exposure brightness.
            plain [BrightnessDown] brightness_down() -> command::Brightness
                where HasBrightnessControl = [command::Brightness::Down];
            /// Sets exposure brightness through the bright-direct command.
            plain [BrightnessSet] brightness_set(level: types::BrightnessLevel)
                -> command::Brightness where HasBrightnessControl
                = [command::Brightness::SetLevel(level)];
            /// Returns gain.
            inquiry [] gain() -> types::GainLevel = [command::GainInquiry];
            /// Resets gain.
            plain [GainReset] gain_reset() -> command::Gain = [command::Gain::Reset];
            /// Increases gain.
            plain [GainUp] gain_up() -> command::Gain = [command::Gain::Up];
            /// Decreases gain.
            plain [GainDown] gain_down() -> command::Gain = [command::Gain::Down];
            /// Sets direct gain.
            plain [GainDirect] gain_direct(level: types::GainLevel) -> command::Gain
                = [command::Gain::SetValue(level)];
            /// Returns the configured gain limit.
            inquiry [] gain_limit() -> types::GainLimit
                = [command::GainLimitInquiry];
            /// Sets the configured gain limit.
            plain [GainLimit] set_gain_limit(limit: types::GainLimit)
                -> command::GainLimitCommand = [command::GainLimitCommand::new(limit)];
            /// Sets anti-flicker mode.
            plain [AntiFlicker] set_anti_flicker(mode: command::AntiFlickerMode)
                -> command::AntiFlickerCommand
                where HasPtzOpticsAntiFlicker
                = [command::AntiFlickerCommand::new(mode)];
            /// Returns the configured anti-flicker mode.
            inquiry [] flicker_mode() -> command::AntiFlickerMode
                where HasPtzOpticsAntiFlicker
                = [command::FlickerModeInquiry];
            /// Enables spotlight mode.
            plain [SpotlightOn] spotlight_on() -> command::SpotlightOn
                where HasSonySpotlight
                = [command::SpotlightOn::new()];
            /// Disables spotlight mode.
            plain [SpotlightOff] spotlight_off() -> command::SpotlightOff
                where HasSonySpotlight
                = [command::SpotlightOff::new()];
            /// Enables automatic slow shutter.
            plain [AutoSlowShutterOn] auto_slow_shutter_on() -> command::AutoSlowShutterOn
                where HasSonyAutoSlowShutter
                = [command::AutoSlowShutterOn::new()];
            /// Disables automatic slow shutter.
            plain [AutoSlowShutterOff] auto_slow_shutter_off() -> command::AutoSlowShutterOff
                where HasSonyAutoSlowShutter
                = [command::AutoSlowShutterOff::new()];


            /// Sony FR7 ae level up (R7).
            plain [Fr7AeLevelUp] ae_level_up() -> command::Fr7AeLevel
                where HasSonyFr7AeLevel
                = [command::Fr7AeLevel::Up];
            /// Sony FR7 ae level down (R7).
            plain [Fr7AeLevelDown] ae_level_down() -> command::Fr7AeLevel
                where HasSonyFr7AeLevel
                = [command::Fr7AeLevel::Down];
            /// Reads Sony FR7 ae level (R7).
            inquiry [] ae_level() -> types::AeLevel
                where HasSonyFr7AeLevel
                = [command::Fr7AeLevelInquiry];
            /// Sony FR7 set auto gain (R7).
            plain [Fr7AutoGain] set_auto_gain(enabled: bool) -> command::Fr7AutoGain
                where HasSonyFr7AutoGain
                = [command::Fr7AutoGain::new(enabled)];
            /// Reads Sony FR7 auto gain (R7).
            inquiry [] auto_gain() -> bool
                where HasSonyFr7AutoGain
                = [command::Fr7AutoGainInquiry];
            /// Sony FR7 set auto shutter (R7).
            plain [Fr7AutoShutter] set_auto_shutter(enabled: bool) -> command::Fr7AutoShutter
                where HasSonyFr7AutoShutter
                = [command::Fr7AutoShutter::new(enabled)];
            /// Reads Sony FR7 auto shutter (R7).
            inquiry [] auto_shutter() -> bool
                where HasSonyFr7AutoShutter
                = [command::Fr7AutoShutterInquiry];
            /// Sony FR7 set auto iris (R7).
            plain [Fr7AutoIris] set_auto_iris(enabled: bool) -> command::Fr7AutoIris
                where HasSonyFr7Iris
                = [command::Fr7AutoIris::new(enabled)];
            /// Reads Sony FR7 auto iris (R7).
            inquiry [] auto_iris() -> bool
                where HasSonyFr7Iris
                = [command::Fr7AutoIrisInquiry];
            /// Sony FR7 iris up (R7).
            applied [Fr7IrisUp] iris_up_by(steps: types::IrisStep) -> command::Fr7IrisUp
                where HasSonyFr7Iris
                = [command::Fr7IrisUp::new(steps)];
            /// Sony FR7 iris down (R7).
            applied [Fr7IrisDown] iris_down_by(steps: types::IrisStep) -> command::Fr7IrisDown
                where HasSonyFr7Iris
                = [command::Fr7IrisDown::new(steps)];

            @noun WhiteBalance {
                accessor: WhiteBalanceAccessor,
                getter: white_balance,
                dyn_trait: DynWhiteBalance,
                gate: [domain HasWhiteBalance],
                doc: "White-balance, color-temperature, and color-channel commands and inquiries.",
            };
            /// Returns the active white-balance mode.
            inquiry [] mode() -> command::WhiteBalanceMode
                = [command::WhiteBalanceModeInquiry];
            /// Selects automatic white balance.
            plain [WhiteBalanceAuto] auto() -> command::WhiteBalanceCommand
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::Auto,
                }];
            /// Selects the indoor white-balance preset.
            plain [WhiteBalanceIndoor] indoor() -> command::WhiteBalanceCommand
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::Indoor,
                }];
            /// Selects the outdoor white-balance preset.
            plain [WhiteBalanceOutdoor] outdoor() -> command::WhiteBalanceCommand
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::Outdoor,
                }];
            /// Selects one-push white balance.
            plain [WhiteBalanceOnePush] one_push() -> command::WhiteBalanceCommand
                where HasOnePushWhiteBalance
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::OnePush,
                }];
            /// Selects auto-tracking white balance.
            plain [WhiteBalanceAutoTracking] atw() -> command::WhiteBalanceCommand
                where HasAutoTrackingWhiteBalance
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::ATW,
                }];
            /// Selects manual white balance.
            plain [WhiteBalanceManual] manual() -> command::WhiteBalanceCommand
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::Manual,
                }];
            /// Selects color-temperature white-balance mode.
            plain [WhiteBalanceColorTemperature] color_temperature_mode()
                -> command::WhiteBalanceCommand where HasColorTemperature
                = [by_value command::WhiteBalanceCommand {
                    mode: command::WhiteBalanceMode::ColorTemperature,
                }];
            /// Sets automatic white-balance sensitivity.
            plain [AutoWhiteBalanceSensitivity] set_sensitivity(
                sensitivity: command::AutoWhiteBalanceSensitivity
            ) -> command::AWBSensitivityCommand where HasAutoWhiteBalanceSensitivity
                = [command::AWBSensitivityCommand::new(sensitivity)];
            /// Returns automatic white-balance sensitivity.
            inquiry [] sensitivity()
                -> command::AutoWhiteBalanceSensitivity
                where HasAutoWhiteBalanceSensitivity
                = [command::AutoWhiteBalanceSensitivityInquiry];
            /// Triggers one-push white-balance calibration.
            plain [OnePushWhiteBalanceTrigger] one_push_trigger()
                -> command::OnePushTriggerCommand where HasOnePushWhiteBalance
                = [command::OnePushTriggerCommand::new()];
            /// Sets red-channel white-balance tuning.
            plain [RedTuning] set_red_tuning(level: types::RedTuning)
                -> command::RedTuningCommand where HasRgbTuning
                = [command::RedTuningCommand::new(level)];
            /// Sets blue-channel white-balance tuning.
            plain [BlueTuning] set_blue_tuning(level: types::BlueTuning)
                -> command::BlueTuningCommand where HasRgbTuning
                = [command::BlueTuningCommand::new(level)];
            /// Returns the color temperature.
            inquiry [] color_temperature() -> types::ColorTemp
                where HasColorTemperatureInquiry
                = [command::ColorTemperatureInquiry];
            /// Resets color temperature.
            plain [ColorTemperatureReset] reset_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = [command::ColorTemperature::Reset];
            /// Increases color temperature.
            plain [ColorTemperatureUp] increase_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = [command::ColorTemperature::Up];
            /// Decreases color temperature.
            plain [ColorTemperatureDown] decrease_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = [command::ColorTemperature::Down];
            /// Sets a direct color-temperature value.
            plain [ColorTemperatureDirect] set_color_temperature(temperature: types::ColorTemp)
                -> command::ColorTemperature where HasColorTemperature
                = [command::ColorTemperature::SetTemperature(temperature)];
            /// Returns the red-channel gain.
            inquiry [] red_gain() -> types::RedChannel where HasRgbGain
                = [command::RedGainInquiry];
            /// Resets red-channel gain.
            plain [RedGainReset] reset_red_gain() -> command::RedGain where HasRgbGain
                = [command::RedGain::Reset];
            /// Increases red-channel gain.
            plain [RedGainUp] increase_red_gain() -> command::RedGain where HasRgbGain
                = [command::RedGain::Up];
            /// Decreases red-channel gain.
            plain [RedGainDown] decrease_red_gain() -> command::RedGain where HasRgbGain
                = [command::RedGain::Down];
            /// Sets direct red-channel gain.
            plain [RedGainDirect] set_red_gain(value: types::RedChannel) -> command::RedGain
                where HasRgbGain
                = [command::RedGain::SetValue(value)];
            /// Returns the blue-channel gain.
            inquiry [] blue_gain() -> types::BlueChannel where HasRgbGain
                = [command::BlueGainInquiry];
            /// Resets blue-channel gain.
            plain [BlueGainReset] reset_blue_gain() -> command::BlueGain where HasRgbGain
                = [command::BlueGain::Reset];
            /// Increases blue-channel gain.
            plain [BlueGainUp] increase_blue_gain() -> command::BlueGain where HasRgbGain
                = [command::BlueGain::Up];
            /// Decreases blue-channel gain.
            plain [BlueGainDown] decrease_blue_gain() -> command::BlueGain where HasRgbGain
                = [command::BlueGain::Down];
            /// Sets direct blue-channel gain.
            plain [BlueGainDirect] set_blue_gain(value: types::BlueChannel) -> command::BlueGain
                where HasRgbGain
                = [command::BlueGain::SetValue(value)];
            /// Returns red-channel tuning.
            inquiry [] red_tuning() -> types::RedTuning where HasRgbTuning
                = [command::RedTuningInquiry];
            /// Returns blue-channel tuning.
            inquiry [] blue_tuning() -> types::BlueTuning where HasRgbTuning
                = [command::BlueTuningInquiry];


            /// Sony FR7 set from scene (R7).
            plain [Fr7WhiteBalanceSet] set_from_scene() -> command::Fr7WhiteBalanceSet
                where HasSonyFr7WhiteBalance
                = [command::Fr7WhiteBalanceSet];
            /// Selects Sony FR7 preset white balance (R7 p. 15).
            plain [WhiteBalancePreset] preset() -> command::WhiteBalanceCommand
                where HasSonyFr7WhiteBalance
                = [by_value command::WhiteBalanceCommand { mode: command::WhiteBalanceMode::Preset }];
            /// Sony FR7 preset white up (R7).
            plain [Fr7PresetWhiteUp] preset_white_up(steps: types::WhiteBalanceStep) -> command::Fr7PresetWhite
                where HasSonyFr7PresetWhite
                = [command::Fr7PresetWhite::Up(steps)];
            /// Sony FR7 preset white down (R7).
            plain [Fr7PresetWhiteDown] preset_white_down(steps: types::WhiteBalanceStep) -> command::Fr7PresetWhite
                where HasSonyFr7PresetWhite
                = [command::Fr7PresetWhite::Down(steps)];
            /// Sony FR7 set preset white (R7).
            plain [Fr7PresetWhiteDirect] set_preset_white(value: types::WhiteKelvin) -> command::Fr7PresetWhite
                where HasSonyFr7PresetWhite
                = [command::Fr7PresetWhite::Direct(value)];
            /// Reads Sony FR7 preset white (R7).
            inquiry [] preset_white() -> types::WhiteKelvin
                where HasSonyFr7PresetWhite
                = [command::Fr7PresetWhiteInquiry];
            /// Sony FR7 memory temperature up (R7).
            plain [Fr7MemoryColorTemperatureUp] memory_temperature_up(steps: types::MemoryColorTemperatureStep) -> command::Fr7MemoryColorTemperature
                where HasSonyFr7MemoryColorTemperature
                = [command::Fr7MemoryColorTemperature::Up(steps)];
            /// Sony FR7 memory temperature down (R7).
            plain [Fr7MemoryColorTemperatureDown] memory_temperature_down(steps: types::MemoryColorTemperatureStep) -> command::Fr7MemoryColorTemperature
                where HasSonyFr7MemoryColorTemperature
                = [command::Fr7MemoryColorTemperature::Down(steps)];
            /// Sony FR7 set memory temperature (R7).
            plain [Fr7MemoryColorTemperatureDirect] set_memory_temperature(value: types::WhiteKelvin) -> command::Fr7MemoryColorTemperature
                where HasSonyFr7MemoryColorTemperature
                = [command::Fr7MemoryColorTemperature::Direct(value)];
            /// Reads Sony FR7 memory temperature (R7).
            inquiry [] memory_temperature() -> types::WhiteKelvin
                where HasSonyFr7MemoryColorTemperature
                = [command::Fr7MemoryColorTemperatureInquiry];
            /// Sony FR7 memory red gain up (R7).
            plain [Fr7MemoryRedGainUp] memory_red_gain_up(steps: types::WhiteBalanceStep) -> command::Fr7MemoryRedGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryRedGain::Up(steps)];
            /// Sony FR7 memory red gain down (R7).
            plain [Fr7MemoryRedGainDown] memory_red_gain_down(steps: types::WhiteBalanceStep) -> command::Fr7MemoryRedGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryRedGain::Down(steps)];
            /// Sony FR7 set memory red gain (R7).
            plain [Fr7MemoryRedGainDirect] set_memory_red_gain(value: types::WhiteBalanceGain) -> command::Fr7MemoryRedGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryRedGain::Direct(value)];
            /// Reads Sony FR7 memory red gain (R7).
            inquiry [] memory_red_gain() -> types::WhiteBalanceGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryRedGainInquiry];
            /// Sony FR7 memory blue gain up (R7).
            plain [Fr7MemoryBlueGainUp] memory_blue_gain_up(steps: types::WhiteBalanceStep) -> command::Fr7MemoryBlueGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryBlueGain::Up(steps)];
            /// Sony FR7 memory blue gain down (R7).
            plain [Fr7MemoryBlueGainDown] memory_blue_gain_down(steps: types::WhiteBalanceStep) -> command::Fr7MemoryBlueGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryBlueGain::Down(steps)];
            /// Sony FR7 set memory blue gain (R7).
            plain [Fr7MemoryBlueGainDirect] set_memory_blue_gain(value: types::WhiteBalanceGain) -> command::Fr7MemoryBlueGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryBlueGain::Direct(value)];
            /// Reads Sony FR7 memory blue gain (R7).
            inquiry [] memory_blue_gain() -> types::WhiteBalanceGain
                where HasSonyFr7MemoryGain
                = [command::Fr7MemoryBlueGainInquiry];
            /// Sony FR7 memory tint up (R7).
            plain [Fr7MemoryTintUp] memory_tint_up(steps: types::WhiteBalanceStep) -> command::Fr7MemoryTint
                where HasSonyFr7MemoryTint
                = [command::Fr7MemoryTint::Up(steps)];
            /// Sony FR7 memory tint down (R7).
            plain [Fr7MemoryTintDown] memory_tint_down(steps: types::WhiteBalanceStep) -> command::Fr7MemoryTint
                where HasSonyFr7MemoryTint
                = [command::Fr7MemoryTint::Down(steps)];
            /// Sony FR7 set memory tint (R7).
            plain [Fr7MemoryTintDirect] set_memory_tint(value: types::TintOffset) -> command::Fr7MemoryTint
                where HasSonyFr7MemoryTint
                = [command::Fr7MemoryTint::Direct(value)];
            /// Reads Sony FR7 memory tint (R7).
            inquiry [] memory_tint() -> types::TintOffset
                where HasSonyFr7MemoryTint
                = [command::Fr7MemoryTintInquiry];
            /// Sony FR7 atw temperature offset up (R7).
            plain [Fr7AtwColorTemperatureOffsetUp] atw_temperature_offset_up(steps: types::WhiteBalanceStep) -> command::Fr7AtwColorTemperatureOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwColorTemperatureOffset::Up(steps)];
            /// Sony FR7 atw temperature offset down (R7).
            plain [Fr7AtwColorTemperatureOffsetDown] atw_temperature_offset_down(steps: types::WhiteBalanceStep) -> command::Fr7AtwColorTemperatureOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwColorTemperatureOffset::Down(steps)];
            /// Sony FR7 set atw temperature offset (R7).
            plain [Fr7AtwColorTemperatureOffsetDirect] set_atw_temperature_offset(value: types::TintOffset) -> command::Fr7AtwColorTemperatureOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwColorTemperatureOffset::Direct(value)];
            /// Reads Sony FR7 atw temperature offset (R7).
            inquiry [] atw_temperature_offset() -> types::TintOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwColorTemperatureOffsetInquiry];
            /// Sony FR7 atw tint offset up (R7).
            plain [Fr7AtwTintOffsetUp] atw_tint_offset_up(steps: types::WhiteBalanceStep) -> command::Fr7AtwTintOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwTintOffset::Up(steps)];
            /// Sony FR7 atw tint offset down (R7).
            plain [Fr7AtwTintOffsetDown] atw_tint_offset_down(steps: types::WhiteBalanceStep) -> command::Fr7AtwTintOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwTintOffset::Down(steps)];
            /// Sony FR7 set atw tint offset (R7).
            plain [Fr7AtwTintOffsetDirect] set_atw_tint_offset(value: types::TintOffset) -> command::Fr7AtwTintOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwTintOffset::Direct(value)];
            /// Reads Sony FR7 atw tint offset (R7).
            inquiry [] atw_tint_offset() -> types::TintOffset
                where HasSonyFr7AtwOffsets
                = [command::Fr7AtwTintOffsetInquiry];

            @noun Image {
                accessor: ImageAccessor,
                getter: image,
                dyn_trait: DynImage,
                gate: [domain HasImageProcessing],
                doc: "Image-processing, noise-reduction, flip, and freeze commands and inquiries.",
            };
            /// Returns image saturation.
            inquiry [] saturation() -> types::SaturationLevel
                where HasSaturationControl
                = [command::SaturationInquiry];
            /// Sets image saturation.
            plain [Saturation] set_saturation(level: types::SaturationLevel)
                -> command::SaturationCommand where HasSaturationControl
                = [command::SaturationCommand::new(level)];
            /// Returns image hue.
            inquiry [] hue() -> types::HueLevel where HasHueControl
                = [command::HueInquiry];
            /// Sets image hue.
            plain [Hue] set_hue(level: types::HueLevel) -> command::HueCommand where HasHueControl
                = [command::HueCommand::new(level)];
            /// Returns image luminance.
            inquiry [] luminance() -> types::LuminanceLevel
                where HasLuminanceControl
                = [command::LuminanceInquiry];
            /// Sets image luminance.
            plain [Luminance] set_luminance(level: types::LuminanceLevel)
                -> command::Luminance where HasLuminanceControl
                = [command::Luminance::new(level)];
            /// Returns image contrast.
            inquiry [] contrast() -> types::ContrastLevel
                where HasContrastControl
                = [command::ContrastInquiry];
            /// Sets image contrast.
            plain [Contrast] set_contrast(level: types::ContrastLevel)
                -> command::Contrast where HasContrastControl
                = [command::Contrast::new(level)];
            /// Returns the gamma curve.
            inquiry [] gamma() -> types::GammaLevel where HasGammaControl
                = [command::GammaInquiry];
            /// Sets the gamma curve.
            plain [Gamma] set_gamma(level: types::GammaLevel) -> command::GammaCommand
                where HasGammaControl
                = [command::GammaCommand::new(level)];
            /// Returns the sharpness mode.
            inquiry [] sharpness_mode() -> command::SharpnessMode
                where HasSharpnessMode
                = [command::SharpnessModeInquiry];
            /// Returns the sharpness level.
            inquiry [] sharpness_level() -> types::SharpnessLevel
                where HasSharpnessControl
                = [command::SharpnessPositionInquiry];
            /// Sets the sharpness mode.
            plain [SharpnessMode] set_sharpness_mode(mode: command::SharpnessMode)
                -> command::Sharpness where HasSharpnessMode
                = [by_value [command::Sharpness::Mode(command::SharpnessMode::Manual)] command::Sharpness::Mode(mode)];
            /// Resets sharpness.
            plain [SharpnessReset] reset_sharpness() -> command::Sharpness
                where HasSharpnessControl = [command::Sharpness::Reset];
            /// Increases sharpness by one step.
            plain [SharpnessUp] increase_sharpness() -> command::Sharpness
                where HasSharpnessControl = [command::Sharpness::Up];
            /// Decreases sharpness by one step.
            plain [SharpnessDown] decrease_sharpness() -> command::Sharpness
                where HasSharpnessControl = [command::Sharpness::Down];
            /// Sets a direct sharpness level.
            plain [SharpnessDirect] set_sharpness(level: types::SharpnessLevel)
                -> command::Sharpness where HasSharpnessControl
                = [command::Sharpness::SetLevel { value: level }];
            /// Returns backlight compensation state.
            inquiry [] backlight() -> bool where HasBacklightCompensation
                = [command::BacklightInquiry];
            /// Enables or disables backlight compensation.
            plain [Backlight] set_backlight(enabled: bool) -> command::BacklightCommand
                where HasBacklightCompensation
                = [command::BacklightCommand::new(enabled)];
            /// Returns 2D noise reduction level.
            inquiry [] noise_reduction_2d()
                -> command::NoiseReduction2D where HasNoiseReduction2D
                = [command::NoiseReduction2DInquiry];
            /// Returns the 2D noise reduction mode.
            inquiry [] noise_reduction_2d_mode()
                -> command::NoiseReduction2DMode where HasNoiseReduction2DMode
                = [command::NoiseReduction2DModeInquiry];
            /// Returns 3D noise reduction level.
            inquiry [] noise_reduction_3d()
                -> command::NoiseReduction3D where HasNoiseReduction3D
                = [command::NoiseReduction3DInquiry];
            /// Sets the 2D noise-reduction mode.
            plain [NoiseReduction2dMode] set_noise_reduction_2d_mode(
                mode: command::NoiseReduction2DMode
            ) -> command::NoiseReduction2DModeCommand where HasNoiseReduction2DMode
                = [command::NoiseReduction2DModeCommand::new(mode)];
            /// Sets the 2D noise-reduction level.
            plain [NoiseReduction2d] set_noise_reduction_2d(level: types::NoiseReduction2DLevel)
                -> command::NoiseReduction2D where HasNoiseReduction2DControl
                = [by_value [command::NoiseReduction2D::with_level(types::NoiseReduction2DLevel::MIN)] command::NoiseReduction2D::with_level(level)];
            /// Disables 2D noise reduction.
            plain [NoiseReduction2dOff] disable_noise_reduction_2d() -> command::NoiseReduction2D
                where HasNoiseReduction2DControl = [by_value command::NoiseReduction2D::off()];
            /// Sets the 3D noise-reduction level.
            plain [NoiseReduction3d] set_noise_reduction_3d(level: types::NoiseReduction3DLevel)
                -> command::NoiseReduction3D where HasNoiseReduction3DControl
                = [by_value [command::NoiseReduction3D::with_level(types::NoiseReduction3DLevel::MIN)] command::NoiseReduction3D::with_level(level)];
            /// Disables 3D noise reduction.
            plain [NoiseReduction3dOff] disable_noise_reduction_3d() -> command::NoiseReduction3D
                where HasNoiseReduction3DControl = [by_value command::NoiseReduction3D::off()];
            /// Disables vertical image flip.
            plain [ImageFlipOff] disable_flip() -> builtin::ImageFlipCommand where HasImageFlip
                = [builtin::ImageFlipCommand::new(command::Flip::Off)];
            /// Enables vertical image flip.
            plain [ImageFlipVertical] enable_flip() -> builtin::ImageFlipCommand where HasImageFlip
                = [builtin::ImageFlipCommand::new(command::Flip::On)];
            /// Enables horizontal image mirroring.
            plain [ImageFlipHorizontal] enable_horizontal_flip() -> builtin::ImageMirrorCommand
                where HasImageMirror
                = [builtin::ImageMirrorCommand::new(true)];
            /// Disables horizontal image mirroring.
            plain [ImageFlipHorizontalOff] disable_horizontal_flip()
                -> builtin::ImageMirrorCommand where HasImageMirror
                = [builtin::ImageMirrorCommand::new(false)];
            /// Sets the combined image-flip mode to both axes.
            ///
            /// This sends the combined flip opcode, so it carries the same
            /// `HasCombinedImageFlip` bound as [`Self::set_flip_mode`].
            plain [ImageFlipBoth] set_flip_both() -> command::ImageFlipCombinedCommand
                where HasCombinedImageFlip
                = [command::ImageFlipCombinedCommand::new(command::ImageFlipMode::Both)];
            /// Sets the combined image-flip mode.
            plain [ImageFlipCombined] set_flip_mode(mode: command::ImageFlipMode)
                -> command::ImageFlipCombinedCommand where HasCombinedImageFlip
                = [command::ImageFlipCombinedCommand::new(mode)];
            /// Freezes the image on profiles with explicitly validated support.
            plain [ImageFreezeOn] freeze_on() -> command::ImageFreeze where HasImageFreeze
                = [command::ImageFreeze::on()];
            /// Resumes live image output on profiles with explicitly validated support.
            plain [ImageFreezeOff] freeze_off() -> command::ImageFreeze where HasImageFreeze
                = [command::ImageFreeze::off()];
            /// Returns vertical image-flip state (`09 04 66`); horizontal is false.
            inquiry [] flip() -> command::FlipState where HasImageFlip
                = [command::ImageFlipInquiry];
            /// Returns the combined image-flip mode.
            inquiry [] flip_mode() -> command::FlipState where HasCombinedImageFlip
                = [command::FlipStateInquiry];
            /// Returns picture-effect mode.
            inquiry [] picture_effect()
                -> command::PictureEffectMode where HasPictureEffect
                = [command::PictureEffectInquiry];
            /// Sets picture-effect mode.
            plain [PictureEffect] set_picture_effect(mode: command::PictureEffectMode)
                -> command::PictureEffectCommand where HasPictureEffect
                = [command::PictureEffectCommand::new(mode)];


            /// Sony FR7 master black up (R7).
            plain [Fr7MasterBlackUp] master_black_up(steps: types::BlackStep) -> command::Fr7MasterBlack
                where HasSonyFr7Black
                = [command::Fr7MasterBlack::Up(steps)];
            /// Sony FR7 master black down (R7).
            plain [Fr7MasterBlackDown] master_black_down(steps: types::BlackStep) -> command::Fr7MasterBlack
                where HasSonyFr7Black
                = [command::Fr7MasterBlack::Down(steps)];
            /// Sony FR7 set master black (R7).
            plain [Fr7MasterBlackDirect] set_master_black(value: types::BlackLevel) -> command::Fr7MasterBlack
                where HasSonyFr7Black
                = [command::Fr7MasterBlack::Direct(value)];
            /// Reads Sony FR7 master black (R7).
            inquiry [] master_black() -> types::BlackLevel
                where HasSonyFr7Black
                = [command::Fr7MasterBlackInquiry];
            /// Sony FR7 red black up (R7).
            plain [Fr7RedBlackUp] red_black_up(steps: types::BlackStep) -> command::Fr7RedBlack
                where HasSonyFr7Black
                = [command::Fr7RedBlack::Up(steps)];
            /// Sony FR7 red black down (R7).
            plain [Fr7RedBlackDown] red_black_down(steps: types::BlackStep) -> command::Fr7RedBlack
                where HasSonyFr7Black
                = [command::Fr7RedBlack::Down(steps)];
            /// Sony FR7 set red black (R7).
            plain [Fr7RedBlackDirect] set_red_black(value: types::BlackLevel) -> command::Fr7RedBlack
                where HasSonyFr7Black
                = [command::Fr7RedBlack::Direct(value)];
            /// Reads Sony FR7 red black (R7).
            inquiry [] red_black() -> types::BlackLevel
                where HasSonyFr7Black
                = [command::Fr7RedBlackInquiry];
            /// Sony FR7 blue black up (R7).
            plain [Fr7BlueBlackUp] blue_black_up(steps: types::BlackStep) -> command::Fr7BlueBlack
                where HasSonyFr7Black
                = [command::Fr7BlueBlack::Up(steps)];
            /// Sony FR7 blue black down (R7).
            plain [Fr7BlueBlackDown] blue_black_down(steps: types::BlackStep) -> command::Fr7BlueBlack
                where HasSonyFr7Black
                = [command::Fr7BlueBlack::Down(steps)];
            /// Sony FR7 set blue black (R7).
            plain [Fr7BlueBlackDirect] set_blue_black(value: types::BlackLevel) -> command::Fr7BlueBlack
                where HasSonyFr7Black
                = [command::Fr7BlueBlack::Direct(value)];
            /// Reads Sony FR7 blue black (R7).
            inquiry [] blue_black() -> types::BlackLevel
                where HasSonyFr7Black
                = [command::Fr7BlueBlackInquiry];
            /// Sony FR7 set detail (R7).
            plain [Fr7DetailSetting] set_detail(enabled: bool) -> command::Fr7DetailSetting
                where HasSonyFr7Detail
                = [command::Fr7DetailSetting::new(enabled)];
            /// Reads Sony FR7 detail (R7).
            inquiry [] detail() -> bool
                where HasSonyFr7Detail
                = [command::Fr7DetailInquiry];
            /// Sony FR7 set knee (R7).
            plain [Fr7KneeSetting] set_knee(enabled: bool) -> command::Fr7KneeSetting
                where HasSonyFr7Knee
                = [command::Fr7KneeSetting::new(enabled)];
            /// Reads Sony FR7 knee (R7).
            inquiry [] knee() -> bool
                where HasSonyFr7Knee
                = [command::Fr7KneeInquiry];
            /// Sony FR7 set knee mode (R7).
            plain [Fr7SetKneeMode] set_knee_mode(value: command::Fr7KneeMode) -> command::Fr7SetKneeMode
                where HasSonyFr7Knee
                = [command::Fr7SetKneeMode::new(value)];
            /// Reads Sony FR7 knee mode (R7).
            inquiry [] knee_mode() -> command::Fr7KneeMode
                where HasSonyFr7Knee
                = [command::Fr7KneeModeInquiry];
            /// Sony FR7 set knee slope (R7).
            plain [Fr7SetKneeSlope] set_knee_slope(value: types::KneeSlope) -> command::Fr7SetKneeSlope
                where HasSonyFr7Knee
                = [command::Fr7SetKneeSlope::new(value)];
            /// Reads Sony FR7 knee slope (R7).
            inquiry [] knee_slope() -> types::KneeSlope
                where HasSonyFr7Knee
                = [command::Fr7KneeSlopeInquiry];
            /// Sony FR7 set knee point (R7).
            plain [Fr7SetKneePoint] set_knee_point(value: types::KneePoint) -> command::Fr7SetKneePoint
                where HasSonyFr7Knee
                = [command::Fr7SetKneePoint::new(value)];
            /// Reads Sony FR7 knee point (R7).
            inquiry [] knee_point() -> types::KneePoint
                where HasSonyFr7Knee
                = [command::Fr7KneePointInquiry];
            /// Reads Sony FR7 knee permission (R7).
            inquiry [] knee_permission(parameter: command::Fr7KneeParameter) -> command::Fr7KneePermission
                where HasSonyFr7Knee
                = [command::Fr7KneePermissionInquiry::new(parameter)];

            @noun Tally {
                accessor: TallyAccessor,
                getter: tally,
                dyn_trait: DynTally,
                gate: [typed HasTally],
                doc: "Tally-light commands and inquiries.",
            };
            /// Turns the red tally on.
            plain [TallyRedOn] red_on() -> command::TallyRedOn = [command::TallyRedOn::new()];
            /// Turns the red tally off.
            plain [TallyRedOff] red_off() -> command::TallyRedOff = [command::TallyRedOff::new()];
            /// Sets low tally brightness on profiles with validated support.
            plain [TallyBrightLow] bright_lo() -> command::TallyBrightLo
                where HasTallyBrightness = [command::TallyBrightLo::new()];
            /// Sets high tally brightness on profiles with validated support.
            plain [TallyBrightHigh] bright_hi() -> command::TallyBrightHi
                where HasTallyBrightness = [command::TallyBrightHi::new()];
            /// Turns the green tally on.
            plain [TallyGreenOn] green_on() -> command::TallyGreenOn
                = [command::TallyGreenOn::new()];
            /// Turns the green tally off.
            plain [TallyGreenOff] green_off() -> command::TallyGreenOff
                = [command::TallyGreenOff::new()];
            /// Sets PTZOptics tally flash mode on profiles with validated support.
            plain [TallyFlash] flash() -> command::TallyFlash where HasPtzOpticsTally
                = [command::TallyFlash::new()];
            /// Sets PTZOptics tally solid-on mode on profiles with validated support.
            plain [TallyOn] on() -> command::TallyOn where HasPtzOpticsTally
                = [command::TallyOn::new()];
            /// Turns PTZOptics tally output off on profiles with validated support.
            plain [TallyOff] off() -> command::TallyOff where HasPtzOpticsTally
                = [command::TallyOff::new()];
            /// Returns red tally state.
            inquiry [] red_status() -> bool = [command::TallyRedInquiry];
            /// Returns green tally state.
            inquiry [] green_status() -> bool = [command::TallyGreenInquiry];

            @noun NdFilter {
                accessor: NdFilterAccessor,
                getter: nd_filter,
                dyn_trait: DynNdFilter,
                gate: [typed HasNdFilter],
                doc: "Neutral-density filter commands and inquiries.",
            };
            /// Returns the Sony FR7 variable ND-filter density code (`00..=14`).
            inquiry [] position() -> command::NdFilterValue
                = [command::NdFilterInquiry];
            /// Returns the current ND-filter preset.
            inquiry [] preset() -> types::NdFilterPreset
                = [command::NdFilterPresetInquiry];
            /// Selects preset or variable ND-filter mode.
            plain [NdFilterMode] set_mode(mode: command::NdFilterMode)
                -> command::NdFilterModeCommand = [command::NdFilterModeCommand::new(mode)];
            /// Sets a direct variable ND-filter value.
            ///
            /// Build the value with [`NdFilterValue::new`](command::NdFilterValue::new)
            /// from a raw density or
            /// [`NdFilterValue::from_stops`](command::NdFilterValue::from_stops)
            /// from photographic stops.
            targeted [NdFilterDirect] set_value(value: command::NdFilterValue)
                -> builtin::NdFilterDirect = [builtin::NdFilterDirect::new(value)];
            /// Increases ND-filter density by one step.
            targeted [NdFilterStepUp] step_up() -> builtin::NdFilterStepUp
                = [builtin::NdFilterStepUp::new()];
            /// Decreases ND-filter density by one step.
            targeted [NdFilterStepDown] step_down() -> builtin::NdFilterStepDown
                = [builtin::NdFilterStepDown::new()];
            /// Enables automatic ND filtering.
            plain [NdFilterAutoOn] auto_on() -> command::AutoNdCommand
                = [command::AutoNdCommand::new(true)];
            /// Disables automatic ND filtering.
            plain [NdFilterAutoOff] auto_off() -> command::AutoNdCommand
                = [command::AutoNdCommand::new(false)];

            @noun MotionSync {
                accessor: MotionSyncAccessor,
                getter: motion_sync,
                dyn_trait: DynMotionSync,
                gate: [typed HasMotionSync],
                doc: "Motion-sync commands and inquiries.",
            };
            /// Returns the motion-sync mode.
            inquiry [] mode() -> command::MotionSyncMode
                = [command::MotionSyncModeInquiry];
            /// Returns the motion-sync preset speed.
            inquiry [] preset() -> command::MotionSyncPreset
                = [command::MotionSyncPresetInquiry];
            /// Enables or disables motion synchronization.
            plain [MotionSyncMode] set_mode(mode: command::MotionSyncMode)
                -> command::SetMotionSyncMode
                = [command::SetMotionSyncMode::new(mode)];
            /// Sets the motion-sync speed.
            ///
            /// [`MotionSyncSpeed`](crate::types::MotionSyncSpeed) owns the
            /// `1..=24` range, and `MotionSyncSpeed::from(MotionSyncPreset)`
            /// maps the slow/normal/fast presets.
            plain [MotionSyncPreset] set_speed(speed: types::MotionSyncSpeed)
                -> command::SetMotionSyncPreset
                = [command::SetMotionSyncPreset::new(speed)];

            @noun Menu {
                accessor: MenuAccessor,
                getter: menu,
                dyn_trait: DynMenu,
                gate: [domain HasMenuControl],
                doc: "On-screen menu display and navigation commands and the menu-status inquiry.",
            };
            /// Returns whether the on-screen menu is open.
            inquiry [] status() -> bool = [command::MenuOpenCloseInquiry];
            /// Displays or hides the on-screen menu.
            plain [MenuDisplay] display(on: bool) -> command::SetMenuDisplay
                = [command::SetMenuDisplay::new(on)];
            /// Moves the menu cursor with a pan/tilt drive.
            ///
            /// # Safety
            ///
            /// If the menu is closed, this starts continuous physical pan/tilt
            /// movement. Send an explicit pan/tilt STOP or call
            /// `stop_all_motion()` to stop it. The applied handle confirms
            /// acceptance, not that physical movement has stopped.
            applied [MenuNavigate] navigate(direction: command::MenuDirection)
                -> command::MenuNavigate
                = [command::MenuNavigate::new(direction)];
            /// Selects the current menu item.
            plain [MenuSelect] select() -> command::PerformMenuAction
                = [command::PerformMenuAction::new(command::MenuAction::Select)];
            /// Cancels or returns from the current menu item.
            plain [MenuCancel] cancel() -> command::PerformMenuAction
                = [command::PerformMenuAction::new(command::MenuAction::Cancel)];
            /// Sends a typed Sony FR7 direct-menu button control.
            plain [DirectMenu] direct(target: command::DirectMenuTarget, button: command::MenuButton)
                -> command::DirectMenuControl where HasDirectMenuControl
                = [command::DirectMenuControl::new(target, button)];
            /// Toggles the on-screen menu open or closed.
            ///
            /// This is the vendor open/close direct control, so it needs no prior
            /// [`Self::status`] round trip to decide which way to move.
            plain [] toggle_display() -> command::DirectMenuControl where HasDirectMenuControl
                = [command::DirectMenuControl::open_close()];

            @noun Advanced {
                accessor: AdvancedAccessor,
                getter: advanced,
                dyn_trait: DynAdvanced,
                gate: [always],
                doc: "Streaming, USB-audio, variable-speed, and vendor status commands and inquiries.",
            };






            /// Returns USB-audio state.
            inquiry [] usb_audio_enabled() -> bool where HasUsbAudio
                = [command::UsbAudioInquiry];


            /// Enables multicast streaming.
            plain [MulticastStreamingOn] multicast_on() -> command::MulticastStreaming
                where HasPtzOpticsMulticastStreaming
                = [command::MulticastStreaming::On];
            /// Disables multicast streaming.
            plain [MulticastStreamingOff] multicast_off() -> command::MulticastStreaming
                where HasPtzOpticsMulticastStreaming
                = [command::MulticastStreaming::Off];
            /// Sets NDI streaming quality.
            plain [NdiQuality] set_ndi_quality(quality: types::NdiQuality)
                -> command::SetNdiQuality
                where HasPtzOpticsNdiQuality
                = [command::SetNdiQuality::new(quality)];
            /// Enables USB audio.
            plain [UsbAudioOn] usb_audio_on() -> command::UsbAudio where HasUsbAudio
                = [command::UsbAudio::On];
            /// Disables USB audio.
            plain [UsbAudioOff] usb_audio_off() -> command::UsbAudio where HasUsbAudio
                = [command::UsbAudio::Off];
            /// Sets pan/tilt variable-speed mode.
            plain [VariableSpeedMode] set_variable_speed_mode(mode: command::VariableSpeedMode)
                -> command::SetVariableSpeedMode where HasVariableSpeed
                = [command::SetVariableSpeedMode::new(mode)];

            @exceptions;
            broadcast [AddressSet] address_set -> command::system::AddressSetCommand;
            broadcast [InterfaceClear] interface_clear -> command::system::InterfaceClearCommand;
            internal [CommandCancel] cancel_command -> command::system::CommandCancelCommand;
        }
    };
}

pub(crate) use noun_table;

/// Hands the three motion-view methods to a consumer macro (#816).
///
/// Optional fixed-shape leading arguments
/// (`motion_table!(my_consumer, [async], [.await], CoreType)`) reach the
/// consumer as single token trees before the `@motion` header.
///
/// Each row is `fn <name>(&self[, <arg>: <type>]) -> <value> =>
/// <core method>(<core arguments>);`: the public method, the value its
/// `Result` carries, and the owner-core call that implements it. Every facade
/// — the blocking and async `MotionAccessor` and the object-safe `DynMotion` —
/// expands these rows, so all of them expose the same names, arities and
/// contracts.
#[cfg(any(feature = "blocking", feature = "async"))]
macro_rules! motion_table {
    ($consumer:ident $(, $arg:tt)*) => {
        $consumer! {
            $($arg)*
            @motion {
                accessor: MotionAccessor,
                getter: motion,
                dyn_trait: DynMotion,
                doc: "Motion safety and observation: stopping all motion, movement queries, and idle waits.",
            };

            /// Orders a halt of supported pan/tilt, zoom, and focus movement.
            ///
            /// The owner fences older declared motion at acceptance and
            /// dispatches one STOP per supported axis under a common deadline,
            /// respecting protocol gates. Inspect each supported axis in the
            /// returned report: an applied STOP is protocol application, not
            /// proof of physical rest. A STOP the camera refuses is reported
            /// promptly and not resent: for example a PTZOptics G2 in
            /// auto-focus mode answers the focus STOP with
            /// [`Error::CommandNotExecutable`](crate::Error::CommandNotExecutable),
            /// so `focus` is `Failed` while the lens is under auto-focus
            /// control.
            fn stop_all_motion(&self) -> crate::HaltReport => stop_all_motion();

            /// Reports whether two protocol position samples, separated by at
            /// least [`MotionQuery::window`](crate::camera::MotionQuery::window),
            /// indicate movement on the selected axes.
            ///
            /// [`MotionQuery::default()`](crate::camera::MotionQuery::default)
            /// samples [`AffectedAxes::MOVEMENT`](crate::AffectedAxes::MOVEMENT)
            /// with the default tolerance and observation window. `false` means
            /// no movement was detected over the window, not that the camera is
            /// physically at rest. A zero window fails with
            /// [`Error::InvalidParameter`](crate::Error::InvalidParameter)
            /// before any inquiry, and a window that cannot elapse within the
            /// observation deadline fails with
            /// [`Error::Timeout`](crate::Error::Timeout) rather than reporting
            /// no movement.
            fn is_moving(&self, query: crate::camera::MotionQuery) -> bool => is_moving(query);

            /// Waits until the selected axes meet the protocol idle condition:
            /// two consecutive position samples, taken every
            /// [`IdleWait::interval`](crate::camera::IdleWait::interval),
            /// agree within tolerance before
            /// [`IdleWait::timeout`](crate::camera::IdleWait::timeout)
            /// elapses, which otherwise fails with
            /// [`Error::Timeout`](crate::Error::Timeout).
            fn wait_until_idle(&self, wait: crate::camera::IdleWait) -> () => wait_until_idle(wait);
        }
    };
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use motion_table;

/// Re-exports one name per noun header from a facade module.
///
/// The noun table names every accessor and `Dyn*` trait once, in its headers;
/// the crate root, the facade modules and the preludes re-export them through
/// this consumer instead of restating the list.  Invoke it as
/// `noun_table!(reexport_nouns, [accessor], [crate::async_nouns])` or with
/// `[dyn_trait]`, and `motion_table!(reexport_nouns, [accessor], [..])` for
/// the motion view.
#[cfg(any(feature = "blocking", feature = "async"))]
macro_rules! reexport_nouns {
    (@use [accessor] [$($path:tt)*] $([$accessor:ident $dyn_trait:ident])*) => {
        pub use $($path)*::{$($accessor),*};
    };
    (@use [dyn_trait] [$($path:tt)*] $([$accessor:ident $dyn_trait:ident])*) => {
        pub use $($path)*::{$($dyn_trait),*};
    };

    ($name:tt $path:tt
        @motion {
            accessor: $accessor:ident,
            getter: $getter:ident,
            dyn_trait: $dyn_trait:ident,
            doc: $doc:literal $(,)?
        };
        $($rows:tt)*
    ) => {
        $crate::noun_table::reexport_nouns!(@use $name $path [$accessor $dyn_trait]);
    };

    ($name:tt $path:tt
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                dyn_trait: $dyn_trait:ident,
                gate: $gate:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $rgate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        $crate::noun_table::reexport_nouns!(@use $name $path $([$accessor $dyn_trait])*);
    };
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use reexport_nouns;
