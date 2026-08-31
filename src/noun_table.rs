//! The typed noun registry behind the static surface metadata and all three
//! noun facades.
//!
//! [`crate::command::surface`], [`crate::async_nouns`],
//! [`crate::blocking::nouns`] and [`crate::dynapi::nouns`] are generated from
//! this registry.  Every noun method is written here once — name, arguments,
//! capability bound, return class and rustdoc — so the type system relates
//! the copies.
//!
//! [`noun_table!`] is a continuation-passing macro in the same style as
//! [`crate::command::inquiry_structs::builtin_inquiry_table`]: it takes the
//! name of a consumer macro and hands that consumer the rows of one noun.
//! Each facade defines its own consumer, so the *differences* between the
//! facades (async `fn` plus `.await`, blocking `fn`, object-safe `dyn`
//! variants, different receiver and session plumbing, different cfg features)
//! live in the consumers, while everything that must agree lives here.
//!
//! # Row grammar
//!
//! ```text
//! <kind> [<BuiltinCommand>, ...] <method>(<arg>: <type>, ...)
//!     [-> <request type>] [where <Marker> [+ <Marker>]] = <request>;
//! ```
//!
//! * `<kind>` is `inquiry`, `plain`, `applied` or `targeted` — the semantic
//!   return class the ledger records for the row.  Every non-inquiry row has an
//!   explicit list of canonical [`BuiltinCommand`] variants in brackets and a
//!   concrete request type after `->`.  An `inquiry` row instead spells its
//!   concrete inquiry command type before the method and its decoded response
//!   type after `->`.
//! * An empty command list (`[]`) is intentional: it marks a convenience row
//!   that delegates to or derives a canonical row and therefore owns no
//!   command ID.  Delegate rows use the explicit `delegate` request form.
//! * `where <Marker>` is the compile-time capability bound the static facades
//!   put on their profile parameter.  It is a bare marker-trait name; the
//!   parity gate checks that every facade resolves that name to the real
//!   `crate::capabilities` trait.  The erased facade carries no bounds at all
//!   (its capability enforcement is the runtime `validate_for_profile` check),
//!   so its consumers ignore this clause.
//! * `<request>` is the command or inquiry value to send.  Five forms exist:
//!   * `<expr>` — an infallible constructor.
//!   * `checked <expr>` — a fallible constructor returning `Result<_>`.
//!   * `with_profile |<name>| <expr>` — a fallible constructor that needs the
//!     session's profile; the binder names it.
//!   * `with_core |<name>| <expr>` — a fallible constructor that needs the
//!     owner core.
//!   * `delegate <method>(<arg>, ...)` — the row is a convenience wrapper that
//!     forwards to another row on the same noun with a fixed argument.
//!
//! Rustdoc is a per-row attribute, so the three surfaces cannot document the
//! same method differently.  The `@noun` marker is an internal callback
//! context used by the all-rows projection; ordinary consumers strip it before
//! expanding their rows.
//!
//! # What is *not* in the table
//!
//! `MotionAccessor`/`DynMotion` is a safety and observation view rather than a
//! ledger noun: its four methods — `stop_all_motion`, `is_moving`,
//! `is_moving_axes` and `wait_until_idle` — reach the owner core directly and
//! share no shape with a command row.  Those four stay hand-written on each
//! facade and are named in the test-only [`crate::noun_parity`] checks, which
//! compare them across the three surfaces.

// With no facade feature selected there is no consumer for this table: the
// blocking, async and dyn-api facades are the only three, and CI's
// `no-default pure engine/domain` leg switches all of them off.  The table is
// still parsed and still has to stay well formed in that configuration, so the
// exemption is targeted at exactly that leg rather than left unconditional.
#![cfg_attr(
    not(any(feature = "blocking", feature = "async", feature = "dyn-api")),
    allow(unused_macros, unused_imports)
)]

/// Hands one noun's rows to a consumer macro.
///
/// See the module documentation for the row grammar.  Invoke it as
/// `noun_table!(Zoom => my_consumer);`.
macro_rules! noun_table {
    // The flat projection deliberately reuses every noun arm instead of
    // carrying a second command list.  Consumers that need noun context (the
    // static surface registry) handle the internal `@noun` marker; the public
    // facade consumers strip it in one forwarding arm.
    (All => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Power Zoom System PanTilt Focus Presets
            Exposure WhiteBalance Image Tally NdFilter MotionSync Menu Advanced @exceptions }
    };

    (Power => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Power }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Power $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Power;
            /// Returns the camera's current power state.
            inquiry command::PowerInquiry state() -> bool = command::PowerInquiry;
            /// Powers the camera on.
            plain [PowerOn] on() -> command::PowerOn = command::PowerOn::new();
            /// Places the camera in standby.
            plain [PowerStandby] off() -> command::PowerStandby = command::PowerStandby::new();
        ]; $($rest)* }
    };

    (Zoom => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Zoom }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Zoom $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Zoom;
            /// Returns the current optical/digital zoom position.
            inquiry command::ZoomPositionInquiry position() -> types::ZoomPosition
                = command::ZoomPositionInquiry;
            /// Drives toward telephoto at standard speed.
            applied [ZoomTele] tele() -> builtin::ZoomDrive = builtin::ZoomDrive::Tele;
            /// Drives toward wide angle at standard speed.
            applied [ZoomWide] wide() -> builtin::ZoomDrive = builtin::ZoomDrive::Wide;
            /// Stops zoom movement.
            applied [ZoomStop] stop() -> builtin::ZoomStop = builtin::ZoomStop;
            /// Drives toward telephoto at a validated variable speed.
            applied [ZoomTeleVariable] tele_variable(speed: types::ZoomSpeed)
                -> builtin::ZoomDrive
                = builtin::ZoomDrive::TeleVariable(speed);
            /// Drives toward wide angle at a validated variable speed.
            applied [ZoomWideVariable] wide_variable(speed: types::ZoomSpeed)
                -> builtin::ZoomDrive
                = builtin::ZoomDrive::WideVariable(speed);
            /// Moves to an absolute zoom position.
            targeted [ZoomPosition] set_position(position: types::ZoomPosition)
                -> builtin::ZoomTarget where HasDirectZoom
                = builtin::ZoomTarget::new(position);
            /// Moves to a normalized position across the optical zoom range.
            ///
            /// `0.0` is the wide end and `1.0` the telephoto end of the profile's
            /// documented optical range.
            targeted [] set_normalized(position: UnitInterval) -> builtin::ZoomTarget
                where HasDirectZoom
                = with_profile |profile| builtin::ZoomTarget::from_normalized(
                    position,
                    ZoomDomain::Optical,
                    profile,
                );
            /// Moves to a normalized position across a documented zoom domain.
            ///
            /// [`ZoomDomain::OpticalPlusDigital`] requires the profile to document a
            /// digital maximum and never falls back to the optical range.
            targeted [] set_normalized_in_domain(position: UnitInterval, domain: ZoomDomain)
                -> builtin::ZoomTarget where HasDirectZoom
                = with_profile |profile| builtin::ZoomTarget::from_normalized(
                    position,
                    domain,
                    profile,
                );
            /// Enables or disables digital zoom.
            plain [DigitalZoom] set_digital_zoom(enabled: bool) -> command::DigitalZoom
                where HasDigitalZoomToggle
                = command::DigitalZoom::new(enabled);
        ]; $($rest)* }
    };

    (System => $consumer:ident) => {
        noun_table! { @collect $consumer; []; System }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; System $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun System;
            /// Returns the camera firmware/version information.
            inquiry command::VersionInquiry version() -> command::VersionInfo
                = command::VersionInquiry;
            /// Saves the camera's current settings to non-volatile storage.
            plain [SettingsSave] save_settings() -> command::SettingsSaveCommand
                where HasPtzOpticsSettingsSave
                = command::SettingsSaveCommand::new();
        ]; $($rest)* }
    };

    (PanTilt => $consumer:ident) => {
        noun_table! { @collect $consumer; []; PanTilt }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; PanTilt $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun PanTilt;
            /// Returns the current pan/tilt position.
            inquiry command::PanTiltPositionInquiry position() -> crate::camera::PanTiltPosition
                = command::PanTiltPositionInquiry;
            /// Moves the pan/tilt mechanism to its home position.
            targeted [PanTiltHome] home() -> builtin::PanTiltHome = builtin::PanTiltHome;
            /// Resets the pan/tilt mechanism.
            targeted [PanTiltReset] reset() -> builtin::PanTiltReset = builtin::PanTiltReset;
            /// Starts a directional pan/tilt drive.
            applied [PanTiltDrive] move_direction(
                direction: command::PanTiltDirection,
                pan_speed: types::PanSpeed,
                tilt_speed: types::TiltSpeed
            ) -> builtin::PanTiltDrive = checked builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed);
            /// Starts an upward pan/tilt drive.
            applied [] up(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Up, pan_speed, tilt_speed);
            /// Starts a downward pan/tilt drive.
            applied [] down(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Down, pan_speed, tilt_speed);
            /// Starts a leftward pan/tilt drive.
            applied [] left(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Left, pan_speed, tilt_speed);
            /// Starts a rightward pan/tilt drive.
            applied [] right(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Right, pan_speed, tilt_speed);
            /// Stops pan/tilt movement using profile-safe stop speeds.
            applied [PanTiltStop] stop() -> builtin::PanTiltStop
                = with_core |core| core.pan_tilt_stop_request();
            /// Moves to an absolute degree position at the selected speed.
            targeted [PanTiltAbsolute] absolute(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                -> builtin::PanTiltAbsolute
                = with_profile |profile| builtin::PanTiltAbsolute::for_profile_speed_level(
                    pan, tilt, speed, profile,
                );
            /// Moves by a relative degree offset at the selected speed.
            targeted [PanTiltRelative] relative(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                -> builtin::PanTiltRelative
                = with_profile |profile| builtin::PanTiltRelative::for_profile_speed_level(
                    pan, tilt, speed, profile,
                );
            /// Sets one pan/tilt movement-limit corner.
            plain [PanTiltLimitSet] limit_set(
                corner: command::PanTiltLimitCorner,
                pan: Degrees<f32>,
                tilt: Degrees<f32>
            ) -> builtin::PanTiltLimitSet = with_profile |profile| builtin::PanTiltLimitSet::for_profile(
                corner,
                pan,
                tilt,
                profile,
            );
            /// Clears one pan/tilt movement-limit corner.
            plain [PanTiltLimitClear] limit_clear(corner: command::PanTiltLimitCorner)
                -> builtin::PanTiltLimitClear
                = with_profile |profile| builtin::PanTiltLimitClear::for_profile(corner, profile);
        ]; $($rest)* }
    };

    (Focus => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Focus }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Focus $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Focus;
            /// Returns the current focus position.
            inquiry command::FocusPositionInquiry position() -> types::FocusPosition
                = command::FocusPositionInquiry;
            /// Returns the current focus mode.
            inquiry command::FocusModeInquiry mode() -> command::FocusMode
                = command::FocusModeInquiry;
            /// Drives focus farther at standard speed.
            applied [FocusFar] far() -> builtin::FocusDrive = builtin::FocusDrive::Far;
            /// Drives focus nearer at standard speed.
            applied [FocusNear] near() -> builtin::FocusDrive = builtin::FocusDrive::Near;
            /// Drives focus farther at a variable speed.
            applied [FocusFarVariable] far_variable(speed: command::FocusSpeed)
                -> builtin::FocusDrive
                = builtin::FocusDrive::FarVariable(speed);
            /// Drives focus nearer at a variable speed.
            applied [FocusNearVariable] near_variable(speed: command::FocusSpeed)
                -> builtin::FocusDrive
                = builtin::FocusDrive::NearVariable(speed);
            /// Stops focus movement.
            applied [FocusStop] stop() -> builtin::FocusStop = builtin::FocusStop;
            /// Moves focus to an absolute position.
            targeted [FocusPosition] set_position(position: types::FocusPosition)
                -> builtin::FocusTarget
                = builtin::FocusTarget::new(position);
            /// Enables automatic focus mode.
            plain [FocusAuto] auto() -> builtin::FocusModeCommand
                = builtin::FocusModeCommand::Auto;
            /// Enables manual focus mode.
            plain [FocusManual] manual() -> builtin::FocusModeCommand
                = builtin::FocusModeCommand::Manual;
            /// Triggers one-push autofocus.
            applied [FocusOnePush] one_push() -> builtin::FocusTrigger where HasOnePushFocus
                = builtin::FocusTrigger::OnePush;
            /// Moves focus to infinity.
            targeted [FocusInfinity] infinity() -> builtin::FocusInfinity
                = builtin::FocusInfinity;
            /// Toggles automatic/manual focus mode.
            plain [FocusToggle] toggle() -> builtin::FocusModeCommand
                = builtin::FocusModeCommand::Toggle;
            /// Triggers vendor snap focus.
            applied [FocusSnap] snap() -> builtin::FocusTrigger where HasPtzOpticsSnapFocus
                = builtin::FocusTrigger::Snap;
            /// Selects a focus zone.
            plain [FocusZone] set_zone(zone: command::FocusZone)
                -> command::FocusZoneCommand where HasFocusZone
                = command::FocusZoneCommand::new(zone);
            /// Sets the autofocus sensitivity.
            plain [FocusAutoSensitivity] set_sensitivity(sensitivity: command::AutoFocusSensitivity)
                -> command::AutoFocusSensitivityCommand where HasAutoFocusSensitivity
                = command::AutoFocusSensitivityCommand::new(sensitivity);
            /// Sets the minimum focus distance.
            plain [FocusNearLimit] set_near_limit(position: types::FocusPosition)
                -> command::FocusNearLimitCommand where HasFocusNearLimitInquiry
                = command::FocusNearLimitCommand::new(position);
            /// Sets the focus-lock mode.
            plain [FocusLock] set_lock(mode: command::FocusLock) -> command::FocusLock
                where HasFocusLock = mode;
            /// Presses the vendor Push-AF control.
            applied [PushAfPress] push_af_press() -> builtin::PushAfPress where HasPushAutoFocus
                = builtin::PushAfPress::new();
            /// Releases the vendor Push-AF control.
            applied [PushAfRelease] push_af_release() -> builtin::PushAfRelease
                where HasPushAutoFocus = builtin::PushAfRelease::new();
            /// Returns the configured focus near limit.
            inquiry command::FocusNearLimitInquiry near_limit() -> types::FocusPosition
                where HasFocusNearLimitInquiry
                = command::FocusNearLimitInquiry;
            /// Returns the configured focus zone.
            inquiry command::FocusZoneInquiry zone() -> command::FocusZone where HasFocusZoneInquiry
                = command::FocusZoneInquiry;
            /// Returns the autofocus sensitivity.
            inquiry command::AutoFocusSensitivityInquiry sensitivity()
                -> command::AutoFocusSensitivity where HasAutoFocusSensitivity
                = command::AutoFocusSensitivityInquiry;
            /// Returns the configured focus range.
            inquiry command::FocusRangeInquiry range() -> command::FocusRange
                = command::FocusRangeInquiry;
        ]; $($rest)* }
    };

    (Presets => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Presets }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Presets $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Presets;
            /// Recalls a stored preset as a targeted operation.
            targeted [PresetRecall] recall(preset: command::PresetNumber)
                -> builtin::PresetRecall
                = with_profile |profile| builtin::PresetRecall::for_profile(preset, profile);
            /// Sets the preset-recall speed.
            plain [PresetRecallSpeed] set_recall_speed(speed: command::PresetRecallSpeed)
                -> command::PresetRecallSpeedCommand
                where HasPtzOpticsPresetRecallSpeed
                = command::PresetRecallSpeedCommand::new(speed);
            /// Stores the current camera state in a preset.
            plain [PresetSet] set(preset: command::PresetNumber) -> builtin::PresetSet
                = builtin::PresetSet::new(preset);
            /// Clears a stored preset.
            plain [PresetReset] reset(preset: command::PresetNumber) -> builtin::PresetReset
                = builtin::PresetReset::new(preset);
        ]; $($rest)* }
    };

    (Exposure => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Exposure }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Exposure $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Exposure;
            /// Returns the active exposure mode.
            inquiry command::ExposureModeInquiry mode() -> command::ExposureMode
                = command::ExposureModeInquiry;
            /// Sets the exposure mode.
            plain [ExposureMode] set_mode(mode: command::ExposureMode)
                -> command::ExposureCommand = command::ExposureCommand::new(mode);
            /// Returns the shutter speed.
            inquiry command::ShutterInquiry shutter() -> types::ShutterSpeed
                = command::ShutterInquiry;
            /// Restores the camera's shutter default.
            plain [ShutterReset] shutter_reset() -> command::Shutter = command::Shutter::Reset;
            /// Increases shutter speed by one camera-defined step.
            plain [ShutterUp] shutter_up() -> command::Shutter = command::Shutter::Up;
            /// Decreases shutter speed by one camera-defined step.
            plain [ShutterDown] shutter_down() -> command::Shutter = command::Shutter::Down;
            /// Sets an explicit shutter speed.
            plain [ShutterDirect] shutter_direct(speed: types::ShutterSpeed)
                -> command::Shutter = command::Shutter::SetSpeed(speed);
            /// Returns the exposure compensation value.
            inquiry command::ExposureCompensationInquiry compensation()
                -> types::ExposureCompensationLevel
                where HasExposureCompensation
                = command::ExposureCompensationInquiry;
            /// Returns whether exposure compensation is enabled.
            inquiry command::ExposureCompensationModeInquiry compensation_enabled() -> bool
                where HasExposureCompensation
                = command::ExposureCompensationModeInquiry;
            /// Returns the camera's exposure compensation position.
            inquiry command::ExposureCompensationPositionInquiry compensation_position()
                -> types::ExposureCompensationPosition
                where HasExposureCompensation
                = command::ExposureCompensationPositionInquiry;
            /// Enables exposure compensation.
            plain [ExposureCompensationOn] compensation_on() -> command::ExposureCompensation
                where HasExposureCompensation
                = command::ExposureCompensation::On;
            /// Disables exposure compensation.
            plain [ExposureCompensationOff] compensation_off() -> command::ExposureCompensation
                where HasExposureCompensation
                = command::ExposureCompensation::Off;
            /// Resets exposure compensation.
            plain [ExposureCompensationReset] compensation_reset()
                -> command::ExposureCompensation where HasExposureCompensation
                = command::ExposureCompensation::Reset;
            /// Increases exposure compensation by one step.
            plain [ExposureCompensationUp] compensation_up() -> command::ExposureCompensation
                where HasExposureCompensation
                = command::ExposureCompensation::Up;
            /// Decreases exposure compensation by one step.
            plain [ExposureCompensationDown] compensation_down()
                -> command::ExposureCompensation where HasExposureCompensation
                = command::ExposureCompensation::Down;
            /// Sets direct exposure compensation.
            plain [ExposureCompensationDirect] compensation_direct(
                level: types::ExposureCompensationLevel
            ) -> command::ExposureCompensation
                where HasExposureCompensation
                = command::ExposureCompensation::SetLevel(level);
            /// Returns the wide-dynamic-range level.
            inquiry command::DynamicRangeInquiry dynamic_range() -> types::DynamicRangeLevel
                where HasWideDynamicRange
                = command::DynamicRangeInquiry;
            /// Sets the wide-dynamic-range level.
            plain [DynamicRange] set_dynamic_range(level: types::DynamicRangeLevel)
                -> command::DynamicRange where HasWideDynamicRange
                = command::DynamicRange::new(level);
            /// Returns whether iris control is automatic.
            inquiry command::IrisControlInquiry iris_control() -> bool where HasIrisControl
                = command::IrisControlInquiry;
            /// Returns the iris level.
            inquiry command::IrisInquiry iris() -> types::IrisLevel where HasIrisControl
                = command::IrisInquiry;
            /// Resets the iris.
            targeted [IrisReset] iris_reset() -> builtin::IrisReset where HasIrisControl
                = builtin::IrisReset::new();
            /// Increases the iris by one step.
            targeted [IrisUp] iris_up() -> builtin::IrisUp where HasIrisControl
                = builtin::IrisUp::new();
            /// Decreases the iris by one step.
            targeted [IrisDown] iris_down() -> builtin::IrisDown where HasIrisControl
                = builtin::IrisDown::new();
            /// Sets a direct iris level.
            targeted [IrisDirect] iris_direct(level: types::IrisLevel)
                -> builtin::IrisDirect where HasIrisControl
                = builtin::IrisDirect::new(level);
            /// Returns exposure brightness.
            inquiry command::BrightnessInquiry brightness() -> types::BrightnessLevel
                where HasBrightnessControl
                = command::BrightnessInquiry;
            /// Resets exposure brightness.
            plain [BrightnessReset] brightness_reset() -> command::Brightness
                where HasBrightnessControl = command::Brightness::Reset;
            /// Increases exposure brightness.
            plain [BrightnessUp] brightness_up() -> command::Brightness
                where HasBrightnessControl = command::Brightness::Up;
            /// Decreases exposure brightness.
            plain [BrightnessDown] brightness_down() -> command::Brightness
                where HasBrightnessControl = command::Brightness::Down;
            /// Sets exposure brightness through the bright-direct command.
            plain [BrightnessSet] brightness_set(level: types::BrightnessLevel)
                -> command::Brightness where HasBrightnessControl
                = command::Brightness::SetLevel(level);
            /// Sets the camera's direct brightness value.
            plain [BrightnessDirect] brightness_direct(level: types::BrightnessLevel)
                -> command::Brightness where HasBrightnessControl
                = command::Brightness::Direct(level);
            /// Returns gain.
            inquiry command::GainInquiry gain() -> types::GainLevel = command::GainInquiry;
            /// Resets gain.
            plain [GainReset] gain_reset() -> command::Gain = command::Gain::Reset;
            /// Increases gain.
            plain [GainUp] gain_up() -> command::Gain = command::Gain::Up;
            /// Decreases gain.
            plain [GainDown] gain_down() -> command::Gain = command::Gain::Down;
            /// Sets direct gain.
            plain [GainDirect] gain_direct(level: types::GainLevel) -> command::Gain
                = command::Gain::SetValue(level);
            /// Returns the configured gain limit.
            inquiry command::GainLimitInquiry gain_limit() -> types::GainLimit
                = command::GainLimitInquiry;
            /// Sets the configured gain limit.
            plain [GainLimit] set_gain_limit(limit: types::GainLimit)
                -> command::GainLimitCommand = command::GainLimitCommand::new(limit);
            /// Sets anti-flicker mode.
            plain [AntiFlicker] set_anti_flicker(mode: command::AntiFlickerMode)
                -> command::AntiFlickerCommand
                where HasPtzOpticsAntiFlicker
                = command::AntiFlickerCommand::new(mode);
            /// Returns the configured anti-flicker mode.
            inquiry command::FlickerModeInquiry flicker_mode() -> command::AntiFlickerMode
                where HasPtzOpticsAntiFlicker
                = command::FlickerModeInquiry;
            /// Enables spotlight mode.
            plain [SpotlightOn] spotlight_on() -> command::SpotlightOn
                where HasSonySpotlight
                = command::SpotlightOn::new();
            /// Disables spotlight mode.
            plain [SpotlightOff] spotlight_off() -> command::SpotlightOff
                where HasSonySpotlight
                = command::SpotlightOff::new();
            /// Enables automatic slow shutter.
            plain [AutoSlowShutterOn] auto_slow_shutter_on() -> command::AutoSlowShutterOn
                where HasSonyAutoSlowShutter
                = command::AutoSlowShutterOn::new();
            /// Disables automatic slow shutter.
            plain [AutoSlowShutterOff] auto_slow_shutter_off() -> command::AutoSlowShutterOff
                where HasSonyAutoSlowShutter
                = command::AutoSlowShutterOff::new();
        ]; $($rest)* }
    };

    (WhiteBalance => $consumer:ident) => {
        noun_table! { @collect $consumer; []; WhiteBalance }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; WhiteBalance $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun WhiteBalance;
            /// Returns the active white-balance mode.
            inquiry command::WhiteBalanceModeInquiry mode() -> command::WhiteBalanceMode
                = command::WhiteBalanceModeInquiry;
            /// Selects automatic white balance.
            plain [WhiteBalanceAuto] auto() -> command::WhiteBalanceCommand
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Auto);
            /// Selects the indoor white-balance preset.
            plain [WhiteBalanceIndoor] indoor() -> command::WhiteBalanceCommand
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Indoor);
            /// Selects the outdoor white-balance preset.
            plain [WhiteBalanceOutdoor] outdoor() -> command::WhiteBalanceCommand
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Outdoor);
            /// Selects one-push white balance.
            plain [WhiteBalanceOnePush] one_push() -> command::WhiteBalanceCommand
                where HasOnePushWhiteBalance
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::OnePush);
            /// Selects auto-tracking white balance.
            plain [WhiteBalanceAutoTracking] atw() -> command::WhiteBalanceCommand
                where HasAutoTrackingWhiteBalance
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ATW);
            /// Selects manual white balance.
            plain [WhiteBalanceManual] manual() -> command::WhiteBalanceCommand
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Manual);
            /// Selects color-temperature white-balance mode.
            plain [WhiteBalanceColorTemperature] color_temperature_mode()
                -> command::WhiteBalanceCommand where HasColorTemperature
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ColorTemperature);
            /// Sets automatic white-balance sensitivity.
            plain [AutoWhiteBalanceSensitivity] set_sensitivity(
                sensitivity: command::AutoWhiteBalanceSensitivity
            ) -> command::AWBSensitivityCommand where HasAutoWhiteBalanceSensitivity
                = command::AWBSensitivityCommand::new(sensitivity);
            /// Returns automatic white-balance sensitivity.
            inquiry command::AutoWhiteBalanceSensitivityInquiry sensitivity()
                -> command::AutoWhiteBalanceSensitivity
                where HasAutoWhiteBalanceSensitivity
                = command::AutoWhiteBalanceSensitivityInquiry;
            /// Triggers one-push white-balance calibration.
            plain [OnePushWhiteBalanceTrigger] one_push_trigger()
                -> command::OnePushTriggerCommand where HasOnePushWhiteBalance
                = command::OnePushTriggerCommand::new();
            /// Sets red-channel white-balance tuning.
            plain [RedTuning] set_red_tuning(level: types::RedTuning)
                -> command::RedTuningCommand where HasRgbTuning
                = command::RedTuningCommand::new(level);
            /// Sets blue-channel white-balance tuning.
            plain [BlueTuning] set_blue_tuning(level: types::BlueTuning)
                -> command::BlueTuningCommand where HasRgbTuning
                = command::BlueTuningCommand::new(level);
            /// Returns the color temperature.
            inquiry command::ColorTemperatureInquiry color_temperature() -> types::ColorTemp
                where HasColorTemperature
                = command::ColorTemperatureInquiry;
            /// Resets color temperature.
            plain [ColorTemperatureReset] reset_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = command::ColorTemperature::Reset;
            /// Increases color temperature.
            plain [ColorTemperatureUp] increase_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = command::ColorTemperature::Up;
            /// Decreases color temperature.
            plain [ColorTemperatureDown] decrease_color_temperature() -> command::ColorTemperature
                where HasColorTemperature
                = command::ColorTemperature::Down;
            /// Sets a direct color-temperature value.
            plain [ColorTemperatureDirect] set_color_temperature(temperature: types::ColorTemp)
                -> command::ColorTemperature where HasColorTemperature
                = command::ColorTemperature::SetTemperature(temperature);
            /// Returns the red-channel gain.
            inquiry command::RedGainInquiry red_gain() -> types::RedChannel where HasRgbGain
                = command::RedGainInquiry;
            /// Resets red-channel gain.
            plain [RedGainReset] reset_red_gain() -> command::RedGain where HasRgbGain
                = command::RedGain::Reset;
            /// Increases red-channel gain.
            plain [RedGainUp] increase_red_gain() -> command::RedGain where HasRgbGain
                = command::RedGain::Up;
            /// Decreases red-channel gain.
            plain [RedGainDown] decrease_red_gain() -> command::RedGain where HasRgbGain
                = command::RedGain::Down;
            /// Sets direct red-channel gain.
            plain [RedGainDirect] set_red_gain(value: types::RedChannel) -> command::RedGain
                where HasRgbGain
                = command::RedGain::SetValue(value);
            /// Returns the blue-channel gain.
            inquiry command::BlueGainInquiry blue_gain() -> types::BlueChannel where HasRgbGain
                = command::BlueGainInquiry;
            /// Resets blue-channel gain.
            plain [BlueGainReset] reset_blue_gain() -> command::BlueGain where HasRgbGain
                = command::BlueGain::Reset;
            /// Increases blue-channel gain.
            plain [BlueGainUp] increase_blue_gain() -> command::BlueGain where HasRgbGain
                = command::BlueGain::Up;
            /// Decreases blue-channel gain.
            plain [BlueGainDown] decrease_blue_gain() -> command::BlueGain where HasRgbGain
                = command::BlueGain::Down;
            /// Sets direct blue-channel gain.
            plain [BlueGainDirect] set_blue_gain(value: types::BlueChannel) -> command::BlueGain
                where HasRgbGain
                = command::BlueGain::SetValue(value);
            /// Returns red-channel tuning.
            inquiry command::RedTuningInquiry red_tuning() -> types::RedTuning where HasRgbTuning
                = command::RedTuningInquiry;
            /// Returns blue-channel tuning.
            inquiry command::BlueTuningInquiry blue_tuning() -> types::BlueTuning where HasRgbTuning
                = command::BlueTuningInquiry;
        ]; $($rest)* }
    };

    (Image => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Image }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Image $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            // The base image noun is gated by `HasImageProcessing` on the
            // accessor; narrower rows add their own markers below.
            @noun Image;
            /// Returns image saturation.
            inquiry command::SaturationInquiry saturation() -> types::SaturationLevel
                where HasSaturationControl
                = command::SaturationInquiry;
            /// Sets image saturation.
            plain [Saturation] set_saturation(level: types::SaturationLevel)
                -> command::SaturationCommand where HasSaturationControl
                = command::SaturationCommand::new(level);
            /// Returns image hue.
            inquiry command::HueInquiry hue() -> types::HueLevel where HasHueControl
                = command::HueInquiry;
            /// Sets image hue.
            plain [Hue] set_hue(level: types::HueLevel) -> command::HueCommand where HasHueControl
                = command::HueCommand::new(level);
            /// Returns image luminance.
            inquiry command::LuminanceInquiry luminance() -> types::LuminanceLevel
                where HasLuminanceControl
                = command::LuminanceInquiry;
            /// Sets image luminance.
            plain [Luminance] set_luminance(level: types::LuminanceLevel)
                -> command::Luminance where HasLuminanceControl
                = command::Luminance::new(level);
            /// Returns image contrast.
            inquiry command::ContrastInquiry contrast() -> types::ContrastLevel
                where HasContrastControl
                = command::ContrastInquiry;
            /// Sets image contrast.
            plain [Contrast] set_contrast(level: types::ContrastLevel)
                -> command::Contrast where HasContrastControl
                = command::Contrast::new(level);
            /// Returns the gamma curve.
            inquiry command::GammaInquiry gamma() -> types::GammaLevel where HasGammaControl
                = command::GammaInquiry;
            /// Sets the gamma curve.
            plain [Gamma] set_gamma(level: types::GammaLevel) -> command::GammaCommand
                where HasGammaControl
                = command::GammaCommand::new(level);
            /// Returns the sharpness mode.
            inquiry command::SharpnessModeInquiry sharpness_mode() -> command::SharpnessMode
                where HasSharpnessControl
                = command::SharpnessModeInquiry;
            /// Returns the sharpness level.
            inquiry command::SharpnessPositionInquiry sharpness_level() -> types::SharpnessLevel
                where HasSharpnessControl
                = command::SharpnessPositionInquiry;
            /// Sets the sharpness mode.
            plain [SharpnessMode] set_sharpness_mode(mode: command::SharpnessMode)
                -> command::Sharpness where HasSharpnessControl
                = command::Sharpness::Mode(mode);
            /// Resets sharpness.
            plain [SharpnessReset] reset_sharpness() -> command::Sharpness
                where HasSharpnessControl = command::Sharpness::Reset;
            /// Increases sharpness by one step.
            plain [SharpnessUp] increase_sharpness() -> command::Sharpness
                where HasSharpnessControl = command::Sharpness::Up;
            /// Decreases sharpness by one step.
            plain [SharpnessDown] decrease_sharpness() -> command::Sharpness
                where HasSharpnessControl = command::Sharpness::Down;
            /// Sets a direct sharpness level.
            plain [SharpnessDirect] set_sharpness(level: types::SharpnessLevel)
                -> command::Sharpness where HasSharpnessControl
                = command::Sharpness::SetLevel { value: level.value() };
            /// Returns backlight compensation state.
            inquiry command::BacklightInquiry backlight() -> bool where HasBacklightCompensation
                = command::BacklightInquiry;
            /// Enables or disables backlight compensation.
            plain [Backlight] set_backlight(enabled: bool) -> command::BacklightCommand
                where HasBacklightCompensation
                = command::BacklightCommand::new(enabled);
            /// Returns 2D noise reduction level.
            inquiry command::NoiseReduction2DInquiry noise_reduction_2d()
                -> types::NoiseReduction2DLevel where HasNoiseReduction2D
                = command::NoiseReduction2DInquiry;
            /// Sets 2D noise reduction level.
            plain [NoiseReduction2d] set_noise_reduction_2d(
                level: types::NoiseReduction2DLevel
            ) -> command::NoiseReduction2D where HasNoiseReduction2D
                = command::NoiseReduction2D::with_level(level);
            /// Disables 2D noise reduction.
            plain [NoiseReduction2dOff] disable_noise_reduction_2d()
                -> command::NoiseReduction2D where HasNoiseReduction2D
                = command::NoiseReduction2D::off();
            /// Returns 3D noise reduction level.
            inquiry command::NoiseReduction3DInquiry noise_reduction_3d()
                -> types::NoiseReduction3DLevel where HasNoiseReduction3D
                = command::NoiseReduction3DInquiry;
            /// Sets 3D noise reduction level.
            plain [NoiseReduction3d] set_noise_reduction_3d(
                level: types::NoiseReduction3DLevel
            ) -> command::NoiseReduction3D where HasNoiseReduction3D
                = command::NoiseReduction3D::with_level(level);
            /// Disables 3D noise reduction.
            plain [NoiseReduction3dOff] disable_noise_reduction_3d()
                -> command::NoiseReduction3D where HasNoiseReduction3D
                = command::NoiseReduction3D::off();
            /// Returns the aggregate noise-reduction level.
            inquiry command::NrLevelInquiry noise_reduction_level() -> types::NoiseReductionLevel
                where HasNoiseReduction
                = command::NrLevelInquiry;
            /// Returns the aggregate noise-reduction mode.
            inquiry command::NrModeInquiry noise_reduction_mode() -> command::NoiseReductionMode
                where HasNoiseReduction
                = command::NrModeInquiry;
            /// Disables vertical image flip.
            plain [ImageFlipOff] disable_flip() -> builtin::ImageFlipCommand where HasImageFlip
                = builtin::ImageFlipCommand::new(command::Flip::Off);
            /// Enables vertical image flip.
            plain [ImageFlipVertical] enable_flip() -> builtin::ImageFlipCommand where HasImageFlip
                = builtin::ImageFlipCommand::new(command::Flip::On);
            /// Enables horizontal image mirroring.
            plain [ImageFlipHorizontal] enable_horizontal_flip() -> builtin::ImageMirrorCommand
                where HasImageMirror
                = builtin::ImageMirrorCommand::new(true);
            /// Disables horizontal image mirroring.
            plain [ImageFlipHorizontalOff] disable_horizontal_flip()
                -> builtin::ImageMirrorCommand where HasImageMirror
                = builtin::ImageMirrorCommand::new(false);
            /// Sets the combined image-flip mode to both axes.
            ///
            /// This sends the combined flip opcode, so it carries the same
            /// `HasCombinedImageFlip` bound as [`Self::set_flip_mode`].
            plain [ImageFlipBoth] set_flip_both() -> command::ImageFlipCombinedCommand
                where HasCombinedImageFlip
                = command::ImageFlipCombinedCommand::new(command::ImageFlipMode::Both);
            /// Sets the combined image-flip mode.
            plain [ImageFlipCombined] set_flip_mode(mode: command::ImageFlipMode)
                -> command::ImageFlipCombinedCommand where HasCombinedImageFlip
                = command::ImageFlipCombinedCommand::new(mode);
            /// Freezes the image.
            plain [ImageFreezeOn] freeze_on() -> command::ImageFreeze
                = command::ImageFreeze::on();
            /// Resumes live image output.
            plain [ImageFreezeOff] freeze_off() -> command::ImageFreeze
                = command::ImageFreeze::off();
            /// Returns the canonical image-flip state.
            inquiry command::ImageFlipInquiry flip() -> command::FlipState where HasImageFlip
                = command::ImageFlipInquiry;
            /// Returns the combined image-flip mode.
            inquiry command::FlipStateInquiry flip_mode() -> command::FlipState where HasImageFlip
                = command::FlipStateInquiry;
            /// Returns picture-effect mode.
            inquiry command::PictureEffectInquiry picture_effect()
                -> command::PictureEffectMode where HasPictureEffect
                = command::PictureEffectInquiry;
            /// Sets picture-effect mode.
            plain [PictureEffect] set_picture_effect(mode: command::PictureEffectMode)
                -> command::PictureEffectCommand where HasPictureEffect
                = command::PictureEffectCommand::new(mode);
            /// Returns the camera's defog level.
            inquiry command::DefogLevelInquiry defog_level() -> types::DefogLevel
                = command::DefogLevelInquiry;
        ]; $($rest)* }
    };

    (Tally => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Tally }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Tally $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Tally;
            /// Returns all tally light state.
            inquiry command::TallyStatusInquiry status() -> command::TallyStatusState
                = command::TallyStatusInquiry;
            /// Turns the red tally on.
            plain [TallyRedOn] red_on() -> command::TallyRedOn = command::TallyRedOn::new();
            /// Turns the red tally off.
            plain [TallyRedOff] red_off() -> command::TallyRedOff = command::TallyRedOff::new();
            /// Sets low tally brightness.
            plain [TallyBrightLow] bright_lo() -> command::TallyBrightLo
                = command::TallyBrightLo::new();
            /// Sets high tally brightness.
            plain [TallyBrightHigh] bright_hi() -> command::TallyBrightHi
                = command::TallyBrightHi::new();
            /// Turns the green tally on.
            plain [TallyGreenOn] green_on() -> command::TallyGreenOn
                = command::TallyGreenOn::new();
            /// Turns the green tally off.
            plain [TallyGreenOff] green_off() -> command::TallyGreenOff
                = command::TallyGreenOff::new();
            /// Sets tally flash mode.
            plain [TallyFlash] flash() -> command::TallyFlash = command::TallyFlash::new();
            /// Sets tally solid-on mode.
            plain [TallyOn] on() -> command::TallyOn = command::TallyOn::new();
            /// Turns tally output off.
            plain [TallyOff] off() -> command::TallyOff = command::TallyOff::new();
            /// Returns red tally state.
            inquiry command::TallyRedInquiry red_status() -> bool = command::TallyRedInquiry;
            /// Returns green tally state.
            inquiry command::TallyGreenInquiry green_status() -> bool = command::TallyGreenInquiry;
            /// Returns automatic tally adjustment state.
            inquiry command::TallyAutoAdjustInquiry auto_adjust_enabled() -> bool
                = command::TallyAutoAdjustInquiry;
        ]; $($rest)* }
    };

    (NdFilter => $consumer:ident) => {
        noun_table! { @collect $consumer; []; NdFilter }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; NdFilter $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun NdFilter;
            /// Returns the current ND-filter position.
            inquiry command::NdFilterInquiry position() -> command::NdFilterPosition
                = command::NdFilterInquiry;
            /// Returns the current ND-filter preset.
            inquiry command::NdFilterPresetInquiry preset() -> types::NdFilterPreset
                = command::NdFilterPresetInquiry;
            /// Selects preset or variable ND-filter mode.
            plain [NdFilterMode] set_mode(mode: command::NdFilterMode)
                -> command::NdFilterModeCommand = command::NdFilterModeCommand::new(mode);
            /// Sets a direct variable ND-filter value.
            targeted [NdFilterDirect] set_value(value: u16) -> builtin::NdFilterDirect
                = checked command::NdFilterValue::new(value).map(builtin::NdFilterDirect::new);
            /// Sets a direct variable ND-filter value in photographic stops.
            ///
            /// `stops` is the light reduction in stops and must lie in `2.0..=7.0`.
            /// Each raw unit is a quarter stop, so `2.0` maps to the minimum density
            /// and `7.0` to the maximum.
            targeted [] set_stops(stops: f32) -> builtin::NdFilterDirect
                = checked command::NdFilterValue::from_stops(stops)
                    .map(builtin::NdFilterDirect::new);
            /// Increases ND-filter density by one step.
            targeted [NdFilterStepUp] step_up() -> builtin::NdFilterStepUp
                = builtin::NdFilterStepUp::new();
            /// Decreases ND-filter density by one step.
            targeted [NdFilterStepDown] step_down() -> builtin::NdFilterStepDown
                = builtin::NdFilterStepDown::new();
            /// Enables automatic ND filtering.
            plain [NdFilterAutoOn] auto_on() -> command::AutoNdCommand
                = command::AutoNdCommand::new(true);
            /// Disables automatic ND filtering.
            plain [NdFilterAutoOff] auto_off() -> command::AutoNdCommand
                = command::AutoNdCommand::new(false);
        ]; $($rest)* }
    };

    (MotionSync => $consumer:ident) => {
        noun_table! { @collect $consumer; []; MotionSync }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; MotionSync $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun MotionSync;
            /// Returns the motion-sync mode.
            inquiry command::MotionSyncModeInquiry mode() -> command::MotionSyncMode
                = command::MotionSyncModeInquiry;
            /// Returns the motion-sync preset speed.
            inquiry command::MotionSyncPresetInquiry preset() -> command::MotionSyncPreset
                = command::MotionSyncPresetInquiry;
            /// Enables or disables motion synchronization.
            plain [MotionSyncMode] set_mode(mode: command::MotionSyncMode)
                -> command::SetMotionSyncMode
                = command::SetMotionSyncMode::new(mode);
            /// Sets the motion-sync speed preset.
            plain [MotionSyncPreset] set_preset(speed: u8) -> command::SetMotionSyncPreset
                = checked command::SetMotionSyncPreset::new(speed);
            /// Sets the motion-sync speed from a range-checked speed value.
            ///
            /// This is [`Self::set_preset`] with the `1..=24` bound moved into the
            /// argument type, so an out-of-range speed cannot be constructed.
            plain [] set_speed(speed: types::MotionSyncSpeed) -> command::SetMotionSyncPreset
                = checked command::SetMotionSyncPreset::new(speed.value());
        ]; $($rest)* }
    };

    (Menu => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Menu }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Menu $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Menu;
            /// Returns whether the on-screen menu is open.
            inquiry command::MenuOpenCloseInquiry status() -> bool = command::MenuOpenCloseInquiry;
            /// Displays or hides the on-screen menu.
            plain [MenuDisplay] display(on: bool) -> command::SetMenuDisplay
                = command::SetMenuDisplay::new(on);
            /// Moves the menu cursor.
            plain [MenuNavigate] navigate(direction: command::MenuDirection)
                -> command::MenuNavigate
                = command::MenuNavigate::new(direction);
            /// Selects the current menu item.
            plain [MenuSelect] select() -> command::PerformMenuAction
                = command::PerformMenuAction::new(command::MenuAction::Select);
            /// Cancels or returns from the current menu item.
            plain [MenuCancel] cancel() -> command::PerformMenuAction
                = command::PerformMenuAction::new(command::MenuAction::Cancel);
            /// Sends a vendor-specific direct menu control.
            plain [DirectMenu] direct(control1: u8, control2: u8)
                -> command::DirectMenuControl where HasDirectMenuControl
                = checked command::DirectMenuControl::new(control1, control2);
            /// Toggles the on-screen menu open or closed.
            ///
            /// This is the vendor open/close direct control, so it needs no prior
            /// [`Self::status`] round trip to decide which way to move.
            plain [] toggle_display() -> command::DirectMenuControl where HasDirectMenuControl
                = command::DirectMenuControl::open_close();
        ]; $($rest)* }
    };

    (Advanced => $consumer:ident) => {
        noun_table! { @collect $consumer; []; Advanced }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; Advanced $($rest:tt)*) => {
        noun_table! { @collect $consumer; [
            $($acc)*
            @noun Advanced;
            /// Returns night/day mode.
            inquiry command::NightDayModeInquiry night_day_mode() -> bool
                = command::NightDayModeInquiry;
            /// Returns standby state.
            inquiry command::StandbyInquiry standby_enabled() -> bool = command::StandbyInquiry;
            /// Returns digital PTZ state.
            inquiry command::DigitalPtzInquiry digital_ptz_enabled() -> bool
                = command::DigitalPtzInquiry;
            /// Returns auto-trace state.
            inquiry command::AutoTraceInquiry auto_trace_enabled() -> bool
                = command::AutoTraceInquiry;
            /// Returns focus-unlock state.
            inquiry command::FocusUnlockInquiry focus_unlock() -> bool
                = command::FocusUnlockInquiry;
            /// Returns the broadcast domain.
            inquiry command::BroadcastDomainInquiry broadcast_domain() -> types::BroadcastDomain
                = command::BroadcastDomainInquiry;
            /// Returns USB-audio state.
            inquiry command::UsbAudioInquiry usb_audio_enabled() -> bool where HasUsbAudio
                = command::UsbAudioInquiry;
            /// Returns two-tone mode.
            inquiry command::TwoToneModeInquiry two_tone_mode_enabled() -> bool
                = command::TwoToneModeInquiry;
            /// Returns digital mode.
            inquiry command::DigitalInquiry digital_mode_enabled() -> bool
                = command::DigitalInquiry;
            /// Enables multicast streaming.
            plain [MulticastStreamingOn] multicast_on() -> command::MulticastStreaming
                where HasPtzOpticsMulticastStreaming
                = command::MulticastStreaming::On;
            /// Disables multicast streaming.
            plain [MulticastStreamingOff] multicast_off() -> command::MulticastStreaming
                where HasPtzOpticsMulticastStreaming
                = command::MulticastStreaming::Off;
            /// Sets NDI streaming quality.
            plain [NdiQuality] set_ndi_quality(quality: types::NdiQuality)
                -> command::SetNdiQuality
                where HasPtzOpticsNdiQuality
                = command::SetNdiQuality::new(quality);
            /// Enables USB audio.
            plain [UsbAudioOn] usb_audio_on() -> command::UsbAudio where HasUsbAudio
                = command::UsbAudio::On;
            /// Disables USB audio.
            plain [UsbAudioOff] usb_audio_off() -> command::UsbAudio where HasUsbAudio
                = command::UsbAudio::Off;
            /// Sets pan/tilt variable-speed mode.
            plain [VariableSpeedMode] set_variable_speed_mode(mode: command::VariableSpeedMode)
                -> command::SetVariableSpeedMode where HasVariableSpeed
                = command::SetVariableSpeedMode::new(mode);
        ]; $($rest)* }
    };

    (@collect $consumer:ident; [$($acc:tt)*]; @exceptions) => {
        $consumer! {
            $($acc)*
            @exceptions;
            broadcast [AddressSet] address_set;
            broadcast [InterfaceClear] interface_clear;
            internal [CommandCancel] cancel_command;
        }
    };

    (@collect $consumer:ident; [$($acc:tt)*];) => {
        $consumer! { $($acc)* }
    };
}

pub(crate) use noun_table;
