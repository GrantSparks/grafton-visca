//! The single row table behind all three noun facades.
//!
//! [`crate::async_nouns`], [`crate::blocking::nouns`] and
//! [`crate::dynapi::nouns`] used to be three independent hand-written
//! transcriptions of the same closed ledger in [`crate::command::surface`]:
//! every noun method was written out three times — name, arguments,
//! capability bound, return class and rustdoc — with nothing in the type
//! system relating the copies.  This module is the one place those facts are
//! now written down.
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
//! <kind> <method>(<arg>: <type>, ...) [where <Marker> [+ <Marker>]] = <request>;
//! ```
//!
//! * `<kind>` is `inquiry`, `plain`, `applied` or `targeted` — the semantic
//!   return class the ledger records for the row.  An `inquiry` row spells its
//!   decoded response type as `-> <type>` and takes no arguments.
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
//! same method differently.
//!
//! # What is *not* in the table
//!
//! `MotionAccessor`/`DynMotion` is a safety and observation view rather than a
//! ledger noun: its four methods reach `stop_all_motion`, `is_moving` and
//! `wait_until_idle` on the owner core directly and share no shape with a
//! command row.  Those four stay hand-written on each facade and are named in
//! [`crate::noun_parity`]'s exemption list, which still compares them across
//! the three surfaces.

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
    (Power => $consumer:ident) => {
        $consumer! {
            /// Returns the camera's current power state.
            inquiry state() -> bool = command::PowerInquiry;
            /// Powers the camera on.
            plain on() = command::PowerOn::new();
            /// Places the camera in standby.
            plain off() = command::PowerStandby::new();
        }
    };

    (Zoom => $consumer:ident) => {
        $consumer! {
            /// Returns the current optical/digital zoom position.
            inquiry position() -> types::ZoomPosition = command::ZoomPositionInquiry;
            /// Drives toward telephoto at standard speed.
            applied tele() = builtin::ZoomDrive::Tele;
            /// Drives toward wide angle at standard speed.
            applied wide() = builtin::ZoomDrive::Wide;
            /// Stops zoom movement.
            applied stop() = builtin::ZoomStop;
            /// Drives toward telephoto at a validated variable speed.
            applied tele_variable(speed: types::ZoomSpeed)
                = builtin::ZoomDrive::TeleVariable(speed);
            /// Drives toward wide angle at a validated variable speed.
            applied wide_variable(speed: types::ZoomSpeed)
                = builtin::ZoomDrive::WideVariable(speed);
            /// Moves to an absolute zoom position.
            targeted set_position(position: types::ZoomPosition) where HasDirectZoom
                = builtin::ZoomTarget::new(position);
            /// Moves to a normalized position across the optical zoom range.
            ///
            /// `0.0` is the wide end and `1.0` the telephoto end of the profile's
            /// documented optical range.
            targeted set_normalized(position: UnitInterval) where HasDirectZoom
                = with_profile |profile| builtin::ZoomTarget::from_normalized(
                    position,
                    ZoomDomain::Optical,
                    profile,
                );
            /// Moves to a normalized position across a documented zoom domain.
            ///
            /// [`ZoomDomain::OpticalPlusDigital`] requires the profile to document a
            /// digital maximum and never falls back to the optical range.
            targeted set_normalized_in_domain(position: UnitInterval, domain: ZoomDomain)
                where HasDirectZoom + HasDigitalZoomRange
                = with_profile |profile| builtin::ZoomTarget::from_normalized(
                    position,
                    domain,
                    profile,
                );
            /// Enables or disables digital zoom.
            plain set_digital_zoom(enabled: bool) where HasDigitalZoomToggle
                = command::DigitalZoom::new(enabled);
        }
    };

    (System => $consumer:ident) => {
        $consumer! {
            /// Returns the camera firmware/version information.
            inquiry version() -> command::VersionInfo = command::VersionInquiry;
            /// Saves the camera's current settings to non-volatile storage.
            plain save_settings() = command::SettingsSaveCommand::new();
        }
    };

    (PanTilt => $consumer:ident) => {
        $consumer! {
            /// Returns the current pan/tilt position.
            inquiry position() -> crate::camera::PanTiltPosition
                = command::PanTiltPositionInquiry;
            /// Moves the pan/tilt mechanism to its home position.
            targeted home() = builtin::PanTiltHome;
            /// Resets the pan/tilt mechanism.
            targeted reset() = builtin::PanTiltReset;
            /// Starts a directional pan/tilt drive.
            applied move_direction(
                direction: command::PanTiltDirection,
                pan_speed: types::PanSpeed,
                tilt_speed: types::TiltSpeed
            ) = checked builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed);
            /// Starts an upward pan/tilt drive.
            applied up(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Up, pan_speed, tilt_speed);
            /// Starts a downward pan/tilt drive.
            applied down(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Down, pan_speed, tilt_speed);
            /// Starts a leftward pan/tilt drive.
            applied left(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Left, pan_speed, tilt_speed);
            /// Starts a rightward pan/tilt drive.
            applied right(pan_speed: types::PanSpeed, tilt_speed: types::TiltSpeed)
                = delegate move_direction(command::PanTiltDirection::Right, pan_speed, tilt_speed);
            /// Stops pan/tilt movement using profile-safe stop speeds.
            applied stop() = with_core |core| core.pan_tilt_stop_request();
            /// Moves to an absolute degree position at the selected speed.
            targeted absolute(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                = with_profile |profile| builtin::PanTiltAbsolute::for_profile(
                    pan,
                    tilt,
                    types::PanSpeed::from(speed),
                    types::TiltSpeed::from(speed),
                    profile,
                );
            /// Moves by a relative degree offset at the selected speed.
            targeted relative(pan: Degrees<f32>, tilt: Degrees<f32>, speed: types::SpeedLevel)
                = with_profile |profile| builtin::PanTiltRelative::for_profile(
                    pan,
                    tilt,
                    types::PanSpeed::from(speed),
                    types::TiltSpeed::from(speed),
                    profile,
                );
            /// Sets one pan/tilt movement-limit corner.
            plain limit_set(
                corner: command::PanTiltLimitCorner,
                pan: Degrees<f32>,
                tilt: Degrees<f32>
            ) = with_profile |profile| builtin::PanTiltLimitSet::for_profile(
                corner,
                pan,
                tilt,
                profile,
            );
            /// Clears one pan/tilt movement-limit corner.
            plain limit_clear(corner: command::PanTiltLimitCorner)
                = builtin::PanTiltLimitClear::new(corner);
        }
    };

    (Focus => $consumer:ident) => {
        $consumer! {
            /// Returns the current focus position.
            inquiry position() -> types::FocusPosition = command::FocusPositionInquiry;
            /// Returns the current focus mode.
            inquiry mode() -> command::FocusMode = command::FocusModeInquiry;
            /// Drives focus farther at standard speed.
            applied far() = builtin::FocusDrive::Far;
            /// Drives focus nearer at standard speed.
            applied near() = builtin::FocusDrive::Near;
            /// Drives focus farther at a variable speed.
            applied far_variable(speed: command::FocusSpeed)
                = builtin::FocusDrive::FarVariable(speed);
            /// Drives focus nearer at a variable speed.
            applied near_variable(speed: command::FocusSpeed)
                = builtin::FocusDrive::NearVariable(speed);
            /// Stops focus movement.
            applied stop() = builtin::FocusStop;
            /// Moves focus to an absolute position.
            targeted set_position(position: types::FocusPosition)
                = builtin::FocusTarget::new(position);
            /// Enables automatic focus mode.
            plain auto() = builtin::FocusModeCommand::Auto;
            /// Enables manual focus mode.
            plain manual() = builtin::FocusModeCommand::Manual;
            /// Triggers one-push autofocus.
            applied one_push() where HasOnePushFocus = builtin::FocusTrigger::OnePush;
            /// Moves focus to infinity.
            targeted infinity() = builtin::FocusInfinity;
            /// Toggles automatic/manual focus mode.
            plain toggle() = builtin::FocusModeCommand::Toggle;
            /// Triggers vendor snap focus.
            applied snap() where HasPtzOpticsSnapFocus = builtin::FocusTrigger::Snap;
            /// Selects a focus zone.
            plain set_zone(zone: command::FocusZone) where HasFocusZone
                = command::FocusZoneCommand::new(zone);
            /// Sets the autofocus sensitivity.
            plain set_sensitivity(sensitivity: command::AutoFocusSensitivity)
                where HasAutoFocusSensitivity
                = command::AutoFocusSensitivityCommand::new(sensitivity);
            /// Sets the minimum focus distance.
            plain set_near_limit(position: types::FocusPosition) where HasFocusNearLimitInquiry
                = command::FocusNearLimitCommand::new(position);
            /// Sets the focus-lock mode.
            plain set_lock(mode: command::FocusLock) where HasFocusLock = mode;
            /// Presses the vendor Push-AF control.
            applied push_af_press() where HasPushAutoFocus = builtin::PushAfPress::new();
            /// Releases the vendor Push-AF control.
            applied push_af_release() where HasPushAutoFocus = builtin::PushAfRelease::new();
            /// Returns the configured focus near limit.
            inquiry near_limit() -> types::FocusPosition where HasFocusNearLimitInquiry
                = command::FocusNearLimitInquiry;
            /// Returns the configured focus zone.
            inquiry zone() -> command::FocusZone where HasFocusZone = command::FocusZoneInquiry;
            /// Returns the autofocus sensitivity.
            inquiry sensitivity() -> command::AutoFocusSensitivity where HasAutoFocusSensitivity
                = command::AutoFocusSensitivityInquiry;
            /// Returns the configured focus range.
            inquiry range() -> command::FocusRange = command::FocusRangeInquiry;
        }
    };

    (Presets => $consumer:ident) => {
        $consumer! {
            /// Recalls a stored preset as a targeted operation.
            targeted recall(preset: command::PresetNumber)
                = with_profile |profile| builtin::PresetRecall::for_profile(preset, profile);
            /// Sets the preset-recall speed.
            plain set_recall_speed(speed: command::PresetRecallSpeed)
                = command::PresetRecallSpeedCommand::new(speed);
            /// Stores the current camera state in a preset.
            plain set(preset: command::PresetNumber) = builtin::PresetSet::new(preset);
            /// Clears a stored preset.
            plain reset(preset: command::PresetNumber) = builtin::PresetReset::new(preset);
        }
    };

    (Exposure => $consumer:ident) => {
        $consumer! {
            /// Returns the active exposure mode.
            inquiry mode() -> command::ExposureMode = command::ExposureModeInquiry;
            /// Sets the exposure mode.
            plain set_mode(mode: command::ExposureMode) = command::ExposureCommand::new(mode);
            /// Returns the shutter speed.
            inquiry shutter() -> types::ShutterSpeed = command::ShutterInquiry;
            /// Restores the camera's shutter default.
            plain shutter_reset() = command::Shutter::Reset;
            /// Increases shutter speed by one camera-defined step.
            plain shutter_up() = command::Shutter::Up;
            /// Decreases shutter speed by one camera-defined step.
            plain shutter_down() = command::Shutter::Down;
            /// Sets an explicit shutter speed.
            plain shutter_direct(speed: types::ShutterSpeed) = command::Shutter::SetSpeed(speed);
            /// Returns the exposure compensation value.
            inquiry compensation() -> types::ExposureCompensationLevel
                where HasExposureCompensation
                = command::ExposureCompensationInquiry;
            /// Returns whether exposure compensation is enabled.
            inquiry compensation_enabled() -> bool where HasExposureCompensation
                = command::ExposureCompensationModeInquiry;
            /// Returns the camera's exposure compensation position.
            inquiry compensation_position() -> types::ExposureCompensationPosition
                where HasExposureCompensation
                = command::ExposureCompensationPositionInquiry;
            /// Enables exposure compensation.
            plain compensation_on() where HasExposureCompensation
                = command::ExposureCompensation::On;
            /// Disables exposure compensation.
            plain compensation_off() where HasExposureCompensation
                = command::ExposureCompensation::Off;
            /// Resets exposure compensation.
            plain compensation_reset() where HasExposureCompensation
                = command::ExposureCompensation::Reset;
            /// Increases exposure compensation by one step.
            plain compensation_up() where HasExposureCompensation
                = command::ExposureCompensation::Up;
            /// Decreases exposure compensation by one step.
            plain compensation_down() where HasExposureCompensation
                = command::ExposureCompensation::Down;
            /// Sets direct exposure compensation.
            plain compensation_direct(level: types::ExposureCompensationLevel)
                where HasExposureCompensation
                = command::ExposureCompensation::SetLevel(level);
            /// Returns the wide-dynamic-range level.
            inquiry dynamic_range() -> types::DynamicRangeLevel where HasWideDynamicRange
                = command::DynamicRangeInquiry;
            /// Sets the wide-dynamic-range level.
            plain set_dynamic_range(level: types::DynamicRangeLevel) where HasWideDynamicRange
                = command::DynamicRange::new(level);
            /// Returns whether iris control is automatic.
            inquiry iris_control() -> bool where HasIrisControl = command::IrisControlInquiry;
            /// Returns the iris level.
            inquiry iris() -> types::IrisLevel where HasIrisControl = command::IrisInquiry;
            /// Resets the iris.
            targeted iris_reset() where HasIrisControl = builtin::IrisReset::new();
            /// Increases the iris by one step.
            targeted iris_up() where HasIrisControl = builtin::IrisUp::new();
            /// Decreases the iris by one step.
            targeted iris_down() where HasIrisControl = builtin::IrisDown::new();
            /// Sets a direct iris level.
            targeted iris_direct(level: types::IrisLevel) where HasIrisControl
                = builtin::IrisDirect::new(level);
            /// Returns exposure brightness.
            inquiry brightness() -> types::BrightnessLevel where HasBrightnessControl
                = command::BrightnessInquiry;
            /// Resets exposure brightness.
            plain brightness_reset() where HasBrightnessControl = command::Brightness::Reset;
            /// Increases exposure brightness.
            plain brightness_up() where HasBrightnessControl = command::Brightness::Up;
            /// Decreases exposure brightness.
            plain brightness_down() where HasBrightnessControl = command::Brightness::Down;
            /// Sets exposure brightness through the bright-direct command.
            plain brightness_set(level: types::BrightnessLevel) where HasBrightnessControl
                = command::Brightness::SetLevel(level);
            /// Sets the camera's direct brightness value.
            plain brightness_direct(level: types::BrightnessLevel) where HasBrightnessControl
                = command::Brightness::Direct(level);
            /// Returns gain.
            inquiry gain() -> types::GainLevel = command::GainInquiry;
            /// Resets gain.
            plain gain_reset() = command::Gain::Reset;
            /// Increases gain.
            plain gain_up() = command::Gain::Up;
            /// Decreases gain.
            plain gain_down() = command::Gain::Down;
            /// Sets direct gain.
            plain gain_direct(level: types::GainLevel) = command::Gain::SetValue(level);
            /// Returns the configured gain limit.
            inquiry gain_limit() -> types::GainLimit = command::GainLimitInquiry;
            /// Sets the configured gain limit.
            plain set_gain_limit(limit: types::GainLimit) = command::GainLimitCommand::new(limit);
            /// Sets anti-flicker mode.
            plain set_anti_flicker(mode: command::AntiFlickerMode)
                = command::AntiFlickerCommand::new(mode);
            /// Returns the configured anti-flicker mode.
            inquiry flicker_mode() -> command::AntiFlickerMode = command::FlickerModeInquiry;
            /// Enables spotlight mode.
            plain spotlight_on() = command::SpotlightOn::new();
            /// Disables spotlight mode.
            plain spotlight_off() = command::SpotlightOff::new();
            /// Enables automatic slow shutter.
            plain auto_slow_shutter_on() = command::AutoSlowShutterOn::new();
            /// Disables automatic slow shutter.
            plain auto_slow_shutter_off() = command::AutoSlowShutterOff::new();
        }
    };

    (WhiteBalance => $consumer:ident) => {
        $consumer! {
            /// Returns the active white-balance mode.
            inquiry mode() -> command::WhiteBalanceMode = command::WhiteBalanceModeInquiry;
            /// Selects automatic white balance.
            plain auto() = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Auto);
            /// Selects the indoor white-balance preset.
            plain indoor() = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Indoor);
            /// Selects the outdoor white-balance preset.
            plain outdoor() = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Outdoor);
            /// Selects one-push white balance.
            plain one_push() where HasOnePushWhiteBalance
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::OnePush);
            /// Selects auto-tracking white balance.
            plain atw() where HasAutoTrackingWhiteBalance
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ATW);
            /// Selects manual white balance.
            plain manual() = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Manual);
            /// Selects color-temperature white-balance mode.
            plain color_temperature_mode() where HasColorTemperature
                = command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ColorTemperature);
            /// Sets automatic white-balance sensitivity.
            plain set_sensitivity(sensitivity: command::AutoWhiteBalanceSensitivity)
                where HasAutoWhiteBalanceSensitivity
                = command::AWBSensitivityCommand::new(sensitivity);
            /// Returns automatic white-balance sensitivity.
            inquiry sensitivity() -> command::AutoWhiteBalanceSensitivity
                where HasAutoWhiteBalanceSensitivity
                = command::AutoWhiteBalanceSensitivityInquiry;
            /// Triggers one-push white-balance calibration.
            plain one_push_trigger() where HasOnePushWhiteBalance
                = command::OnePushTriggerCommand::new();
            /// Sets red-channel white-balance tuning.
            plain set_red_tuning(level: types::RedTuning) where HasRgbTuning
                = command::RedTuningCommand::new(level);
            /// Sets blue-channel white-balance tuning.
            plain set_blue_tuning(level: types::BlueTuning) where HasRgbTuning
                = command::BlueTuningCommand::new(level);
            /// Returns the color temperature.
            inquiry color_temperature() -> types::ColorTemp where HasColorTemperature
                = command::ColorTemperatureInquiry;
            /// Resets color temperature.
            plain reset_color_temperature() where HasColorTemperature
                = command::ColorTemperature::Reset;
            /// Increases color temperature.
            plain increase_color_temperature() where HasColorTemperature
                = command::ColorTemperature::Up;
            /// Decreases color temperature.
            plain decrease_color_temperature() where HasColorTemperature
                = command::ColorTemperature::Down;
            /// Sets a direct color-temperature value.
            plain set_color_temperature(temperature: types::ColorTemp) where HasColorTemperature
                = command::ColorTemperature::SetTemperature(temperature);
            /// Returns the red-channel gain.
            inquiry red_gain() -> types::RedChannel where HasRgbGain = command::RedGainInquiry;
            /// Resets red-channel gain.
            plain reset_red_gain() where HasRgbGain = command::RedGain::Reset;
            /// Increases red-channel gain.
            plain increase_red_gain() where HasRgbGain = command::RedGain::Up;
            /// Decreases red-channel gain.
            plain decrease_red_gain() where HasRgbGain = command::RedGain::Down;
            /// Sets direct red-channel gain.
            plain set_red_gain(value: types::RedChannel) where HasRgbGain
                = command::RedGain::SetValue(value);
            /// Returns the blue-channel gain.
            inquiry blue_gain() -> types::BlueChannel where HasRgbGain = command::BlueGainInquiry;
            /// Resets blue-channel gain.
            plain reset_blue_gain() where HasRgbGain = command::BlueGain::Reset;
            /// Increases blue-channel gain.
            plain increase_blue_gain() where HasRgbGain = command::BlueGain::Up;
            /// Decreases blue-channel gain.
            plain decrease_blue_gain() where HasRgbGain = command::BlueGain::Down;
            /// Sets direct blue-channel gain.
            plain set_blue_gain(value: types::BlueChannel) where HasRgbGain
                = command::BlueGain::SetValue(value);
            /// Returns red-channel tuning.
            inquiry red_tuning() -> types::RedTuning where HasRgbTuning
                = command::RedTuningInquiry;
            /// Returns blue-channel tuning.
            inquiry blue_tuning() -> types::BlueTuning where HasRgbTuning
                = command::BlueTuningInquiry;
        }
    };

    (Image => $consumer:ident) => {
        $consumer! {
            /// Returns the camera resolution mode.
            inquiry resolution() -> command::ResolutionMode = command::ResolutionInquiry;
            /// Returns image saturation.
            inquiry saturation() -> types::SaturationLevel where HasSaturationControl
                = command::SaturationInquiry;
            /// Sets image saturation.
            plain set_saturation(level: types::SaturationLevel) where HasSaturationControl
                = command::SaturationCommand::new(level);
            /// Returns image hue.
            inquiry hue() -> types::HueLevel where HasHueControl = command::HueInquiry;
            /// Sets image hue.
            plain set_hue(level: types::HueLevel) where HasHueControl
                = command::HueCommand::new(level);
            /// Returns image luminance.
            inquiry luminance() -> types::LuminanceLevel where HasLuminanceControl
                = command::LuminanceInquiry;
            /// Sets image luminance.
            plain set_luminance(level: types::LuminanceLevel) where HasLuminanceControl
                = command::Luminance::new(level);
            /// Returns image contrast.
            inquiry contrast() -> types::ContrastLevel where HasContrastControl
                = command::ContrastInquiry;
            /// Sets image contrast.
            plain set_contrast(level: types::ContrastLevel) where HasContrastControl
                = command::Contrast::new(level);
            /// Returns the gamma curve.
            inquiry gamma() -> types::GammaLevel where HasGammaControl = command::GammaInquiry;
            /// Sets the gamma curve.
            plain set_gamma(level: types::GammaLevel) where HasGammaControl
                = command::GammaCommand::new(level);
            /// Returns the sharpness mode.
            inquiry sharpness_mode() -> command::SharpnessMode where HasSharpnessControl
                = command::SharpnessModeInquiry;
            /// Returns the sharpness level.
            inquiry sharpness_level() -> types::SharpnessLevel where HasSharpnessControl
                = command::SharpnessPositionInquiry;
            /// Sets the sharpness mode.
            plain set_sharpness_mode(mode: command::SharpnessMode) where HasSharpnessControl
                = command::Sharpness::Mode(mode);
            /// Resets sharpness.
            plain reset_sharpness() where HasSharpnessControl = command::Sharpness::Reset;
            /// Increases sharpness by one step.
            plain increase_sharpness() where HasSharpnessControl = command::Sharpness::Up;
            /// Decreases sharpness by one step.
            plain decrease_sharpness() where HasSharpnessControl = command::Sharpness::Down;
            /// Sets a direct sharpness level.
            plain set_sharpness(level: types::SharpnessLevel) where HasSharpnessControl
                = command::Sharpness::SetLevel { value: level.value() };
            /// Returns backlight compensation state.
            inquiry backlight() -> bool where HasBacklightCompensation
                = command::BacklightInquiry;
            /// Enables or disables backlight compensation.
            plain set_backlight(enabled: bool) where HasBacklightCompensation
                = command::BacklightCommand::new(enabled);
            /// Returns 2D noise reduction level.
            inquiry noise_reduction_2d() -> types::NoiseReduction2DLevel where HasNoiseReduction2D
                = command::NoiseReduction2DInquiry;
            /// Sets 2D noise reduction level.
            plain set_noise_reduction_2d(level: types::NoiseReduction2DLevel)
                where HasNoiseReduction2D
                = command::NoiseReduction2D::with_level(level);
            /// Disables 2D noise reduction.
            plain disable_noise_reduction_2d() where HasNoiseReduction2D
                = command::NoiseReduction2D::off();
            /// Returns 3D noise reduction level.
            inquiry noise_reduction_3d() -> types::NoiseReduction3DLevel where HasNoiseReduction3D
                = command::NoiseReduction3DInquiry;
            /// Sets 3D noise reduction level.
            plain set_noise_reduction_3d(level: types::NoiseReduction3DLevel)
                where HasNoiseReduction3D
                = command::NoiseReduction3D::with_level(level);
            /// Disables 3D noise reduction.
            plain disable_noise_reduction_3d() where HasNoiseReduction3D
                = command::NoiseReduction3D::off();
            /// Returns the aggregate noise-reduction level.
            inquiry noise_reduction_level() -> types::NoiseReductionLevel where HasNoiseReduction
                = command::NrLevelInquiry;
            /// Returns the aggregate noise-reduction mode.
            inquiry noise_reduction_mode() -> command::NoiseReductionMode where HasNoiseReduction
                = command::NrModeInquiry;
            /// Disables vertical image flip.
            plain disable_flip() where HasImageFlip
                = builtin::ImageFlipCommand::new(command::Flip::Off);
            /// Enables vertical image flip.
            plain enable_flip() where HasImageFlip
                = builtin::ImageFlipCommand::new(command::Flip::On);
            /// Enables horizontal image mirroring.
            plain enable_horizontal_flip() where HasImageMirror
                = builtin::ImageMirrorCommand::new(true);
            /// Disables horizontal image mirroring.
            plain disable_horizontal_flip() where HasImageMirror
                = builtin::ImageMirrorCommand::new(false);
            /// Sets the combined image-flip mode to both axes.
            ///
            /// This sends the combined flip opcode, so it carries the same
            /// `HasCombinedImageFlip` bound as [`Self::set_flip_mode`].
            plain set_flip_both() where HasCombinedImageFlip
                = command::ImageFlipCombinedCommand::new(command::ImageFlipMode::Both);
            /// Sets the combined image-flip mode.
            plain set_flip_mode(mode: command::ImageFlipMode) where HasCombinedImageFlip
                = command::ImageFlipCombinedCommand::new(mode);
            /// Freezes the image.
            plain freeze_on() = command::ImageFreeze::on();
            /// Resumes live image output.
            plain freeze_off() = command::ImageFreeze::off();
            /// Returns the canonical image-flip state.
            inquiry flip() -> command::FlipState where HasImageFlip = command::ImageFlipInquiry;
            /// Returns the combined image-flip mode.
            inquiry flip_mode() -> command::FlipState where HasImageFlip
                = command::FlipStateInquiry;
            /// Returns whether black-and-white mode is active.
            inquiry black_white() -> bool where HasPictureEffect = command::BlackWhiteInquiry;
            /// Returns black-and-white mode.
            inquiry black_white_mode() -> command::BlackWhiteMode where HasPictureEffect
                = command::BlackWhiteModeInquiry;
            /// Returns picture-effect mode.
            inquiry picture_effect() -> command::PictureEffectMode where HasPictureEffect
                = command::PictureEffectInquiry;
            /// Sets picture-effect mode.
            plain set_picture_effect(mode: command::PictureEffectMode) where HasPictureEffect
                = command::PictureEffectCommand::new(mode);
            /// Returns the camera's defog level.
            inquiry defog_level() -> types::DefogLevel = command::DefogLevelInquiry;
        }
    };

    (Tally => $consumer:ident) => {
        $consumer! {
            /// Returns all tally light state.
            inquiry status() -> command::TallyStatusState = command::TallyStatusInquiry;
            /// Turns the red tally on.
            plain red_on() = command::TallyRedOn::new();
            /// Turns the red tally off.
            plain red_off() = command::TallyRedOff::new();
            /// Sets low tally brightness.
            plain bright_lo() = command::TallyBrightLo::new();
            /// Sets high tally brightness.
            plain bright_hi() = command::TallyBrightHi::new();
            /// Turns the green tally on.
            plain green_on() = command::TallyGreenOn::new();
            /// Turns the green tally off.
            plain green_off() = command::TallyGreenOff::new();
            /// Sets tally flash mode.
            plain flash() = command::TallyFlash::new();
            /// Sets tally solid-on mode.
            plain on() = command::TallyOn::new();
            /// Turns tally output off.
            plain off() = command::TallyOff::new();
            /// Returns red tally state.
            inquiry red_status() -> bool = command::TallyRedInquiry;
            /// Returns green tally state.
            inquiry green_status() -> bool = command::TallyGreenInquiry;
            /// Returns automatic tally adjustment state.
            inquiry auto_adjust_enabled() -> bool = command::TallyAutoAdjustInquiry;
        }
    };

    (NdFilter => $consumer:ident) => {
        $consumer! {
            /// Returns the current ND-filter position.
            inquiry position() -> command::NdFilterPosition = command::NdFilterInquiry;
            /// Returns the current ND-filter preset.
            inquiry preset() -> types::NdFilterPreset = command::NdFilterPresetInquiry;
            /// Selects preset or variable ND-filter mode.
            plain set_mode(mode: command::NdFilterMode) = command::NdFilterModeCommand::new(mode);
            /// Sets a direct variable ND-filter value.
            targeted set_value(value: u16)
                = checked command::NdFilterValue::new(value).map(builtin::NdFilterDirect::new);
            /// Sets a direct variable ND-filter value in photographic stops.
            ///
            /// `stops` is the light reduction in stops and must lie in `2.0..=7.0`.
            /// Each raw unit is a quarter stop, so `2.0` maps to the minimum density
            /// and `7.0` to the maximum.
            targeted set_stops(stops: f32)
                = checked command::NdFilterValue::from_stops(stops)
                    .map(builtin::NdFilterDirect::new);
            /// Increases ND-filter density by one step.
            targeted step_up() = builtin::NdFilterStepUp::new();
            /// Decreases ND-filter density by one step.
            targeted step_down() = builtin::NdFilterStepDown::new();
            /// Enables automatic ND filtering.
            plain auto_on() = command::AutoNdCommand::new(true);
            /// Disables automatic ND filtering.
            plain auto_off() = command::AutoNdCommand::new(false);
        }
    };

    (MotionSync => $consumer:ident) => {
        $consumer! {
            /// Returns the motion-sync mode.
            inquiry mode() -> command::MotionSyncMode = command::MotionSyncModeInquiry;
            /// Returns the motion-sync preset speed.
            inquiry preset() -> command::MotionSyncPreset = command::MotionSyncPresetInquiry;
            /// Enables or disables motion synchronization.
            plain set_mode(mode: command::MotionSyncMode)
                = command::SetMotionSyncMode::new(mode);
            /// Sets the motion-sync speed preset.
            plain set_preset(speed: u8) = checked command::SetMotionSyncPreset::new(speed);
            /// Sets the motion-sync speed from a range-checked speed value.
            ///
            /// This is [`Self::set_preset`] with the `1..=24` bound moved into the
            /// argument type, so an out-of-range speed cannot be constructed.
            plain set_speed(speed: types::MotionSyncSpeed)
                = checked command::SetMotionSyncPreset::new(speed.value());
        }
    };

    (Menu => $consumer:ident) => {
        $consumer! {
            /// Returns whether the on-screen menu is open.
            inquiry status() -> bool = command::MenuOpenCloseInquiry;
            /// Displays or hides the on-screen menu.
            plain display(on: bool) = command::SetMenuDisplay::new(on);
            /// Moves the menu cursor.
            plain navigate(direction: command::MenuDirection)
                = command::MenuNavigate::new(direction);
            /// Selects the current menu item.
            plain select() = command::PerformMenuAction::new(command::MenuAction::Select);
            /// Cancels or returns from the current menu item.
            plain cancel() = command::PerformMenuAction::new(command::MenuAction::Cancel);
            /// Sends a vendor-specific direct menu control.
            plain direct(control1: u8, control2: u8) where HasDirectMenuControl
                = command::DirectMenuControl::new(control1, control2);
            /// Toggles the on-screen menu open or closed.
            ///
            /// This is the vendor open/close direct control, so it needs no prior
            /// [`Self::status`] round trip to decide which way to move.
            plain toggle_display() where HasDirectMenuControl
                = command::DirectMenuControl::open_close();
        }
    };

    (Advanced => $consumer:ident) => {
        $consumer! {
            /// Returns night/day mode.
            inquiry night_day_mode() -> bool = command::NightDayModeInquiry;
            /// Returns standby state.
            inquiry standby_enabled() -> bool = command::StandbyInquiry;
            /// Returns digital PTZ state.
            inquiry digital_ptz_enabled() -> bool = command::DigitalPtzInquiry;
            /// Returns auto-trace state.
            inquiry auto_trace_enabled() -> bool = command::AutoTraceInquiry;
            /// Returns focus-unlock state.
            inquiry focus_unlock() -> bool = command::FocusUnlockInquiry;
            /// Returns the broadcast domain.
            inquiry broadcast_domain() -> types::BroadcastDomain
                = command::BroadcastDomainInquiry;
            /// Returns USB-audio state.
            inquiry usb_audio_enabled() -> bool = command::UsbAudioInquiry;
            /// Returns two-tone mode.
            inquiry two_tone_mode_enabled() -> bool = command::TwoToneModeInquiry;
            /// Returns digital mode.
            inquiry digital_mode_enabled() -> bool = command::DigitalInquiry;
            /// Enables multicast streaming.
            plain multicast_on() = command::MulticastStreaming::On;
            /// Disables multicast streaming.
            plain multicast_off() = command::MulticastStreaming::Off;
            /// Sets NDI streaming quality.
            plain set_ndi_quality(quality: types::NdiQuality)
                = command::SetNdiQuality::new(quality);
            /// Enables USB audio.
            plain usb_audio_on() = command::UsbAudio::On;
            /// Disables USB audio.
            plain usb_audio_off() = command::UsbAudio::Off;
            /// Sets pan/tilt variable-speed mode.
            plain set_variable_speed_mode(mode: command::VariableSpeedMode) where HasVariableSpeed
                = command::SetVariableSpeedMode::new(mode);
        }
    };
}

pub(crate) use noun_table;
