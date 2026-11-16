//! Authoritative noun methods, canonical request types and capability gates.
//!
//! Each row names one public request under `command::<noun>`. Request types
//! retain their protocol-granular ledger identities privately. Facades,
//! convention metadata and runtime gate projections all consume this table.
//!
//! `checked` handles a fallible value constructor; `with_target` performs
//! profile-dependent lowering before admission. Unit requests are unit values.

/// Completion wording shared by generated noun documentation and metadata.
#[cfg(any(feature = "blocking", feature = "async", test))]
macro_rules! completion_doc {
    (inquiry) => { "Returns the decoded value after the inquiry completes." };
    (plain) => { "Returns after the command is accepted and protocol-completed." };
    (applied) => { "Returns an applied-only operation handle; dropping it does not stop motion." };
    (targeted) => { "Returns a targeted operation handle; applied() waits for protocol completion and settled() waits for profile-selected settlement evidence. Dropping it does not stop motion." };
}

#[cfg(any(feature = "blocking", feature = "async", test))]
pub(crate) use completion_doc;

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
            /// Reads the power status, including standby and an internal circuit error.
            ///
            /// The result is the categorical `PowerStatus::On`, `Standby` or
            /// `CircuitError`; it is not a boolean or a numeric power level. The profile's
            /// power domain declares availability.
            inquiry [StatusInquiry] status() -> types::PowerStatus
                = [command::power::StatusInquiry];
            /// Powers the camera on.
            plain [On] on() -> command::power::On
                = [command::power::On];
            /// Places the camera in standby.
            plain [Standby] standby() -> command::power::Standby where HasPowerStandby
                = [command::power::Standby];

            @noun Zoom {
                accessor: ZoomAccessor,
                getter: zoom,
                dyn_trait: DynZoom,
                gate: [domain HasZoom],
                doc: "Optical and digital zoom commands and the zoom-position inquiry.",
            };
            /// Returns the current optical/digital zoom position.
            ///
            /// Positions are `ZoomPosition` codes, not millimetres or magnification
            /// ratios. The selected profile's `ZoomCaps::optical_max` and optional
            /// `digital_max` define the usable position domain; the type's union is
            /// `0x0000..=0x7FFF`.
            inquiry [PositionInquiry] position() -> types::ZoomPosition
                = [command::zoom::PositionInquiry];
            /// Drives toward telephoto at standard speed.
            applied [Tele] tele() -> command::zoom::Tele
                = [command::zoom::Tele];
            /// Drives toward wide angle at standard speed.
            applied [Wide] wide() -> command::zoom::Wide
                = [command::zoom::Wide];
            /// Stops zoom movement.
            applied [Stop] stop() -> command::zoom::Stop
                = [command::zoom::Stop];
            /// Drives toward telephoto at a validated variable speed.
            ///
            /// An exact `ZoomSpeed` is a unitless protocol speed code in `0..=7`; zero is
            /// the slowest drive, not STOP. `SpeedLevel` resolves within the selected
            /// profile's `ZoomCaps::speed` range, which also bounds exact inputs.
            applied [TeleVariable] tele_variable(speed: types::Speed<types::ZoomSpeed>) -> command::zoom::TeleVariable
                = [with_target |target| command::zoom::TeleVariable::for_target(target, speed)];
            /// Drives toward wide angle at a validated variable speed.
            ///
            /// An exact `ZoomSpeed` is a unitless protocol speed code in `0..=7`; zero is
            /// the slowest drive, not STOP. `SpeedLevel` resolves within the selected
            /// profile's `ZoomCaps::speed` range, which also bounds exact inputs.
            applied [WideVariable] wide_variable(speed: types::Speed<types::ZoomSpeed>) -> command::zoom::WideVariable
                = [with_target |target| command::zoom::WideVariable::for_target(target, speed)];
            /// Moves to an absolute zoom position.
            ///
            /// Positions are `ZoomPosition` codes, not millimetres or magnification
            /// ratios. The selected profile's `ZoomCaps::optical_max` and optional
            /// `digital_max` define the usable position domain; the type's union is
            /// `0x0000..=0x7FFF`.
            targeted [SetPosition] set_position(position: types::ZoomPosition) -> command::zoom::SetPosition where HasDirectZoom
                = [command::zoom::SetPosition::new(position)];
            /// Moves to a normalized position across a documented zoom domain.  `0.0` is the wide end and `1.0` the
            /// telephoto end of the selected domain. [`crate::types::ZoomDomain::Optical`] spans the profile's documented optical
            /// range; [`crate::types::ZoomDomain::OpticalPlusDigital`] requires the profile to document a digital maximum and
            /// never falls back to the optical range.
            ///
            /// The value is a dimensionless `UnitInterval` in `0.0..=1.0`.
            /// `ZoomCaps::position_at` and `normalize` use the selected `ZoomDomain` and
            /// the profile's `optical_max` or documented `digital_max`; they do not
            /// normalize against the type's union maximum.
            targeted [SetNormalized] set_normalized(position: types::UnitInterval, domain: types::ZoomDomain) -> command::zoom::SetNormalized where HasDirectZoom
                = [with_target |target| command::zoom::SetNormalized::for_target(target, position, domain)];
            /// Enables or disables digital zoom.
            ///
            /// `true` enables digital zoom and `false` disables it. This boolean has no
            /// numeric range; the selected profile's `DigitalZoomToggle` grant supplies
            /// the documented control.
            plain [SetDigitalZoom] set_digital_zoom(enabled: bool) -> command::zoom::SetDigitalZoom where HasDigitalZoomToggle
                = [command::zoom::SetDigitalZoom::new(enabled)];
            /// Drives zoom toward telephoto using a high-resolution speed code in `0x0000..=0x7FFE`. Zero selects
            /// the slowest drive speed; use `stop_high_resolution()` to stop movement (R7 p. 16). Availability
            /// depends on the attached lens and current camera state.
            applied [TeleHighResolution] tele_high_resolution(speed: types::HighResolutionZoomSpeed) -> command::zoom::TeleHighResolution where HasHighResolutionZoom
                = [command::zoom::TeleHighResolution::new(speed)];
            /// Drives zoom toward wide angle using a high-resolution speed code in `0x0000..=0x7FFE`. Zero selects
            /// the slowest drive speed; use `stop_high_resolution()` to stop movement (R7 p. 16). Availability
            /// depends on the attached lens and current camera state.
            applied [WideHighResolution] wide_high_resolution(speed: types::HighResolutionZoomSpeed) -> command::zoom::WideHighResolution where HasHighResolutionZoom
                = [command::zoom::WideHighResolution::new(speed)];
            /// Requests a stop of high-resolution zoom drive using the dedicated stop command, which is distinct
            /// from driving at speed zero (R7 p. 16).
            applied [StopHighResolution] stop_high_resolution() -> command::zoom::StopHighResolution where HasHighResolutionZoom
                = [command::zoom::StopHighResolution];
            /// Reads the zoom position across the optical and Clear Image Zoom ranges in position units: `0x0000`
            /// is optical wide, `0x4000` optical tele, `0x5556` the 1.5× endpoint at 3840×2160 or higher, and
            /// `0x6000` the 2× endpoint. This readback does not provide a direct position setter (R7 pp. 13, 22 and
            /// 26).
            inquiry [ClearImagePositionInquiry] clear_image_position() -> types::ClearImageZoomPosition where HasClearImageZoom
                = [command::zoom::ClearImagePositionInquiry];
            /// Reads zoom position as a checked `0.0..=1.0` fraction of the selected profile domain. The
            /// optical-plus-digital domain requires a documented digital maximum.
            ///
            /// The value is a dimensionless `UnitInterval` in `0.0..=1.0`.
            /// `ZoomCaps::position_at` and `normalize` use the selected `ZoomDomain` and
            /// the profile's `optical_max` or documented `digital_max`; they do not
            /// normalize against the type's union maximum.
            inquiry [NormalizedInquiry] normalized(domain: types::ZoomDomain) -> types::UnitInterval where HasDirectZoom
                = [with_target |target| command::zoom::NormalizedInquiry::for_target(target, domain)];

            @noun System {
                accessor: SystemAccessor,
                getter: system,
                dyn_trait: DynSystem,
                gate: [always],
                doc: "Settings persistence and the version inquiry.",
            };
            /// Returns the camera's Sony-format `CAM_VersionInq` reply: vendor, model, ROM revision and maximum
            /// socket.  Gated by `HasVersionInquiry`: profiles whose cameras reply in another, unsourced layout
            /// (PTZOptics) do not expose it. Send the raw `81 09 00 02 FF` inquiry there instead.
            ///
            /// `VersionInfo` contains vendor, model and ROM-version identifiers plus the
            /// reported command-socket count. These are reported identifiers and counts,
            /// not a semantic-version string; the profile's version-inquiry grant selects
            /// the supported reply contract.
            inquiry [VersionInquiry] version() -> types::VersionInfo where HasVersionInquiry
                = [command::system::VersionInquiry];
            /// Saves the camera's current settings to non-volatile storage.
            plain [SaveSettings] save_settings() -> command::system::SaveSettings where HasSettingsSave
                = [command::system::SaveSettings];

            @noun PanTilt {
                accessor: PanTiltAccessor,
                getter: pan_tilt,
                dyn_trait: DynPanTilt,
                gate: [domain HasPanTilt],
                doc: "Pan/tilt movement, limits, and the position inquiry.",
            };
            /// Reads pan and tilt in degrees using the registered target's coordinate conversion. A profile without
            /// a declared conversion returns `FeatureNotSupported`.
            ///
            /// `PanTiltPosition` stores checked degrees on both axes: pan is positive
            /// right and tilt positive up. The target's registered
            /// `PanTiltCoordinateConversion` maps its profile's `PanTiltCaps::pan_range`
            /// and mount-selected `tilt_ranges` into degree limits.
            inquiry [PositionInquiry] position() -> types::PanTiltPosition
                = [mapped [command::pan_tilt::PositionInquiry] |reply, profile| {
                    let coordinates = profile.pan_tilt_coordinates().ok_or(crate::Error::FeatureNotSupported { feature: "pan/tilt coordinate conversion" })?;
                    types::PanTiltPosition::from_degrees(coordinates.pan_degrees(reply.pan), coordinates.tilt_degrees(reply.tilt))
                }];
            /// Moves the pan/tilt mechanism to its home position.
            targeted [Home] home() -> command::pan_tilt::Home
                = [command::pan_tilt::Home];
            /// Resets the pan/tilt mechanism.
            targeted [Reset] reset() -> command::pan_tilt::Reset
                = [command::pan_tilt::Reset];
            /// Drives in the selected direction at coarse or exact pan/tilt speeds. The target profile supplies the
            /// allowed speed ranges; send `stop()` to end continuous motion.
            ///
            /// `PanTiltDirection` selects an active direction; use `stop()` rather than
            /// its STOP selector to stop. `Speed<PanTiltSpeed>` resolves coarse levels
            /// through the profile's `PanTiltCaps::pan_speed` and `tilt_speed`; exact
            /// speeds use checked protocol codes, not degrees per second. Active axes
            /// require nonzero speeds.
            applied [Drive] drive(direction: types::PanTiltDirection, speed: types::Speed<types::PanTiltSpeed>) -> command::pan_tilt::Drive
                = [with_target |target| command::pan_tilt::Drive::for_target(target, direction, speed)];
            /// Stops pan/tilt movement using profile-safe stop speeds.
            applied [Stop] stop() -> command::pan_tilt::Stop
                = [with_target |target| command::pan_tilt::Stop::for_target(target)];
            /// Moves to an absolute position in degrees within the target profile's coordinate limits. Coarse speed
            /// levels are lowered through that profile; exact speeds retain their checked axis values.
            ///
            /// `PanTiltPosition` stores checked degrees on both axes: pan is positive
            /// right and tilt positive up. The target's registered
            /// `PanTiltCoordinateConversion` maps its profile's `PanTiltCaps::pan_range`
            /// and mount-selected `tilt_ranges` into degree limits. The speed is
            /// `Speed<PanTiltSpeed>`: each exact axis uses a unitless protocol code, while
            /// `SpeedLevel` resolves through `PanTiltCaps::pan_speed` and `tilt_speed`.
            /// Position moves also honor the profile's paired-speed grammar and confirmed
            /// speed mode.
            targeted [Absolute] absolute(position: types::PanTiltPosition, speed: types::Speed<types::PanTiltSpeed>) -> command::pan_tilt::Absolute
                = [with_target |target| command::pan_tilt::Absolute::for_target(target, position, speed)];
            /// Moves by a relative degree offset. Each axis accepts the span of the target profile's absolute
            /// limits; speed lowering uses that profile's position-command grammar.
            ///
            /// `PanTiltPosition` stores checked degrees on both axes: pan is positive
            /// right and tilt positive up. The target's registered
            /// `PanTiltCoordinateConversion` maps its profile's `PanTiltCaps::pan_range`
            /// and mount-selected `tilt_ranges` into degree limits. The speed is
            /// `Speed<PanTiltSpeed>`: each exact axis uses a unitless protocol code, while
            /// `SpeedLevel` resolves through `PanTiltCaps::pan_speed` and `tilt_speed`.
            /// Position moves also honor the profile's paired-speed grammar and confirmed
            /// speed mode.
            targeted [Relative] relative(offset: types::PanTiltPosition, speed: types::Speed<types::PanTiltSpeed>) -> command::pan_tilt::Relative
                = [with_target |target| command::pan_tilt::Relative::for_target(target, offset, speed)];
            /// Sets one motion-limit corner in degrees using the registered target's coordinate conversion and
            /// documented position limits.
            ///
            /// `PanTiltPosition` stores checked degrees on both axes: pan is positive
            /// right and tilt positive up. The target's registered
            /// `PanTiltCoordinateConversion` maps its profile's `PanTiltCaps::pan_range`
            /// and mount-selected `tilt_ranges` into degree limits.
            plain [SetLimit] set_limit(corner: types::PanTiltLimitCorner, position: types::PanTiltPosition) -> command::pan_tilt::SetLimit
                = [with_target |target| command::pan_tilt::SetLimit::for_target(target, corner, position)];
            /// Clears one pan/tilt movement-limit corner.
            ///
            /// `PanTiltLimitCorner` selects the documented upper-right or lower-left
            /// corner. This clears that corner's configured position bound; it does not
            /// move the camera or take a new numeric position.
            plain [ClearLimit] clear_limit(corner: types::PanTiltLimitCorner) -> command::pan_tilt::ClearLimit
                = [with_target |target| command::pan_tilt::ClearLimit::for_target(target, corner)];
            /// Selects the documented pan/tilt speed-step range: normal 24-step or extended 50-step control (R7
            /// pan/tilt speed-step command).
            ///
            /// `PanTiltSpeedMode` selects the documented standard or extended speed-code
            /// mode, not a physical angular speed. Availability comes from the profile's
            /// `PanTiltSpeedMode` grant; subsequent speed preparation also checks the
            /// confirmed mode.
            plain [SetSpeedMode] set_speed_mode(mode: types::PanTiltSpeedMode) -> command::pan_tilt::SetSpeedMode where HasPanTiltSpeedMode
                = [command::pan_tilt::SetSpeedMode::new(mode)];

            @noun Focus {
                accessor: FocusAccessor,
                getter: focus,
                dyn_trait: DynFocus,
                gate: [domain HasFocus],
                doc: "Focus movement, modes, and focus inquiries.",
            };
            /// Returns the current focus position.
            ///
            /// Positions are `FocusPosition` codes in the four-nibble union
            /// `0x0000..=0xFFFF`, not distances in metres. The selected profile's
            /// `FocusCaps::range` supplies the supported domain.
            inquiry [PositionInquiry] position() -> types::FocusPosition
                = [command::focus::PositionInquiry];
            /// Returns the current focus mode.
            ///
            /// The mode is the categorical `FocusMode::Auto` or `Manual`; automatic mode
            /// additionally requires the selected profile's `FocusCaps::auto_focus` fact.
            /// Mode changes are not focus-position targets.
            inquiry [ModeInquiry] mode() -> types::FocusMode
                = [command::focus::ModeInquiry];
            /// Drives focus farther at standard speed.
            applied [Far] far() -> command::focus::Far
                = [command::focus::Far];
            /// Drives focus nearer at standard speed.
            applied [Near] near() -> command::focus::Near
                = [command::focus::Near];
            /// Drives focus farther at a variable speed.
            ///
            /// An exact `FocusSpeed` is a unitless protocol speed code in `0..=7`; zero is
            /// the slowest drive, not STOP. `SpeedLevel` resolves within the selected
            /// profile's `FocusCaps::speed` range, which also bounds exact inputs.
            applied [FarVariable] far_variable(speed: types::Speed<types::FocusSpeed>) -> command::focus::FarVariable
                = [with_target |target| command::focus::FarVariable::for_target(target, speed)];
            /// Drives focus nearer at a variable speed.
            ///
            /// An exact `FocusSpeed` is a unitless protocol speed code in `0..=7`; zero is
            /// the slowest drive, not STOP. `SpeedLevel` resolves within the selected
            /// profile's `FocusCaps::speed` range, which also bounds exact inputs.
            applied [NearVariable] near_variable(speed: types::Speed<types::FocusSpeed>) -> command::focus::NearVariable
                = [with_target |target| command::focus::NearVariable::for_target(target, speed)];
            /// Stops focus movement.
            applied [Stop] stop() -> command::focus::Stop
                = [command::focus::Stop];
            /// Moves focus to an absolute position.
            ///
            /// Positions are `FocusPosition` codes in the four-nibble union
            /// `0x0000..=0xFFFF`, not distances in metres. The selected profile's
            /// `FocusCaps::range` supplies the supported domain.
            targeted [SetPosition] set_position(position: types::FocusPosition) -> command::focus::SetPosition
                = [command::focus::SetPosition::new(position)];
            /// Triggers one-push autofocus.
            applied [OnePush] one_push() -> command::focus::OnePush where HasOnePushFocus
                = [command::focus::OnePush];
            /// Moves focus to infinity.
            targeted [Infinity] infinity() -> command::focus::Infinity where HasFocusInfinity
                = [command::focus::Infinity];
            /// Toggles automatic/manual focus mode.
            ///
            /// The mode is the categorical `FocusMode::Auto` or `Manual`; automatic mode
            /// additionally requires the selected profile's `FocusCaps::auto_focus` fact.
            /// Mode changes are not focus-position targets.
            plain [ToggleMode] toggle_mode() -> command::focus::ToggleMode
                = [command::focus::ToggleMode];
            /// Triggers vendor snap focus.
            applied [Snap] snap() -> command::focus::Snap where HasSnapFocus
                = [command::focus::Snap];
            /// Selects a focus zone.
            ///
            /// `FocusZone` is a categorical image-area selector, including the preserved
            /// `Zone03` value whose physical area is not documented. The selected
            /// profile's `FocusCaps::zones` inventory determines which selectors may be
            /// sent.
            plain [SetZone] set_zone(zone: types::FocusZone) -> command::focus::SetZone where HasFocusZone
                = [command::focus::SetZone::new(zone)];
            /// Sets the autofocus sensitivity.
            ///
            /// `AutoFocusSensitivity` selects `Low`, `Normal` or `High`; these are
            /// response categories, not measured time constants. Availability is declared
            /// by the profile's `AutoFocusSensitivity` grant and
            /// `FocusCaps::af_sensitivity` fact.
            plain [SetSensitivity] set_sensitivity(sensitivity: types::AutoFocusSensitivity) -> command::focus::SetSensitivity where HasAutoFocusSensitivity
                = [command::focus::SetSensitivity::new(sensitivity)];
            /// Sets the minimum focus distance.
            ///
            /// Positions are `FocusPosition` codes in the four-nibble union
            /// `0x0000..=0xFFFF`, not distances in metres. The selected profile's
            /// `FocusCaps::range` supplies the supported domain.
            plain [SetNearLimit] set_near_limit(position: types::FocusPosition) -> command::focus::SetNearLimit where HasFocusNearLimit
                = [command::focus::SetNearLimit::new(position)];
            /// Sets the focus-lock mode.
            ///
            /// `true` locks focus adjustment and `false` releases it. This boolean control
            /// is separate from automatic/manual mode and is available through the
            /// profile's `FocusLock` grant.
            plain [SetLock] set_lock(enabled: bool) -> command::focus::SetLock where HasFocusLock
                = [command::focus::SetLock::new(enabled)];
            /// Presses the vendor Push-AF control.
            applied [PushAutoFocusPress] push_auto_focus_press() -> command::focus::PushAutoFocusPress where HasPushAutoFocus
                = [command::focus::PushAutoFocusPress];
            /// Releases the vendor Push-AF control.
            applied [PushAutoFocusRelease] push_auto_focus_release() -> command::focus::PushAutoFocusRelease where HasPushAutoFocus
                = [command::focus::PushAutoFocusRelease];
            /// Returns the configured focus near limit.
            ///
            /// Positions are `FocusPosition` codes in the four-nibble union
            /// `0x0000..=0xFFFF`, not distances in metres. The selected profile's
            /// `FocusCaps::range` supplies the supported domain.
            inquiry [NearLimitInquiry] near_limit() -> types::FocusPosition where HasFocusNearLimit
                = [command::focus::NearLimitInquiry];
            /// Returns the configured focus zone.
            ///
            /// `FocusZone` is a categorical image-area selector, including the preserved
            /// `Zone03` value whose physical area is not documented. The selected
            /// profile's `FocusCaps::zones` inventory determines which selectors may be
            /// sent.
            inquiry [ZoneInquiry] zone() -> types::FocusZone where HasFocusZoneInquiry
                = [command::focus::ZoneInquiry];
            /// Returns the autofocus sensitivity.
            ///
            /// `AutoFocusSensitivity` selects `Low`, `Normal` or `High`; these are
            /// response categories, not measured time constants. Availability is declared
            /// by the profile's `AutoFocusSensitivity` grant and
            /// `FocusCaps::af_sensitivity` fact.
            inquiry [SensitivityInquiry] sensitivity() -> types::AutoFocusSensitivity where HasAutoFocusSensitivity
                = [command::focus::SensitivityInquiry];
            /// Selects automatic or manual focus. The target profile validates mode support before encoding.
            ///
            /// The mode is the categorical `FocusMode::Auto` or `Manual`; automatic mode
            /// additionally requires the selected profile's `FocusCaps::auto_focus` fact.
            /// Mode changes are not focus-position targets.
            plain [SetMode] set_mode(mode: types::FocusMode) -> command::focus::SetMode
                = [command::focus::SetMode::new(mode)];

            @noun Presets {
                accessor: PresetsAccessor,
                getter: presets,
                dyn_trait: DynPresets,
                gate: [domain HasPresets],
                doc: "Preset recall, store, clear, and recall-speed commands.",
            };
            /// Recalls a stored preset as a targeted operation.
            ///
            /// `PresetNumber` is a zero-based memory-slot index. Its supported domain is
            /// `0..=PresetCaps::highest` on the selected profile; constructing a value
            /// within the type's wider union does not establish camera support.
            targeted [Recall] recall(preset: types::PresetNumber) -> command::presets::Recall
                = [with_target |target| command::presets::Recall::for_target(target, preset)];
            /// Sets the speed level for later preset recalls without starting a recall. The target profile selects
            /// the encoding and range: PTZOptics uses levels `1..=24` (R1 command table), while FR7 common speed
            /// uses `1..=50` (R7 p. 19). These are speed levels rather than degrees per second.
            plain [SetRecallSpeed] set_recall_speed(speed: types::PresetSpeed) -> command::presets::SetRecallSpeed where HasPresetRecallSpeed
                = [with_target |target| command::presets::SetRecallSpeed::for_target(target, speed)];
            /// Stores the current camera state in a preset slot. The registered profile's preset domain validates
            /// the zero-based number before I/O.
            ///
            /// `PresetNumber` is a zero-based memory-slot index. Its supported domain is
            /// `0..=PresetCaps::highest` on the selected profile; constructing a value
            /// within the type's wider union does not establish camera support.
            plain [Store] store(preset: types::PresetNumber) -> command::presets::Store where HasPresetMemory
                = [command::presets::Store::new(preset)];
            /// Clears a stored preset slot. The registered profile's preset domain validates the zero-based number
            /// before I/O.
            ///
            /// `PresetNumber` is a zero-based memory-slot index. Its supported domain is
            /// `0..=PresetCaps::highest` on the selected profile; constructing a value
            /// within the type's wider union does not establish camera support.
            plain [Clear] clear(preset: types::PresetNumber) -> command::presets::Clear where HasPresetMemory
                = [command::presets::Clear::new(preset)];
            /// Selects how later preset recalls choose their transition setting: `Separate` uses each preset's
            /// configured speed or duration, while `Common` uses the shared recall speed. This configures recalls
            /// without starting one (R7 p. 19).
            plain [SetSpeedSelection] set_speed_selection(selection: types::PresetSpeedSelection) -> command::presets::SetSpeedSelection where HasSeparatePresetSpeed
                = [command::presets::SetSpeedSelection::new(selection)];
            /// Reads whether later recalls use per-preset speed/duration settings (`Separate`) or one shared recall
            /// speed (`Common`) (R7 pp. 19 and 22).
            inquiry [SpeedSelectionInquiry] speed_selection() -> types::PresetSpeedSelection where HasSeparatePresetSpeed
                = [command::presets::SpeedSelectionInquiry];
            /// Reads the shared preset-recall speed level, `1..=50` for the FR7 encoding. This is the value used
            /// when the speed selection is `Common` (R7 pp. 19 and 22).
            inquiry [RecallSpeedInquiry] recall_speed() -> types::PresetSpeed where HasPresetRecallSpeedInquiry
                = [command::presets::RecallSpeedInquiry];
            /// Selects whether the selected preset uses its configured speed or its configured duration on a later
            /// recall. `PresetNumber` is the zero-based packet slot `0..=99` for FR7, corresponding to menu presets
            /// `1..=100` (R7 p. 19).
            plain [SetSeparateMode] set_separate_mode(preset: types::PresetNumber, mode: types::PresetSeparateMode) -> command::presets::SetSeparateMode where HasSeparatePresetSpeed
                = [command::presets::SetSeparateMode::new(preset, mode)];
            /// Reads whether the selected preset uses its configured speed or its configured duration on a later
            /// recall. `PresetNumber` is the zero-based packet slot `0..=99` for FR7, corresponding to menu presets
            /// `1..=100` (R7 pp. 19 and 22).
            inquiry [SeparateModeInquiry] separate_mode(preset: types::PresetNumber) -> types::PresetSeparateMode where HasSeparatePresetSpeed
                = [command::presets::SeparateModeInquiry::new(preset)];
            /// Sets the speed level for the selected preset's later recall, `1..=50` on FR7. It applies when
            /// selection is `Separate` and that preset's mode is `Speed`; the packet slot `0..=99` corresponds to
            /// menu presets `1..=100` (R7 p. 19).
            plain [SetSeparateSpeed] set_separate_speed(preset: types::PresetNumber, speed: types::PresetSpeed) -> command::presets::SetSeparateSpeed where HasSeparatePresetSpeed
                = [command::presets::SetSeparateSpeed::new(preset, speed)];
            /// Reads the speed level for the selected preset's later recall, `1..=50` on FR7. It applies when
            /// selection is `Separate` and that preset's mode is `Speed`; the packet slot `0..=99` corresponds to
            /// menu presets `1..=100` (R7 pp. 19 and 22).
            inquiry [SeparateSpeedInquiry] separate_speed(preset: types::PresetNumber) -> types::PresetSpeed where HasSeparatePresetSpeed
                = [command::presets::SeparateSpeedInquiry::new(preset)];
            /// Sets the transition duration for the selected preset's later recall, from 1 to 99 seconds in exact
            /// 100 ms increments. It applies when selection is `Separate` and that preset's mode is `Duration`;
            /// packet slots `0..=99` correspond to menu presets `1..=100` (R7 p. 19).
            plain [SetSeparateDuration] set_separate_duration(preset: types::PresetNumber, duration: std::time::Duration) -> command::presets::SetSeparateDuration where HasSeparatePresetDuration
                = [checked command::presets::SetSeparateDuration::new(preset, duration)];
            /// Reads the transition duration for the selected preset's later recall, from 1 to 99 seconds in exact
            /// 100 ms increments. It applies when selection is `Separate` and that preset's mode is `Duration`;
            /// packet slots `0..=99` correspond to menu presets `1..=100` (R7 pp. 19 and 22).
            inquiry [SeparateDurationInquiry] separate_duration(preset: types::PresetNumber) -> std::time::Duration where HasSeparatePresetDuration
                = [command::presets::SeparateDurationInquiry::new(preset)];

            @noun Exposure {
                accessor: ExposureAccessor,
                getter: exposure,
                dyn_trait: DynExposure,
                gate: [domain HasExposure],
                doc: "Exposure, iris, shutter, brightness, and gain commands and inquiries.",
            };
            /// Returns the active exposure mode.
            ///
            /// `ExposureMode` selects `Auto`, `Manual`, `Shutter`, `Iris` or `Bright`; the
            /// selected profile's `ExposureCaps::modes` inventory determines admissible
            /// modes. The selector has no numeric exposure unit.
            inquiry [ModeInquiry] mode() -> types::ExposureMode where HasExposureMode
                = [command::exposure::ModeInquiry];
            /// Sets the exposure mode.
            ///
            /// `ExposureMode` selects `Auto`, `Manual`, `Shutter`, `Iris` or `Bright`; the
            /// selected profile's `ExposureCaps::modes` inventory determines admissible
            /// modes. The selector has no numeric exposure unit.
            plain [SetMode] set_mode(mode: types::ExposureMode) -> command::exposure::SetMode where HasExposureMode
                = [command::exposure::SetMode::new(mode)];
            /// Returns the shutter speed.
            ///
            /// `ShutterSpeed` is a protocol table code, not a duration. The selected
            /// profile's `ExposureCaps::shutter_speeds` table defines supported codes and
            /// their `ExposureTime` values in seconds; one step requests the adjacent
            /// camera setting, not a fixed time delta.
            inquiry [ShutterInquiry] shutter() -> types::ShutterSpeed where HasShutter
                = [command::exposure::ShutterInquiry];
            /// Restores the camera's shutter default.
            ///
            /// `ShutterSpeed` is a protocol table code, not a duration. The selected
            /// profile's `ExposureCaps::shutter_speeds` table defines supported codes and
            /// their `ExposureTime` values in seconds; one step requests the adjacent
            /// camera setting, not a fixed time delta.
            plain [ShutterReset] shutter_reset() -> command::exposure::ShutterReset where HasShutter
                = [command::exposure::ShutterReset];
            /// Increases shutter speed by one camera-defined step.
            ///
            /// `ShutterSpeed` is a protocol table code, not a duration. The selected
            /// profile's `ExposureCaps::shutter_speeds` table defines supported codes and
            /// their `ExposureTime` values in seconds; one step requests the adjacent
            /// camera setting, not a fixed time delta.
            plain [ShutterUp] shutter_up() -> command::exposure::ShutterUp where HasShutter
                = [command::exposure::ShutterUp];
            /// Decreases shutter speed by one camera-defined step.
            ///
            /// `ShutterSpeed` is a protocol table code, not a duration. The selected
            /// profile's `ExposureCaps::shutter_speeds` table defines supported codes and
            /// their `ExposureTime` values in seconds; one step requests the adjacent
            /// camera setting, not a fixed time delta.
            plain [ShutterDown] shutter_down() -> command::exposure::ShutterDown where HasShutter
                = [command::exposure::ShutterDown];
            /// Sets an explicit shutter speed.
            ///
            /// `ShutterSpeed` is a protocol table code, not a duration. The selected
            /// profile's `ExposureCaps::shutter_speeds` table defines supported codes and
            /// their `ExposureTime` values in seconds; one step requests the adjacent
            /// camera setting, not a fixed time delta.
            plain [SetShutter] set_shutter(speed: types::ShutterSpeed) -> command::exposure::SetShutter where HasDirectShutter
                = [command::exposure::SetShutter::new(speed)];
            /// Returns the exposure compensation value.
            ///
            /// `ExposureCompensationLevel` is a signed adjustment-step count with union
            /// `-7..=7` and zero neutral. The profile's `ExposureCaps::compensation_range`
            /// supplies the supported bounds; the API does not assign a universal EV size
            /// to one step.
            inquiry [CompensationInquiry] compensation() -> types::ExposureCompensationLevel where HasExposureCompensation
                = [command::exposure::CompensationInquiry];
            /// Returns whether exposure compensation is enabled.
            ///
            /// `true` enables exposure compensation and `false` disables it without
            /// choosing a compensation level. Availability is the selected profile's
            /// `ExposureCompensation` grant; levels use the separate compensation-step
            /// domain.
            inquiry [CompensationEnabledInquiry] compensation_enabled() -> bool where HasExposureCompensation
                = [command::exposure::CompensationEnabledInquiry];
            /// Enables or disables exposure compensation without changing its selected level.
            ///
            /// `true` enables exposure compensation and `false` disables it without
            /// choosing a compensation level. Availability is the selected profile's
            /// `ExposureCompensation` grant; levels use the separate compensation-step
            /// domain.
            plain [SetCompensationEnabled] set_compensation_enabled(enabled: bool) -> command::exposure::SetCompensationEnabled where HasExposureCompensation
                = [command::exposure::SetCompensationEnabled::new(enabled)];
            /// Resets exposure compensation.
            ///
            /// `ExposureCompensationLevel` is a signed adjustment-step count with union
            /// `-7..=7` and zero neutral. The profile's `ExposureCaps::compensation_range`
            /// supplies the supported bounds; the API does not assign a universal EV size
            /// to one step.
            plain [CompensationReset] compensation_reset() -> command::exposure::CompensationReset where HasExposureCompensation
                = [command::exposure::CompensationReset];
            /// Increases exposure compensation by one step.
            ///
            /// `ExposureCompensationLevel` is a signed adjustment-step count with union
            /// `-7..=7` and zero neutral. The profile's `ExposureCaps::compensation_range`
            /// supplies the supported bounds; the API does not assign a universal EV size
            /// to one step.
            plain [CompensationUp] compensation_up() -> command::exposure::CompensationUp where HasExposureCompensation
                = [command::exposure::CompensationUp];
            /// Decreases exposure compensation by one step.
            ///
            /// `ExposureCompensationLevel` is a signed adjustment-step count with union
            /// `-7..=7` and zero neutral. The profile's `ExposureCaps::compensation_range`
            /// supplies the supported bounds; the API does not assign a universal EV size
            /// to one step.
            plain [CompensationDown] compensation_down() -> command::exposure::CompensationDown where HasExposureCompensation
                = [command::exposure::CompensationDown];
            /// Sets direct exposure compensation.
            ///
            /// `ExposureCompensationLevel` is a signed adjustment-step count with union
            /// `-7..=7` and zero neutral. The profile's `ExposureCaps::compensation_range`
            /// supplies the supported bounds; the API does not assign a universal EV size
            /// to one step.
            plain [SetCompensation] set_compensation(level: types::ExposureCompensationLevel) -> command::exposure::SetCompensation where HasExposureCompensation
                = [command::exposure::SetCompensation::new(level)];
            /// Sets the wide-dynamic-range level.
            ///
            /// `WideDynamicRangeLevel` is a protocol adjustment code, not a percentage or
            /// a number of photographic stops. Its checked union is `0..=8`; the profile's
            /// `WideDynamicRange` grant and `ExposureCaps::wide_dynamic_range` fact
            /// declare support.
            plain [SetWideDynamicRange] set_wide_dynamic_range(level: types::WideDynamicRangeLevel) -> command::exposure::SetWideDynamicRange where HasWideDynamicRange
                = [command::exposure::SetWideDynamicRange::new(level)];
            /// Returns the iris level.
            ///
            /// `IrisLevel` is a camera table code, not a physical F-number. The selected
            /// profile's `ExposureCaps::iris_range` defines the supported codes. One
            /// step requests the next camera setting, not a fixed F-number delta.
            inquiry [IrisInquiry] iris() -> types::IrisLevel where HasIris
                = [command::exposure::IrisInquiry];
            /// Resets the iris.
            ///
            /// `IrisLevel` is a camera table code, not a physical F-number. The selected
            /// profile's `ExposureCaps::iris_range` defines the supported codes. One
            /// step requests the next camera setting, not a fixed F-number delta.
            targeted [IrisReset] iris_reset() -> command::exposure::IrisReset where HasIris
                = [command::exposure::IrisReset];
            /// Increases the iris by one step.
            ///
            /// `IrisLevel` is a camera table code, not a physical F-number. The selected
            /// profile's `ExposureCaps::iris_range` defines the supported codes. One
            /// step requests the next camera setting, not a fixed F-number delta.
            targeted [IrisUp] iris_up() -> command::exposure::IrisUp where HasIris
                = [command::exposure::IrisUp];
            /// Decreases the iris by one step.
            ///
            /// `IrisLevel` is a camera table code, not a physical F-number. The selected
            /// profile's `ExposureCaps::iris_range` defines the supported codes. One
            /// step requests the next camera setting, not a fixed F-number delta.
            targeted [IrisDown] iris_down() -> command::exposure::IrisDown where HasIris
                = [command::exposure::IrisDown];
            /// Sets a direct iris level.
            ///
            /// `IrisLevel` is a camera table code, not a physical F-number. The selected
            /// profile's `ExposureCaps::iris_range` defines the supported codes. One
            /// step requests the next camera setting, not a fixed F-number delta.
            targeted [SetIris] set_iris(level: types::IrisLevel) -> command::exposure::SetIris where HasIris
                = [command::exposure::SetIris::new(level)];
            /// Returns exposure brightness.
            ///
            /// `BrightnessLevel` is a camera table code, not a physical brightness
            /// percentage. The selected profile's `ExposureCaps::brightness_range` defines
            /// the supported codes. A step requests the next camera setting rather than a
            /// universal physical delta.
            inquiry [BrightnessInquiry] brightness() -> types::BrightnessLevel where HasBrightness
                = [command::exposure::BrightnessInquiry];
            /// Resets exposure brightness.
            ///
            /// `BrightnessLevel` is a camera table code, not a physical brightness
            /// percentage. The selected profile's `ExposureCaps::brightness_range` defines
            /// the supported codes. A step requests the next camera setting rather than a
            /// universal physical delta.
            plain [BrightnessReset] brightness_reset() -> command::exposure::BrightnessReset where HasBrightness
                = [command::exposure::BrightnessReset];
            /// Increases exposure brightness.
            ///
            /// `BrightnessLevel` is a camera table code, not a physical brightness
            /// percentage. The selected profile's `ExposureCaps::brightness_range` defines
            /// the supported codes. A step requests the next camera setting rather than a
            /// universal physical delta.
            plain [BrightnessUp] brightness_up() -> command::exposure::BrightnessUp where HasBrightness
                = [command::exposure::BrightnessUp];
            /// Decreases exposure brightness.
            ///
            /// `BrightnessLevel` is a camera table code, not a physical brightness
            /// percentage. The selected profile's `ExposureCaps::brightness_range` defines
            /// the supported codes. A step requests the next camera setting rather than a
            /// universal physical delta.
            plain [BrightnessDown] brightness_down() -> command::exposure::BrightnessDown where HasBrightness
                = [command::exposure::BrightnessDown];
            /// Sets exposure brightness through the bright-direct command.
            ///
            /// `BrightnessLevel` is a camera table code, not a physical brightness
            /// percentage. The selected profile's `ExposureCaps::brightness_range` defines
            /// the supported codes. A step requests the next camera setting rather than a
            /// universal physical delta.
            plain [SetBrightness] set_brightness(level: types::BrightnessLevel) -> command::exposure::SetBrightness where HasBrightness
                = [command::exposure::SetBrightness::new(level)];
            /// Reads the current gain code from the camera. The selected profile
            /// supplies the supported gain-code domain; the result uses the same
            /// checked value type accepted by `set_gain`.
            ///
            /// `GainLevel` is a camera table code, not a physical gain multiplier. The
            /// selected profile's `ExposureCaps::gain_range` defines the supported codes.
            /// A step requests the next camera setting rather than a universal physical
            /// delta.
            inquiry [GainInquiry] gain() -> types::GainLevel where HasGain
                = [command::exposure::GainInquiry];
            /// Resets gain.
            ///
            /// `GainLevel` is a camera table code, not a physical gain multiplier. The
            /// selected profile's `ExposureCaps::gain_range` defines the supported codes.
            /// A step requests the next camera setting rather than a universal physical
            /// delta.
            plain [GainReset] gain_reset() -> command::exposure::GainReset where HasGain
                = [command::exposure::GainReset];
            /// Increases gain.
            ///
            /// `GainLevel` is a camera table code, not a physical gain multiplier. The
            /// selected profile's `ExposureCaps::gain_range` defines the supported codes.
            /// A step requests the next camera setting rather than a universal physical
            /// delta.
            plain [GainUp] gain_up() -> command::exposure::GainUp where HasGain
                = [command::exposure::GainUp];
            /// Decreases gain.
            ///
            /// `GainLevel` is a camera table code, not a physical gain multiplier. The
            /// selected profile's `ExposureCaps::gain_range` defines the supported codes.
            /// A step requests the next camera setting rather than a universal physical
            /// delta.
            plain [GainDown] gain_down() -> command::exposure::GainDown where HasGain
                = [command::exposure::GainDown];
            /// Sets direct gain.
            ///
            /// `GainLevel` is a camera table code, not a physical gain multiplier. The
            /// selected profile's `ExposureCaps::gain_range` defines the supported codes.
            /// A step requests the next camera setting rather than a universal physical
            /// delta.
            plain [SetGain] set_gain(level: types::GainLevel) -> command::exposure::SetGain where HasGain
                = [command::exposure::SetGain::new(level)];
            /// Returns the configured gain limit.
            ///
            /// `GainLimit` is a maximum-auto-gain protocol code with checked union
            /// `0x00..=0x0F`, not a gain multiplier. The profile's `GainLimit` grant and
            /// positive packet evidence determine whether this register may be used.
            inquiry [GainLimitInquiry] gain_limit() -> types::GainLimit where HasGainLimit
                = [command::exposure::GainLimitInquiry];
            /// Sets the configured gain limit.
            ///
            /// `GainLimit` is a maximum-auto-gain protocol code with checked union
            /// `0x00..=0x0F`, not a gain multiplier. The profile's `GainLimit` grant and
            /// positive packet evidence determine whether this register may be used.
            plain [SetGainLimit] set_gain_limit(limit: types::GainLimit) -> command::exposure::SetGainLimit where HasGainLimit
                = [command::exposure::SetGainLimit::new(limit)];
            /// Sets anti-flicker mode.
            ///
            /// `AntiFlickerMode` is the categorical off/50 Hz/60 Hz mains-flicker setting,
            /// not a shutter duration. Availability comes from the selected profile's
            /// `AntiFlicker` grant.
            plain [SetAntiFlicker] set_anti_flicker(mode: types::AntiFlickerMode) -> command::exposure::SetAntiFlicker where HasAntiFlicker
                = [command::exposure::SetAntiFlicker::new(mode)];
            /// Returns the configured anti-flicker mode.
            ///
            /// `AntiFlickerMode` is the categorical off/50 Hz/60 Hz mains-flicker setting,
            /// not a shutter duration. Availability comes from the selected profile's
            /// `AntiFlicker` grant.
            inquiry [AntiFlickerInquiry] anti_flicker() -> types::AntiFlickerMode where HasAntiFlicker
                = [command::exposure::AntiFlickerInquiry];
            /// Enables or disables the profile's spotlight exposure compensation.
            ///
            /// `true` enables spotlight exposure compensation and `false` disables it.
            /// This boolean uses the selected profile's `Spotlight` grant rather than a
            /// numeric compensation level.
            plain [SetSpotlight] set_spotlight(enabled: bool) -> command::exposure::SetSpotlight where HasSpotlight
                = [command::exposure::SetSpotlight::new(enabled)];
            /// Enables or disables automatic slow-shutter exposure on profiles with documented support.
            ///
            /// `true` permits automatic slow-shutter exposure and `false` disables that
            /// behavior. This boolean has no duration argument; availability comes from
            /// the selected profile's `AutoSlowShutter` grant.
            plain [SetAutoSlowShutter] set_auto_slow_shutter(enabled: bool) -> command::exposure::SetAutoSlowShutter where HasAutoSlowShutter
                = [command::exposure::SetAutoSlowShutter::new(enabled)];
            /// Raises the automatic-exposure target by one quarter EV within the documented −3.0 to +3.0 EV range
            /// (R7 p. 14).
            plain [AeLevelUp] ae_level_up() -> command::exposure::AeLevelUp where HasAeLevel
                = [command::exposure::AeLevelUp];
            /// Lowers the automatic-exposure target by one quarter EV within the documented −3.0 to +3.0 EV range
            /// (R7 p. 14).
            plain [AeLevelDown] ae_level_down() -> command::exposure::AeLevelDown where HasAeLevel
                = [command::exposure::AeLevelDown];
            /// Reads the automatic-exposure target adjustment as `AeLevel`: `-12..=12` quarter-EV units represent
            /// −3.0 to +3.0 EV, with zero neutral (R7 pp. 14 and 21).
            inquiry [AeLevelInquiry] ae_level() -> types::AeLevel where HasAeLevel
                = [command::exposure::AeLevelInquiry];
            /// Enables or disables automatic gain control (AGC) (R7 p. 14).
            plain [SetAutoGain] set_auto_gain(enabled: bool) -> command::exposure::SetAutoGain where HasAutoGain
                = [command::exposure::SetAutoGain::new(enabled)];
            /// Reads whether automatic gain control (AGC) is enabled (R7 pp. 14 and 21).
            inquiry [AutoGainInquiry] auto_gain() -> bool where HasAutoGain
                = [command::exposure::AutoGainInquiry];
            /// Enables or disables automatic shutter control (R7 p. 14).
            plain [SetAutoShutter] set_auto_shutter(enabled: bool) -> command::exposure::SetAutoShutter where HasAutoShutter
                = [command::exposure::SetAutoShutter::new(enabled)];
            /// Reads whether automatic shutter control is enabled (R7 pp. 14 and 21).
            inquiry [AutoShutterInquiry] auto_shutter() -> bool where HasAutoShutter
                = [command::exposure::AutoShutterInquiry];
            /// Enables or disables automatic iris control (R7 p. 14).
            plain [SetAutoIris] set_auto_iris(enabled: bool) -> command::exposure::SetAutoIris where HasAutoIris
                = [command::exposure::SetAutoIris::new(enabled)];
            /// Reads whether automatic iris control is enabled (R7 pp. 14 and 21).
            inquiry [AutoIrisInquiry] auto_iris() -> bool where HasAutoIris
                = [command::exposure::AutoIrisInquiry];
            /// Moves the iris brighter by the requested `1..=255` relative steps, approximately 1/256 EV per step.
            /// The count is an exposure adjustment rather than an absolute aperture position (R7 p. 14).
            applied [IrisUpBy] iris_up_by(steps: types::IrisStep) -> command::exposure::IrisUpBy where HasIrisSteps
                = [command::exposure::IrisUpBy::new(steps)];
            /// Moves the iris darker by the requested `1..=255` relative steps, approximately 1/256 EV per step.
            /// The count is an exposure adjustment rather than an absolute aperture position (R7 p. 14).
            applied [IrisDownBy] iris_down_by(steps: types::IrisStep) -> command::exposure::IrisDownBy where HasIrisSteps
                = [command::exposure::IrisDownBy::new(steps)];
            /// Reads whether the profile's backlight exposure compensation is enabled.
            ///
            /// `true` enables backlight compensation and `false` disables it. The
            /// profile's `ExposureCaps::backlight_compensation` fact and
            /// `BacklightCompensation` grant declare this boolean register.
            inquiry [BacklightCompensationInquiry] backlight_compensation() -> bool where HasBacklightCompensation
                = [command::exposure::BacklightCompensationInquiry];
            /// Enables or disables the profile's backlight exposure compensation.
            ///
            /// `true` enables backlight compensation and `false` disables it. The
            /// profile's `ExposureCaps::backlight_compensation` fact and
            /// `BacklightCompensation` grant declare this boolean register.
            plain [SetBacklightCompensation] set_backlight_compensation(enabled: bool) -> command::exposure::SetBacklightCompensation where HasBacklightCompensation
                = [command::exposure::SetBacklightCompensation::new(enabled)];

            @noun WhiteBalance {
                accessor: WhiteBalanceAccessor,
                getter: white_balance,
                dyn_trait: DynWhiteBalance,
                gate: [domain HasWhiteBalance],
                doc: "White-balance, color-temperature, and color-channel commands and inquiries.",
            };
            /// Returns the active white-balance mode.
            ///
            /// `WhiteBalanceMode` is a categorical operating-mode selector. Only values in
            /// the selected profile's `WhiteBalanceCaps::modes` inventory may be sent;
            /// setting a mode is separate from triggering `one_push()`.
            inquiry [ModeInquiry] mode() -> types::WhiteBalanceMode
                = [command::white_balance::ModeInquiry];
            /// Sets automatic white-balance sensitivity.
            ///
            /// `AutoWhiteBalanceSensitivity` selects `Low`, `Normal` or `High`, not a
            /// measured response time. Availability comes from the selected profile's
            /// `AutoWhiteBalanceSensitivity` grant.
            plain [SetSensitivity] set_sensitivity(sensitivity: types::AutoWhiteBalanceSensitivity) -> command::white_balance::SetSensitivity where HasAutoWhiteBalanceSensitivity
                = [command::white_balance::SetSensitivity::new(sensitivity)];
            /// Returns automatic white-balance sensitivity.
            ///
            /// `AutoWhiteBalanceSensitivity` selects `Low`, `Normal` or `High`, not a
            /// measured response time. Availability comes from the selected profile's
            /// `AutoWhiteBalanceSensitivity` grant.
            inquiry [SensitivityInquiry] sensitivity() -> types::AutoWhiteBalanceSensitivity where HasAutoWhiteBalanceSensitivity
                = [command::white_balance::SensitivityInquiry];
            /// Triggers one-shot white-balance measurement using the target profile's encoding without changing the
            /// selected mode. Standard VISCA uses its supported one-push mode. FR7 WB SET stores the scene
            /// measurement in Memory A, or briefly corrects ATW before tracking resumes; capture in Preset mode is
            /// not established (R7 p. 15; Sony FR7 Help Guide, “Running Auto White Balance”). Protocol completion
            /// does not independently establish white-balance quality.
            plain [OnePush] one_push() -> command::white_balance::OnePush where HasOnePushWhiteBalance
                = [command::white_balance::OnePush];
            /// Sets red-channel white-balance tuning.
            ///
            /// `RgbTuning` is a signed adjustment level in `-10..=10`, with zero neutral.
            /// The selected profile's `WhiteBalanceCaps::rgb_tuning.red` range supplies
            /// this channel's supported domain; the level is not a Kelvin offset.
            plain [SetRedTuning] set_red_tuning(level: types::RgbTuning) -> command::white_balance::SetRedTuning where HasRgbTuning
                = [command::white_balance::SetRedTuning::new(level)];
            /// Sets blue-channel white-balance tuning.
            ///
            /// `RgbTuning` is a signed adjustment level in `-10..=10`, with zero neutral.
            /// The selected profile's `WhiteBalanceCaps::rgb_tuning.blue` range supplies
            /// this channel's supported domain; the level is not a Kelvin offset.
            plain [SetBlueTuning] set_blue_tuning(level: types::RgbTuning) -> command::white_balance::SetBlueTuning where HasRgbTuning
                = [command::white_balance::SetBlueTuning::new(level)];
            /// Returns the color temperature.
            ///
            /// Temperature is physical Kelvin in the checked `Kelvin` union `2000..=15000
            /// K`. The selected profile's `WhiteBalanceCaps::color_temperature_steps`
            /// table lists its supported physical values and owns their wire-code mapping;
            /// steps do not imply a universal Kelvin delta.
            inquiry [ColorTemperatureInquiry] color_temperature() -> types::Kelvin where HasColorTemperatureInquiry
                = [command::white_balance::ColorTemperatureInquiry];
            /// Resets color temperature.
            ///
            /// Temperature is physical Kelvin in the checked `Kelvin` union `2000..=15000
            /// K`. The selected profile's `WhiteBalanceCaps::color_temperature_steps`
            /// table lists its supported physical values and owns their wire-code mapping;
            /// steps do not imply a universal Kelvin delta.
            plain [ColorTemperatureReset] color_temperature_reset() -> command::white_balance::ColorTemperatureReset where HasColorTemperature
                = [command::white_balance::ColorTemperatureReset];
            /// Increases color temperature.
            ///
            /// Temperature is physical Kelvin in the checked `Kelvin` union `2000..=15000
            /// K`. The selected profile's `WhiteBalanceCaps::color_temperature_steps`
            /// table lists its supported physical values and owns their wire-code mapping;
            /// steps do not imply a universal Kelvin delta.
            plain [ColorTemperatureUp] color_temperature_up() -> command::white_balance::ColorTemperatureUp where HasColorTemperature
                = [command::white_balance::ColorTemperatureUp];
            /// Decreases color temperature.
            ///
            /// Temperature is physical Kelvin in the checked `Kelvin` union `2000..=15000
            /// K`. The selected profile's `WhiteBalanceCaps::color_temperature_steps`
            /// table lists its supported physical values and owns their wire-code mapping;
            /// steps do not imply a universal Kelvin delta.
            plain [ColorTemperatureDown] color_temperature_down() -> command::white_balance::ColorTemperatureDown where HasColorTemperature
                = [command::white_balance::ColorTemperatureDown];
            /// Sets a direct color-temperature value.
            ///
            /// Temperature is physical Kelvin in the checked `Kelvin` union `2000..=15000
            /// K`. The selected profile's `WhiteBalanceCaps::color_temperature_steps`
            /// table lists its supported physical values and owns their wire-code mapping;
            /// steps do not imply a universal Kelvin delta.
            plain [SetColorTemperature] set_color_temperature(temperature: types::Kelvin) -> command::white_balance::SetColorTemperature where HasColorTemperature
                = [with_target |target| command::white_balance::SetColorTemperature::for_target(target, temperature)];
            /// Returns the red-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.red` range supplies this channel's supported
            /// codes.
            inquiry [RedGainInquiry] red_gain() -> types::RgbGain where HasRgbGain
                = [command::white_balance::RedGainInquiry];
            /// Resets red-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.red` range supplies this channel's supported
            /// codes.
            plain [RedGainReset] red_gain_reset() -> command::white_balance::RedGainReset where HasRgbGain
                = [command::white_balance::RedGainReset];
            /// Increases red-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.red` range supplies this channel's supported
            /// codes.
            plain [RedGainUp] red_gain_up() -> command::white_balance::RedGainUp where HasRgbGain
                = [command::white_balance::RedGainUp];
            /// Decreases red-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.red` range supplies this channel's supported
            /// codes.
            plain [RedGainDown] red_gain_down() -> command::white_balance::RedGainDown where HasRgbGain
                = [command::white_balance::RedGainDown];
            /// Sets direct red-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.red` range supplies this channel's supported
            /// codes.
            plain [SetRedGain] set_red_gain(gain: types::RgbGain) -> command::white_balance::SetRedGain where HasRgbGain
                = [command::white_balance::SetRedGain::new(gain)];
            /// Returns the blue-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.blue` range supplies this channel's supported
            /// codes.
            inquiry [BlueGainInquiry] blue_gain() -> types::RgbGain where HasRgbGain
                = [command::white_balance::BlueGainInquiry];
            /// Resets blue-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.blue` range supplies this channel's supported
            /// codes.
            plain [BlueGainReset] blue_gain_reset() -> command::white_balance::BlueGainReset where HasRgbGain
                = [command::white_balance::BlueGainReset];
            /// Increases blue-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.blue` range supplies this channel's supported
            /// codes.
            plain [BlueGainUp] blue_gain_up() -> command::white_balance::BlueGainUp where HasRgbGain
                = [command::white_balance::BlueGainUp];
            /// Decreases blue-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.blue` range supplies this channel's supported
            /// codes.
            plain [BlueGainDown] blue_gain_down() -> command::white_balance::BlueGainDown where HasRgbGain
                = [command::white_balance::BlueGainDown];
            /// Sets direct blue-channel gain.
            ///
            /// `RgbGain` is a channel protocol code in the union `0x00..=0xFF`, not a
            /// physical multiplier. The selected profile's
            /// `WhiteBalanceCaps::rgb_gain.blue` range supplies this channel's supported
            /// codes.
            plain [SetBlueGain] set_blue_gain(gain: types::RgbGain) -> command::white_balance::SetBlueGain where HasRgbGain
                = [command::white_balance::SetBlueGain::new(gain)];
            /// Returns red-channel tuning.
            ///
            /// `RgbTuning` is a signed adjustment level in `-10..=10`, with zero neutral.
            /// The selected profile's `WhiteBalanceCaps::rgb_tuning.red` range supplies
            /// this channel's supported domain; the level is not a Kelvin offset.
            inquiry [RedTuningInquiry] red_tuning() -> types::RgbTuning where HasRgbTuning
                = [command::white_balance::RedTuningInquiry];
            /// Returns blue-channel tuning.
            ///
            /// `RgbTuning` is a signed adjustment level in `-10..=10`, with zero neutral.
            /// The selected profile's `WhiteBalanceCaps::rgb_tuning.blue` range supplies
            /// this channel's supported domain; the level is not a Kelvin offset.
            inquiry [BlueTuningInquiry] blue_tuning() -> types::RgbTuning where HasRgbTuning
                = [command::white_balance::BlueTuningInquiry];
            /// Raises the preset white-balance color temperature by `steps` protocol adjustments (`1..=255`), with
            /// a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
            plain [PresetWhiteUpBy] preset_white_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::PresetWhiteUpBy where HasPresetWhite
                = [command::white_balance::PresetWhiteUpBy::new(steps)];
            /// Lowers the preset white-balance color temperature by `steps` protocol adjustments (`1..=255`), with
            /// a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
            plain [PresetWhiteDownBy] preset_white_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::PresetWhiteDownBy where HasPresetWhite
                = [command::white_balance::PresetWhiteDownBy::new(steps)];
            /// Sets the preset white-balance color temperature in Kelvin over `2000..=15000 K` (R7 p. 15).
            plain [SetPresetWhite] set_preset_white(temperature: types::Kelvin) -> command::white_balance::SetPresetWhite where HasPresetWhite
                = [command::white_balance::SetPresetWhite::new(temperature)];
            /// Reads the preset white-balance color temperature in Kelvin over `2000..=15000 K` (R7 pp. 15 and 21).
            inquiry [PresetWhiteInquiry] preset_white() -> types::Kelvin where HasPresetWhite
                = [command::white_balance::PresetWhiteInquiry];
            /// Raises the Memory A white-balance color temperature by `steps` protocol adjustments (`1..=30`), with
            /// a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
            plain [MemoryColorTemperatureUpBy] memory_color_temperature_up_by(steps: types::MemoryColorTemperatureStep) -> command::white_balance::MemoryColorTemperatureUpBy where HasMemoryColorTemperature
                = [command::white_balance::MemoryColorTemperatureUpBy::new(steps)];
            /// Lowers the Memory A white-balance color temperature by `steps` protocol adjustments (`1..=30`), with
            /// a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
            plain [MemoryColorTemperatureDownBy] memory_color_temperature_down_by(steps: types::MemoryColorTemperatureStep) -> command::white_balance::MemoryColorTemperatureDownBy where HasMemoryColorTemperature
                = [command::white_balance::MemoryColorTemperatureDownBy::new(steps)];
            /// Sets the Memory A white-balance color temperature in Kelvin over `2000..=15000 K` (R7 p. 15).
            plain [SetMemoryColorTemperature] set_memory_color_temperature(temperature: types::Kelvin) -> command::white_balance::SetMemoryColorTemperature where HasMemoryColorTemperature
                = [command::white_balance::SetMemoryColorTemperature::new(temperature)];
            /// Reads the Memory A white-balance color temperature in Kelvin over `2000..=15000 K` (R7 pp. 15 and
            /// 21).
            inquiry [MemoryColorTemperatureInquiry] memory_color_temperature() -> types::Kelvin where HasMemoryColorTemperature
                = [command::white_balance::MemoryColorTemperatureInquiry];
            /// Raises the Memory A red-channel white-balance gain by `1..=255` relative adjustment steps. Direct
            /// values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing
            /// gain can also change the related Memory A temperature/tint readback.
            plain [MemoryRedGainUpBy] memory_red_gain_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryRedGainUpBy where HasMemoryGain
                = [command::white_balance::MemoryRedGainUpBy::new(steps)];
            /// Lowers the Memory A red-channel white-balance gain by `1..=255` relative adjustment steps. Direct
            /// values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing
            /// gain can also change the related Memory A temperature/tint readback.
            plain [MemoryRedGainDownBy] memory_red_gain_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryRedGainDownBy where HasMemoryGain
                = [command::white_balance::MemoryRedGainDownBy::new(steps)];
            /// Sets the Memory A red-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents
            /// −99.0 to +99.0 displayed units, with zero neutral (R7 p. 15). Changing gain can also change the
            /// related Memory A temperature/tint readback.
            plain [SetMemoryRedGain] set_memory_red_gain(value: types::WhiteBalanceGain) -> command::white_balance::SetMemoryRedGain where HasMemoryGain
                = [command::white_balance::SetMemoryRedGain::new(value)];
            /// Reads the Memory A red-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents
            /// −99.0 to +99.0 displayed units, with zero neutral (R7 pp. 15 and 21).
            inquiry [MemoryRedGainInquiry] memory_red_gain() -> types::WhiteBalanceGain where HasMemoryGain
                = [command::white_balance::MemoryRedGainInquiry];
            /// Raises the Memory A blue-channel white-balance gain by `1..=255` relative adjustment steps. Direct
            /// values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing
            /// gain can also change the related Memory A temperature/tint readback.
            plain [MemoryBlueGainUpBy] memory_blue_gain_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryBlueGainUpBy where HasMemoryGain
                = [command::white_balance::MemoryBlueGainUpBy::new(steps)];
            /// Lowers the Memory A blue-channel white-balance gain by `1..=255` relative adjustment steps. Direct
            /// values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing
            /// gain can also change the related Memory A temperature/tint readback.
            plain [MemoryBlueGainDownBy] memory_blue_gain_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryBlueGainDownBy where HasMemoryGain
                = [command::white_balance::MemoryBlueGainDownBy::new(steps)];
            /// Sets the Memory A blue-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents
            /// −99.0 to +99.0 displayed units, with zero neutral (R7 p. 15). Changing gain can also change the
            /// related Memory A temperature/tint readback.
            plain [SetMemoryBlueGain] set_memory_blue_gain(value: types::WhiteBalanceGain) -> command::white_balance::SetMemoryBlueGain where HasMemoryGain
                = [command::white_balance::SetMemoryBlueGain::new(value)];
            /// Reads the Memory A blue-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents
            /// −99.0 to +99.0 displayed units, with zero neutral (R7 pp. 15 and 21).
            inquiry [MemoryBlueGainInquiry] memory_blue_gain() -> types::WhiteBalanceGain where HasMemoryGain
                = [command::white_balance::MemoryBlueGainInquiry];
            /// Raises the Memory A white-balance tint by `1..=255` relative adjustment steps within the documented
            /// −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15).
            plain [MemoryTintUpBy] memory_tint_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryTintUpBy where HasMemoryTint
                = [command::white_balance::MemoryTintUpBy::new(steps)];
            /// Lowers the Memory A white-balance tint by `1..=255` relative adjustment steps within the documented
            /// −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15).
            plain [MemoryTintDownBy] memory_tint_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::MemoryTintDownBy where HasMemoryTint
                = [command::white_balance::MemoryTintDownBy::new(steps)];
            /// Sets the Memory A white-balance tint as a signed offset level in `-99..=99`, with zero neutral. The
            /// offset is dimensionless rather than a Kelvin value (R7 p. 15).
            plain [SetMemoryTint] set_memory_tint(offset: types::WhiteBalanceOffset) -> command::white_balance::SetMemoryTint where HasMemoryTint
                = [command::white_balance::SetMemoryTint::new(offset)];
            /// Reads the Memory A white-balance tint as a signed offset level in `-99..=99`, with zero neutral. The
            /// offset is dimensionless rather than a Kelvin value (R7 pp. 15 and 21).
            inquiry [MemoryTintInquiry] memory_tint() -> types::WhiteBalanceOffset where HasMemoryTint
                = [command::white_balance::MemoryTintInquiry];
            /// Raises the ATW color-temperature offset by `1..=255` relative adjustment steps within the documented
            /// −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this
            /// offset only when its `Offset White<ATW>` setting permits it.
            plain [AtwColorTemperatureOffsetUpBy] atw_color_temperature_offset_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::AtwColorTemperatureOffsetUpBy where HasAtwOffsets
                = [command::white_balance::AtwColorTemperatureOffsetUpBy::new(steps)];
            /// Lowers the ATW color-temperature offset by `1..=255` relative adjustment steps within the documented
            /// −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this
            /// offset only when its `Offset White<ATW>` setting permits it.
            plain [AtwColorTemperatureOffsetDownBy] atw_color_temperature_offset_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::AtwColorTemperatureOffsetDownBy where HasAtwOffsets
                = [command::white_balance::AtwColorTemperatureOffsetDownBy::new(steps)];
            /// Sets the ATW color-temperature offset as a signed offset level in `-99..=99`, with zero neutral. The
            /// offset is dimensionless rather than a Kelvin value (R7 p. 15). The camera applies this offset only
            /// when its `Offset White<ATW>` setting permits it.
            plain [SetAtwColorTemperatureOffset] set_atw_color_temperature_offset(offset: types::WhiteBalanceOffset) -> command::white_balance::SetAtwColorTemperatureOffset where HasAtwOffsets
                = [command::white_balance::SetAtwColorTemperatureOffset::new(offset)];
            /// Reads the ATW color-temperature offset as a signed offset level in `-99..=99`, with zero neutral.
            /// The offset is dimensionless rather than a Kelvin value (R7 pp. 15 and 21). The camera applies this
            /// offset only when its `Offset White<ATW>` setting permits it.
            inquiry [AtwColorTemperatureOffsetInquiry] atw_color_temperature_offset() -> types::WhiteBalanceOffset where HasAtwOffsets
                = [command::white_balance::AtwColorTemperatureOffsetInquiry];
            /// Raises the ATW tint offset by `1..=255` relative adjustment steps within the documented −99 to +99
            /// offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only
            /// when its `Offset White<ATW>` setting permits it.
            plain [AtwTintOffsetUpBy] atw_tint_offset_up_by(steps: types::WhiteBalanceStep) -> command::white_balance::AtwTintOffsetUpBy where HasAtwOffsets
                = [command::white_balance::AtwTintOffsetUpBy::new(steps)];
            /// Lowers the ATW tint offset by `1..=255` relative adjustment steps within the documented −99 to +99
            /// offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only
            /// when its `Offset White<ATW>` setting permits it.
            plain [AtwTintOffsetDownBy] atw_tint_offset_down_by(steps: types::WhiteBalanceStep) -> command::white_balance::AtwTintOffsetDownBy where HasAtwOffsets
                = [command::white_balance::AtwTintOffsetDownBy::new(steps)];
            /// Sets the ATW tint offset as a signed offset level in `-99..=99`, with zero neutral. The offset is
            /// dimensionless rather than a Kelvin value (R7 p. 15). The camera applies this offset only when its
            /// `Offset White<ATW>` setting permits it.
            plain [SetAtwTintOffset] set_atw_tint_offset(offset: types::WhiteBalanceOffset) -> command::white_balance::SetAtwTintOffset where HasAtwOffsets
                = [command::white_balance::SetAtwTintOffset::new(offset)];
            /// Reads the ATW tint offset as a signed offset level in `-99..=99`, with zero neutral. The offset is
            /// dimensionless rather than a Kelvin value (R7 pp. 15 and 21). The camera applies this offset only
            /// when its `Offset White<ATW>` setting permits it.
            inquiry [AtwTintOffsetInquiry] atw_tint_offset() -> types::WhiteBalanceOffset where HasAtwOffsets
                = [command::white_balance::AtwTintOffsetInquiry];
            /// Selects a white-balance mode from the target profile's supported inventory. Selecting `OnePush` does
            /// not trigger measurement; use `one_push()` to measure the scene.
            ///
            /// `WhiteBalanceMode` is a categorical operating-mode selector. Only values in
            /// the selected profile's `WhiteBalanceCaps::modes` inventory may be sent;
            /// setting a mode is separate from triggering `one_push()`.
            plain [SetMode] set_mode(mode: types::WhiteBalanceMode) -> command::white_balance::SetMode
                = [command::white_balance::SetMode::new(mode)];

            @noun Image {
                accessor: ImageAccessor,
                getter: image,
                dyn_trait: DynImage,
                gate: [domain HasImage],
                doc: "Image-processing, noise-reduction, flip, and freeze commands and inquiries.",
            };
            /// Returns image saturation.
            ///
            /// `SaturationLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::saturation_range` supplies the supported
            /// code domain.
            inquiry [SaturationInquiry] saturation() -> types::SaturationLevel where HasSaturation
                = [command::image::SaturationInquiry];
            /// Sets image saturation.
            ///
            /// `SaturationLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::saturation_range` supplies the supported
            /// code domain.
            plain [SetSaturation] set_saturation(level: types::SaturationLevel) -> command::image::SetSaturation where HasSaturation
                = [command::image::SetSaturation::new(level)];
            /// Returns image hue.
            ///
            /// `HueLevel` is a protocol adjustment code, not an angle in degrees. The
            /// selected profile's `ImageCaps::hue_range` supplies the supported code
            /// domain.
            inquiry [HueInquiry] hue() -> types::HueLevel where HasHue
                = [command::image::HueInquiry];
            /// Sets image hue.
            ///
            /// `HueLevel` is a protocol adjustment code, not an angle in degrees. The
            /// selected profile's `ImageCaps::hue_range` supplies the supported code
            /// domain.
            plain [SetHue] set_hue(level: types::HueLevel) -> command::image::SetHue where HasHue
                = [command::image::SetHue::new(level)];
            /// Returns image luminance.
            ///
            /// `LuminanceLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::luminance_range` supplies the supported code
            /// domain.
            inquiry [LuminanceInquiry] luminance() -> types::LuminanceLevel where HasLuminance
                = [command::image::LuminanceInquiry];
            /// Sets image luminance.
            ///
            /// `LuminanceLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::luminance_range` supplies the supported code
            /// domain.
            plain [SetLuminance] set_luminance(level: types::LuminanceLevel) -> command::image::SetLuminance where HasLuminance
                = [command::image::SetLuminance::new(level)];
            /// Returns image contrast.
            ///
            /// `ContrastLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::contrast_range` supplies the supported code
            /// domain.
            inquiry [ContrastInquiry] contrast() -> types::ContrastLevel where HasContrast
                = [command::image::ContrastInquiry];
            /// Sets image contrast.
            ///
            /// `ContrastLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::contrast_range` supplies the supported code
            /// domain.
            plain [SetContrast] set_contrast(level: types::ContrastLevel) -> command::image::SetContrast where HasContrast
                = [command::image::SetContrast::new(level)];
            /// Returns the gamma curve.
            ///
            /// `GammaLevel` is a protocol adjustment code, not a percentage. The selected
            /// profile's `ImageCaps::gamma_range` supplies the supported code domain.
            inquiry [GammaInquiry] gamma() -> types::GammaLevel where HasGamma
                = [command::image::GammaInquiry];
            /// Sets the gamma curve.
            ///
            /// `GammaLevel` is a protocol adjustment code, not a percentage. The selected
            /// profile's `ImageCaps::gamma_range` supplies the supported code domain.
            plain [SetGamma] set_gamma(level: types::GammaLevel) -> command::image::SetGamma where HasGamma
                = [command::image::SetGamma::new(level)];
            /// Returns the sharpness mode.
            ///
            /// `SharpnessMode` selects `Auto` or `Manual`. This is a categorical selector;
            /// the selected profile's `SharpnessMode` grant determines availability.
            inquiry [SharpnessModeInquiry] sharpness_mode() -> types::SharpnessMode where HasSharpnessMode
                = [command::image::SharpnessModeInquiry];
            /// Returns the sharpness level.
            ///
            /// `SharpnessLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::sharpness_range` supplies the supported code
            /// domain; a step requests one camera adjustment rather than a universal
            /// physical delta.
            inquiry [SharpnessInquiry] sharpness() -> types::SharpnessLevel where HasSharpness
                = [command::image::SharpnessInquiry];
            /// Sets the sharpness mode.
            ///
            /// `SharpnessMode` selects `Auto` or `Manual`. This is a categorical selector;
            /// the selected profile's `SharpnessMode` grant determines availability.
            plain [SetSharpnessMode] set_sharpness_mode(mode: types::SharpnessMode) -> command::image::SetSharpnessMode where HasSharpnessMode
                = [command::image::SetSharpnessMode::new(mode)];
            /// Resets sharpness.
            ///
            /// `SharpnessLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::sharpness_range` supplies the supported code
            /// domain; a step requests one camera adjustment rather than a universal
            /// physical delta.
            plain [SharpnessReset] sharpness_reset() -> command::image::SharpnessReset where HasSharpness
                = [command::image::SharpnessReset];
            /// Increases sharpness by one step.
            ///
            /// `SharpnessLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::sharpness_range` supplies the supported code
            /// domain; a step requests one camera adjustment rather than a universal
            /// physical delta.
            plain [SharpnessUp] sharpness_up() -> command::image::SharpnessUp where HasSharpness
                = [command::image::SharpnessUp];
            /// Decreases sharpness by one step.
            ///
            /// `SharpnessLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::sharpness_range` supplies the supported code
            /// domain; a step requests one camera adjustment rather than a universal
            /// physical delta.
            plain [SharpnessDown] sharpness_down() -> command::image::SharpnessDown where HasSharpness
                = [command::image::SharpnessDown];
            /// Sets a direct sharpness level.
            ///
            /// `SharpnessLevel` is a protocol adjustment code, not a percentage. The
            /// selected profile's `ImageCaps::sharpness_range` supplies the supported code
            /// domain; a step requests one camera adjustment rather than a universal
            /// physical delta.
            plain [SetSharpness] set_sharpness(level: types::SharpnessLevel) -> command::image::SetSharpness where HasSharpness
                = [command::image::SetSharpness::new(level)];
            /// Returns 2D noise reduction level.
            ///
            /// `NoiseReduction2D` is `Off` or `Level(NoiseReduction2DLevel)` with levels
            /// `1..=5`. These are filter-strength categories, not percentages; the
            /// selected profile's `NoiseReduction2D` family grant declares availability.
            inquiry [NoiseReduction2dInquiry] noise_reduction_2d() -> types::NoiseReduction2D where HasNoiseReduction2D
                = [command::image::NoiseReduction2dInquiry];
            /// Returns the 2D noise reduction mode.
            ///
            /// `NoiseReduction2DMode` selects `Auto` or `Manual`, separately from the
            /// filter-strength value. The selected profile's `NoiseReduction2DMode` grant
            /// declares this categorical mode register.
            inquiry [NoiseReduction2dModeInquiry] noise_reduction_2d_mode() -> types::NoiseReduction2DMode where HasNoiseReduction2DMode
                = [command::image::NoiseReduction2dModeInquiry];
            /// Returns 3D noise reduction level.
            ///
            /// `NoiseReduction3D` is `Off` or `Level(NoiseReduction3DLevel)` with levels
            /// `1..=8`. These are filter-strength categories, not percentages; the
            /// selected profile's `NoiseReduction3D` family grant declares availability.
            inquiry [NoiseReduction3dInquiry] noise_reduction_3d() -> types::NoiseReduction3D where HasNoiseReduction3D
                = [command::image::NoiseReduction3dInquiry];
            /// Sets the 2D noise-reduction mode.
            ///
            /// `NoiseReduction2DMode` selects `Auto` or `Manual`, separately from the
            /// filter-strength value. The selected profile's `NoiseReduction2DMode` grant
            /// declares this categorical mode register.
            plain [SetNoiseReduction2dMode] set_noise_reduction_2d_mode(mode: types::NoiseReduction2DMode) -> command::image::SetNoiseReduction2dMode where HasNoiseReduction2DMode
                = [command::image::SetNoiseReduction2dMode::new(mode)];
            /// Sets the 2D noise-reduction level.
            ///
            /// `NoiseReduction2D` is `Off` or `Level(NoiseReduction2DLevel)` with levels
            /// `1..=5`. These are filter-strength categories, not percentages; the
            /// selected profile's `NoiseReduction2D` family grant declares availability.
            plain [SetNoiseReduction2d] set_noise_reduction_2d(setting: types::NoiseReduction2D) -> command::image::SetNoiseReduction2d where HasNoiseReduction2D
                = [command::image::SetNoiseReduction2d::new(setting)];
            /// Sets the 3D noise-reduction level.
            ///
            /// `NoiseReduction3D` is `Off` or `Level(NoiseReduction3DLevel)` with levels
            /// `1..=8`. These are filter-strength categories, not percentages; the
            /// selected profile's `NoiseReduction3D` family grant declares availability.
            plain [SetNoiseReduction3d] set_noise_reduction_3d(setting: types::NoiseReduction3D) -> command::image::SetNoiseReduction3d where HasNoiseReduction3D
                = [command::image::SetNoiseReduction3d::new(setting)];
            /// Enables or disables vertical image flipping using the profile's documented command.
            ///
            /// `true` selects vertical image inversion and `false` disables it. The
            /// profile's `ImageCaps::flip` fact and image-flip grant declare availability;
            /// horizontal mirroring is a separate property.
            plain [SetFlip] set_flip(enabled: bool) -> command::image::SetFlip where HasImageFlip
                = [command::image::SetFlip::new(enabled)];
            /// Enables or disables horizontal image mirroring.
            ///
            /// `true` selects horizontal mirroring and `false` disables it. Availability
            /// comes from the profile's `ImageCaps::mirror` fact and mirror grant; the
            /// combined orientation is read with `flip_mode()`.
            plain [SetMirror] set_mirror(enabled: bool) -> command::image::SetMirror where HasImageMirror
                = [command::image::SetMirror::new(enabled)];
            /// Sets the combined image-flip mode.
            ///
            /// `ImageFlipMode` selects `Off`, `Horizontal`, `Vertical` or `Both`. These
            /// are image orientations, not angles; the selected profile's combined-flip
            /// grant and `ImageCaps::flip`/`mirror` facts supply support.
            plain [SetFlipMode] set_flip_mode(mode: types::ImageFlipMode) -> command::image::SetFlipMode where HasCombinedImageFlip
                = [command::image::SetFlipMode::new(mode)];
            /// Freezes image output when enabled and resumes live output when disabled.
            ///
            /// `true` freezes the camera image and `false` resumes it. This is a boolean
            /// image control provided by the selected profile's `ImageFreeze` grant.
            plain [SetFreeze] set_freeze(enabled: bool) -> command::image::SetFreeze where HasImageFreeze
                = [command::image::SetFreeze::new(enabled)];
            /// Returns vertical image-flip state (`09 04 66`); horizontal is false.
            ///
            /// `true` selects vertical image inversion and `false` disables it. The
            /// profile's `ImageCaps::flip` fact and image-flip grant declare availability;
            /// horizontal mirroring is a separate property.
            inquiry [FlipInquiry] flip() -> bool where HasImageFlip
                = [command::image::FlipInquiry];
            /// Returns the combined image-flip mode.
            ///
            /// `ImageFlipMode` selects `Off`, `Horizontal`, `Vertical` or `Both`. These
            /// are image orientations, not angles; the selected profile's combined-flip
            /// grant and `ImageCaps::flip`/`mirror` facts supply support.
            inquiry [FlipModeInquiry] flip_mode() -> types::ImageFlipMode where HasCombinedImageFlip
                = [command::image::FlipModeInquiry];
            /// Returns picture-effect mode.
            ///
            /// `PictureEffectMode` is an effect selector rather than an intensity. It
            /// preserves unknown reply codes as well as the named effects; the selected
            /// profile's `ImageCaps::picture_effect` fact and picture-effect grant declare
            /// availability.
            inquiry [PictureEffectInquiry] picture_effect() -> types::PictureEffectMode where HasPictureEffect
                = [command::image::PictureEffectInquiry];
            /// Sets picture-effect mode.
            ///
            /// `PictureEffectMode` is an effect selector rather than an intensity. It
            /// preserves unknown reply codes as well as the named effects; the selected
            /// profile's `ImageCaps::picture_effect` fact and picture-effect grant declare
            /// availability.
            plain [SetPictureEffect] set_picture_effect(mode: types::PictureEffectMode) -> command::image::SetPictureEffect where HasPictureEffect
                = [command::image::SetPictureEffect::new(mode)];
            /// Raises the master black level by `1..=255` relative adjustment steps. Direct levels span −99.0 to
            /// +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [MasterBlackUpBy] master_black_up_by(steps: types::BlackStep) -> command::image::MasterBlackUpBy where HasBlackLevels
                = [command::image::MasterBlackUpBy::new(steps)];
            /// Lowers the master black level by `1..=255` relative adjustment steps. Direct levels span −99.0 to
            /// +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [MasterBlackDownBy] master_black_down_by(steps: types::BlackStep) -> command::image::MasterBlackDownBy where HasBlackLevels
                = [command::image::MasterBlackDownBy::new(steps)];
            /// Sets the master black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed units,
            /// with zero neutral (R7 p. 16).
            plain [SetMasterBlack] set_master_black(value: types::BlackLevel) -> command::image::SetMasterBlack where HasBlackLevels
                = [command::image::SetMasterBlack::new(value)];
            /// Reads the master black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed
            /// units, with zero neutral (R7 pp. 16 and 21).
            inquiry [MasterBlackInquiry] master_black() -> types::BlackLevel where HasBlackLevels
                = [command::image::MasterBlackInquiry];
            /// Raises the red-channel black level by `1..=255` relative adjustment steps. Direct levels span −99.0
            /// to +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [RedBlackUpBy] red_black_up_by(steps: types::BlackStep) -> command::image::RedBlackUpBy where HasBlackLevels
                = [command::image::RedBlackUpBy::new(steps)];
            /// Lowers the red-channel black level by `1..=255` relative adjustment steps. Direct levels span −99.0
            /// to +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [RedBlackDownBy] red_black_down_by(steps: types::BlackStep) -> command::image::RedBlackDownBy where HasBlackLevels
                = [command::image::RedBlackDownBy::new(steps)];
            /// Sets the red-channel black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed
            /// units, with zero neutral (R7 p. 16).
            plain [SetRedBlack] set_red_black(value: types::BlackLevel) -> command::image::SetRedBlack where HasBlackLevels
                = [command::image::SetRedBlack::new(value)];
            /// Reads the red-channel black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed
            /// units, with zero neutral (R7 pp. 16 and 21).
            inquiry [RedBlackInquiry] red_black() -> types::BlackLevel where HasBlackLevels
                = [command::image::RedBlackInquiry];
            /// Raises the blue-channel black level by `1..=255` relative adjustment steps. Direct levels span −99.0
            /// to +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [BlueBlackUpBy] blue_black_up_by(steps: types::BlackStep) -> command::image::BlueBlackUpBy where HasBlackLevels
                = [command::image::BlueBlackUpBy::new(steps)];
            /// Lowers the blue-channel black level by `1..=255` relative adjustment steps. Direct levels span −99.0
            /// to +99.0 displayed units; the count is not an absolute black level (R7 p. 16).
            plain [BlueBlackDownBy] blue_black_down_by(steps: types::BlackStep) -> command::image::BlueBlackDownBy where HasBlackLevels
                = [command::image::BlueBlackDownBy::new(steps)];
            /// Sets the blue-channel black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed
            /// units, with zero neutral (R7 p. 16).
            plain [SetBlueBlack] set_blue_black(value: types::BlackLevel) -> command::image::SetBlueBlack where HasBlackLevels
                = [command::image::SetBlueBlack::new(value)];
            /// Reads the blue-channel black level as `BlackLevel`: `-990..=990` represents −99.0 to +99.0 displayed
            /// units, with zero neutral (R7 pp. 16 and 21).
            inquiry [BlueBlackInquiry] blue_black() -> types::BlackLevel where HasBlackLevels
                = [command::image::BlueBlackInquiry];
            /// Enables or disables detail processing; adjust its level with the separate sharpness control (R7 p.
            /// 16).
            plain [SetDetail] set_detail(enabled: bool) -> command::image::SetDetail where HasDetail
                = [command::image::SetDetail::new(enabled)];
            /// Reads whether detail processing is enabled (R7 pp. 16 and 21).
            inquiry [DetailInquiry] detail() -> bool where HasDetail
                = [command::image::DetailInquiry];
            /// Enables or disables knee correction; point, slope and automatic/manual mode remain separate settings
            /// (R7 p. 16).
            plain [SetKnee] set_knee(enabled: bool) -> command::image::SetKnee where HasKnee
                = [command::image::SetKnee::new(enabled)];
            /// Reads whether knee correction is enabled (R7 pp. 16 and 22).
            inquiry [KneeInquiry] knee() -> bool where HasKnee
                = [command::image::KneeInquiry];
            /// Selects automatic or manual knee correction. `Auto` lets the camera control the knee; `Manual` uses
            /// the configured point and slope (R7 p. 16).
            plain [SetKneeMode] set_knee_mode(mode: types::KneeMode) -> command::image::SetKneeMode where HasKnee
                = [command::image::SetKneeMode::new(mode)];
            /// Reads automatic or manual knee correction. `Auto` lets the camera control the knee; `Manual` uses
            /// the configured point and slope (R7 pp. 16 and 22).
            inquiry [KneeModeInquiry] knee_mode() -> types::KneeMode where HasKnee
                = [command::image::KneeModeInquiry];
            /// Sets the signed knee-slope level in `-99..=99`, with zero neutral. This is a camera adjustment level
            /// rather than an angle or percentage (R7 p. 16).
            plain [SetKneeSlope] set_knee_slope(value: types::KneeSlope) -> command::image::SetKneeSlope where HasKnee
                = [command::image::SetKneeSlope::new(value)];
            /// Reads the signed knee-slope level in `-99..=99`, with zero neutral. This is a camera adjustment
            /// level rather than an angle or percentage (R7 pp. 16 and 22).
            inquiry [KneeSlopeInquiry] knee_slope() -> types::KneeSlope where HasKnee
                = [command::image::KneeSlopeInquiry];
            /// Sets the knee point in percent over `75..=109%` (R7 p. 16).
            plain [SetKneePoint] set_knee_point(value: types::KneePoint) -> command::image::SetKneePoint where HasKnee
                = [command::image::SetKneePoint::new(value)];
            /// Reads the knee point in percent over `75..=109%` (R7 pp. 16 and 22).
            inquiry [KneePointInquiry] knee_point() -> types::KneePoint where HasKnee
                = [command::image::KneePointInquiry];
            /// Reads the current camera permission for the selected knee parameter (`Setting`, `Mode`, `Point` or
            /// `Slope`). `Enabled` permits configuration, `Disabled` is unavailable, and `DisplayOnly` means the
            /// read value is valid but cannot be changed (R7 pp. 16 and 22). This is current camera-state
            /// permission, separate from the profile's capability grant.
            inquiry [KneePermissionInquiry] knee_permission(parameter: types::KneeParameter) -> types::KneePermission where HasKnee
                = [command::image::KneePermissionInquiry::new(parameter)];

            @noun Tally {
                accessor: TallyAccessor,
                getter: tally,
                dyn_trait: DynTally,
                gate: [typed HasTally],
                doc: "Tally-light commands and inquiries.",
            };
            /// Enables or disables the red tally lamp using the profile's documented control.
            ///
            /// `true` means the selected tally lamp is on and `false` means off. The
            /// selected profile's `Tally` grant supplies its documented lamp-control and
            /// reply contract; no brightness value is encoded by this boolean.
            plain [SetRed] set_red(enabled: bool) -> command::tally::SetRed
                = [command::tally::SetRed::new(enabled)];
            /// Selects low or high tally brightness on profiles that document that adjustment.
            ///
            /// `TallyBrightness` selects `Low` or `High`, not a numeric brightness
            /// percentage. The selected profile's `TallyBrightness` grant declares this
            /// two-value adjustment.
            plain [SetBrightness] set_brightness(brightness: types::TallyBrightness) -> command::tally::SetBrightness where HasTallyBrightness
                = [command::tally::SetBrightness::new(brightness)];
            /// Enables or disables the green tally lamp using the profile's documented control.
            ///
            /// `true` means the selected tally lamp is on and `false` means off. The
            /// selected profile's `Tally` grant supplies its documented lamp-control and
            /// reply contract; no brightness value is encoded by this boolean.
            plain [SetGreen] set_green(enabled: bool) -> command::tally::SetGreen
                = [command::tally::SetGreen::new(enabled)];
            /// Selects off, steady-on, or flashing tally indication using the profile's documented lamp-mode
            /// command.
            ///
            /// `TallyMode` selects `Off`, steady `On` or `Flash`; it is not a boolean
            /// lamp-state value. The selected profile's `TallyMode` grant supplies the
            /// supported mode control.
            plain [SetMode] set_mode(mode: types::TallyMode) -> command::tally::SetMode where HasTallyMode
                = [command::tally::SetMode::new(mode)];
            /// Returns red tally state.
            ///
            /// `true` means the selected tally lamp is on and `false` means off. The
            /// selected profile's `Tally` grant supplies its documented lamp-control and
            /// reply contract; no brightness value is encoded by this boolean.
            inquiry [RedInquiry] red() -> bool
                = [command::tally::RedInquiry];
            /// Returns green tally state.
            ///
            /// `true` means the selected tally lamp is on and `false` means off. The
            /// selected profile's `Tally` grant supplies its documented lamp-control and
            /// reply contract; no brightness value is encoded by this boolean.
            inquiry [GreenInquiry] green() -> bool
                = [command::tally::GreenInquiry];

            @noun NdFilter {
                accessor: NdFilterAccessor,
                getter: nd_filter,
                dyn_trait: DynNdFilter,
                gate: [typed HasNdFilter],
                doc: "Neutral-density filter commands and inquiries.",
            };
            /// Returns the Sony FR7 variable ND-filter density code (`00..=14`).
            ///
            /// `NdFilterDensity` codes `0x00..=0x14` represent `2..=7` photographic stops
            /// in quarter-stop increments (`from_stops`/`to_stops`). This is the
            /// documented variable-ND domain of the profile's `NdFilter` grant (R7 p. 14).
            inquiry [DensityInquiry] density() -> types::NdFilterDensity
                = [command::nd_filter::DensityInquiry];
            /// Returns the current ND-filter preset.
            ///
            /// `NdFilterPreset` is the categorical preset code `0..=3`, not an
            /// optical-density value. Its documented reply is supplied by the profile's
            /// `NdFilter` grant.
            inquiry [PresetInquiry] preset() -> types::NdFilterPreset
                = [command::nd_filter::PresetInquiry];
            /// Selects preset or variable ND-filter mode.
            ///
            /// `NdFilterMode` selects the categorical `Preset` or `Variable` operating
            /// mode. The selected profile's `NdFilter` grant declares support; actual
            /// variable density uses a separate value in photographic stops.
            plain [SetMode] set_mode(mode: types::NdFilterMode) -> command::nd_filter::SetMode
                = [command::nd_filter::SetMode::new(mode)];
            /// Sets a direct variable ND-filter value.  Build the value with
            /// [`crate::types::NdFilterDensity::new`](crate::types::NdFilterDensity::new) from a raw density or
            /// [`crate::types::NdFilterDensity::from_stops`](crate::types::NdFilterDensity::from_stops) from photographic stops.
            ///
            /// `NdFilterDensity` codes `0x00..=0x14` represent `2..=7` photographic stops
            /// in quarter-stop increments (`from_stops`/`to_stops`). This is the
            /// documented variable-ND domain of the profile's `NdFilter` grant (R7 p. 14).
            targeted [SetDensity] set_density(density: types::NdFilterDensity) -> command::nd_filter::SetDensity
                = [command::nd_filter::SetDensity::new(density)];
            /// Increases ND-filter density by one step.
            ///
            /// `NdFilterDensity` codes `0x00..=0x14` represent `2..=7` photographic stops
            /// in quarter-stop increments (`from_stops`/`to_stops`). This is the
            /// documented variable-ND domain of the profile's `NdFilter` grant (R7 p. 14).
            targeted [DensityUp] density_up() -> command::nd_filter::DensityUp
                = [command::nd_filter::DensityUp];
            /// Decreases ND-filter density by one step.
            ///
            /// `NdFilterDensity` codes `0x00..=0x14` represent `2..=7` photographic stops
            /// in quarter-stop increments (`from_stops`/`to_stops`). This is the
            /// documented variable-ND domain of the profile's `NdFilter` grant (R7 p. 14).
            targeted [DensityDown] density_down() -> command::nd_filter::DensityDown
                = [command::nd_filter::DensityDown];
            /// Enables or disables automatic variable-ND density control (R7 p. 14).
            ///
            /// `true` enables automatic variable-ND adjustment and `false` disables it.
            /// This boolean has no optical-density argument; availability comes from the
            /// profile's `NdFilter` grant (R7 p. 14).
            plain [SetAuto] set_auto(enabled: bool) -> command::nd_filter::SetAuto
                = [command::nd_filter::SetAuto::new(enabled)];

            @noun MotionSync {
                accessor: MotionSyncAccessor,
                getter: motion_sync,
                dyn_trait: DynMotionSync,
                gate: [typed HasMotionSync],
                doc: "Motion-sync commands and inquiries.",
            };
            /// Reads whether synchronized preset movement is enabled.
            ///
            /// `true` enables synchronized preset-recall movement and `false` disables it.
            /// This boolean is supported through the selected profile's `MotionSync`
            /// grant; it does not itself select a speed or start movement.
            inquiry [EnabledInquiry] enabled() -> bool
                = [command::motion_sync::EnabledInquiry];
            /// Enables or disables synchronized preset movement.
            ///
            /// `true` enables synchronized preset-recall movement and `false` disables it.
            /// This boolean is supported through the selected profile's `MotionSync`
            /// grant; it does not itself select a speed or start movement.
            plain [SetEnabled] set_enabled(enabled: bool) -> command::motion_sync::SetEnabled
                = [command::motion_sync::SetEnabled::new(enabled)];
            /// Sets the motion-sync speed.  [`crate::types::MotionSyncSpeed`](crate::types::MotionSyncSpeed) owns the `1..=24`
            /// range, and `MotionSyncSpeed::from(MotionSyncPreset)` maps the slow/normal/fast presets.
            ///
            /// `MotionSyncSpeed` is a unitless protocol code with checked union `1..=24`.
            /// The selected profile's `PanTiltCaps::motion_sync_speed_range` supplies
            /// supported bounds; the setting does not specify degrees per second.
            plain [SetSpeed] set_speed(speed: types::MotionSyncSpeed) -> command::motion_sync::SetSpeed
                = [command::motion_sync::SetSpeed::new(speed)];

            @noun Menu {
                accessor: MenuAccessor,
                getter: menu,
                dyn_trait: DynMenu,
                gate: [domain HasMenu],
                doc: "On-screen menu display and navigation commands and the menu-status inquiry.",
            };
            /// Returns whether the on-screen menu is open.
            ///
            /// `true` means the on-screen menu is visible and `false` means hidden.
            /// Menu-domain presence and the selected profile's display-control or
            /// display-inquiry grant provide the applicable boolean operation.
            inquiry [VisibleInquiry] visible() -> bool where HasMenuDisplayInquiry
                = [command::menu::VisibleInquiry];
            /// Displays or hides the on-screen menu.
            ///
            /// `true` means the on-screen menu is visible and `false` means hidden.
            /// Menu-domain presence and the selected profile's display-control or
            /// display-inquiry grant provide the applicable boolean operation.
            plain [SetVisible] set_visible(visible: bool) -> command::menu::SetVisible where HasMenuDisplay
                = [command::menu::SetVisible::new(visible)];
            /// Moves the menu cursor with a pan/tilt drive.  # Safety  If the menu is closed, this starts
            /// continuous physical pan/tilt movement. Send an explicit pan/tilt STOP or call `stop_all()` to
            /// stop it. The applied handle confirms acceptance, not that physical movement has stopped.
            ///
            /// `MenuDirection` selects `Up`, `Down`, `Left` or `Right`, not a pixel
            /// distance. The menu-navigation grant supplies its fixed protocol drive
            /// settings; when the menu is closed this command can move the physical
            /// camera, so an explicit stop remains necessary.
            applied [Navigate] navigate(direction: types::MenuDirection) -> command::menu::Navigate
                = [command::menu::Navigate::new(direction)];
            /// Selects the current menu item.
            plain [Enter] enter() -> command::menu::Enter where HasMenuEnterReturn
                = [command::menu::Enter];
            /// Cancels or returns from the current menu item.
            plain [Back] back() -> command::menu::Back where HasMenuEnterReturn
                = [command::menu::Back];
            /// Sends a typed Sony FR7 direct-menu button control.
            ///
            /// `DirectMenuTarget` selects one of the documented ND, iris, ISO/gain,
            /// shutter, AE-level/mode or exit controls; `MenuButton` selects `Press` or
            /// `Release`. These are categorical button events from the profile's
            /// `DirectMenu` grant, not numeric camera settings (R7 p. 20).
            plain [Direct] direct(target: types::DirectMenuTarget, button: types::MenuButton) -> command::menu::Direct where HasDirectMenu
                = [command::menu::Direct::new(target, button)];
            /// Toggles the on-screen menu open or closed.  This is the vendor open/close direct control, so it
            /// needs no prior [`Self::visible`] round trip to decide which way to move.
            ///
            /// `true` means the on-screen menu is visible and `false` means hidden.
            /// Menu-domain presence and the selected profile's display-control or
            /// display-inquiry grant provide the applicable boolean operation.
            plain [ToggleVisible] toggle_visible() -> command::menu::ToggleVisible where HasDirectMenu
                = [command::menu::ToggleVisible];

            @noun Streaming {
                accessor: StreamingAccessor,
                getter: streaming,
                dyn_trait: DynStreaming,
                gate: [always],
                doc: "Streaming quality, multicast output, and USB-audio controls and inquiries.",
            };
            /// Returns USB-audio state.
            ///
            /// `true` enables USB audio output and `false` disables it. The selected
            /// profile's `UsbAudio` grant supplies the sourced boolean control and reply
            /// convention.
            inquiry [UsbAudioInquiry] usb_audio() -> bool where HasUsbAudio
                = [command::streaming::UsbAudioInquiry];
            /// Enables or disables multicast video streaming.
            ///
            /// `true` enables multicast video output and `false` disables it. This boolean
            /// uses the selected profile's `MulticastStreaming` grant; it does not choose
            /// an address, port or bitrate.
            plain [SetMulticast] set_multicast(enabled: bool) -> command::streaming::SetMulticast where HasMulticastStreaming
                = [command::streaming::SetMulticast::new(enabled)];
            /// Sets NDI streaming quality.
            ///
            /// `NdiQuality` selects `High`, `Medium`, `Low` or `Off`; these are quality
            /// categories rather than numeric bitrates. Availability comes from the
            /// selected profile's `NdiQuality` grant.
            plain [SetNdiQuality] set_ndi_quality(quality: types::NdiQuality) -> command::streaming::SetNdiQuality where HasNdiQuality
                = [command::streaming::SetNdiQuality::new(quality)];
            /// Enables or disables USB audio output.
            ///
            /// `true` enables USB audio output and `false` disables it. The selected
            /// profile's `UsbAudio` grant supplies the sourced boolean control and reply
            /// convention.
            plain [SetUsbAudio] set_usb_audio(enabled: bool) -> command::streaming::SetUsbAudio where HasUsbAudio
                = [command::streaming::SetUsbAudio::new(enabled)];

            @exceptions;
            broadcast [AddressSet] address_set -> command::system_wire::AddressSetCommand;
            broadcast [InterfaceClear] interface_clear -> command::system_wire::InterfaceClearCommand;
            internal [CommandCancel] cancel_command -> command::system_wire::CommandCancelCommand;
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
/// — the blocking and async `MotionAccessor` —
/// expands these rows, so all of them expose the same names, arities and
/// contracts.
#[cfg(any(feature = "blocking", feature = "async", test))]
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
            fn stop_all(&self) -> crate::operation::HaltReport => stop_all_motion();

            /// Reports whether two protocol position samples, separated by at
            /// least [`crate::types::MotionQuery::window`](types::MotionQuery::window),
            /// indicate movement on the selected axes.
            ///
            /// [`crate::types::MotionQuery::default()`](types::MotionQuery::default)
            /// samples [`crate::types::AffectedAxes::MOVEMENT`](crate::types::AffectedAxes::MOVEMENT)
            /// with the default tolerance and observation window. `false` means
            /// no movement was detected over the window, not that the camera is
            /// physically at rest. A zero window fails with
            /// [`Error::InvalidParameter`](crate::Error::InvalidParameter)
            /// before any inquiry, and a window that cannot elapse within the
            /// observation deadline fails with
            /// [`Error::Timeout`](crate::Error::Timeout) rather than reporting
            /// no movement.
            fn is_moving(&self, query: types::MotionQuery) -> bool => is_moving(query);

            /// Waits until the selected axes meet the protocol idle condition:
            /// two consecutive position samples, taken every
            /// [`crate::types::IdleWait::interval`](types::IdleWait::interval),
            /// agree within tolerance before
            /// [`crate::types::IdleWait::timeout`](types::IdleWait::timeout)
            /// elapses, which otherwise fails with
            /// [`Error::Timeout`](crate::Error::Timeout).
            fn wait_until_idle(&self, wait: types::IdleWait) -> () => wait_until_idle(wait);
        }
    };
}

#[cfg(any(feature = "blocking", feature = "async", test))]
pub(crate) use motion_table;

/// Re-exports one name per noun header from a facade module.
///
/// The noun table names every accessor once in its headers;
/// the blocking facade re-exports them through this consumer instead of
/// restating the list. Invoke it as
/// `noun_table!(reexport_nouns, [accessor], [blocking_nouns])`, and use
/// `motion_table!(reexport_nouns, [accessor], [blocking_nouns])` for motion.
#[cfg(feature = "blocking")]
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

#[cfg(feature = "blocking")]
pub(crate) use reexport_nouns;
