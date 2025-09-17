//! VISCA command definitions and traits.
//!
//! This module provides all command types for controlling VISCA cameras,
//! organized by functionality.

// Command modules
pub mod color;
pub mod exposure;
pub mod flip;
pub mod focus;
pub mod gain;
pub mod image;
pub mod inquiry;
pub(crate) mod inquiry_structs; // Internal module for macro-generated inquiry commands
pub mod inquiry_types;
pub mod menu;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod resolution;
pub mod response;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod typed;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Command encoding module
pub mod bytes;

// New unified ViscaCommand trait
pub mod encode;

// Re-export command types
pub use self::{
    color::*,
    encode::{CommandKind, ViscaCommand},
    exposure::*,
    // flip::*,
    focus::*,
    image::*,
    // inquiry::*,
    inquiry_types::{FlipState, IrisControl, NightDayMode, TallyStatus, Version},
    menu::*,
    // motion_sync::*,
    nd_filter::*,
    pan_tilt::*,
    power::*,
    preset::*,
    response::{InquiryKind, Response},
    system::{MotionSyncMode, MotionSyncPreset},
    // tally::*,
    variable_speed::*,
    white_balance::*,
    // zoom::*,
};

/// Response data from VISCA inquiry commands.
///
/// Each variant represents a different type of inquiry response with its associated data.
/// These are returned wrapped in `Response::Inquiry(...)`.
#[derive(Debug, Copy, Clone)]
pub enum InquiryData {
    /// Power status inquiry response.
    Power {
        /// Whether the camera is powered on.
        on: bool,
    },
    /// Pan/Tilt position inquiry response.
    PanTiltPosition {
        /// Current pan position.
        pan: i16,
        /// Current tilt position.
        tilt: i16,
    },

    /// Luminance level inquiry response.
    Luminance(u8),
    /// Contrast level inquiry response.
    Contrast(u8),
    /// Sharpness value inquiry response.
    Sharpness {
        /// Current sharpness value.
        value: u8,
    },
    /// Sharpness mode inquiry response.
    SharpnessMode {
        /// Current sharpness mode.
        mode: SharpnessMode,
    },
    /// Color saturation inquiry response.
    Saturation {
        /// Saturation level (0x0=60% to 0xE=200%).
        level: u8,
    },
    /// Color hue inquiry response.
    Hue {
        /// Hue value (0x0=0 to 0xE=14).
        hue: u8,
    },

    /// Current zoom position inquiry response.
    ZoomPosition {
        /// Zoom position value.
        position: u16,
    },
    /// Current focus position inquiry response.
    FocusPosition {
        /// Focus position value.
        position: u16,
    },
    /// Focus zone inquiry response.
    FocusZone {
        /// Current focus zone setting.
        zone: FocusZone,
    },
    /// Auto-focus sensitivity inquiry response.
    AutoFocusSensitivity {
        /// Current auto-focus sensitivity setting.
        sensitivity: AutoFocusSensitivity,
    },
    /// Focus near limit inquiry response.
    FocusNearLimit {
        /// Near limit position value.
        position: u16,
    },

    /// Exposure mode inquiry response.
    ExposureMode {
        /// Current exposure mode (Auto, Manual, Shutter, Iris, or Bright).
        mode: ExposureMode,
    },
    /// Exposure compensation inquiry response.
    ExposureCompensation {
        /// Exposure compensation value (-7 to +7).
        value: i8,
    },
    /// Exposure compensation mode inquiry response.
    ExposureCompensationMode {
        /// Whether exposure compensation is enabled.
        on: bool,
    },
    /// Gain level inquiry response.
    GainLevel {
        /// Gain level value (0x00=0 to 0x07=7).
        gain: u8,
    },
    /// Gain limit inquiry response.
    GainLimit {
        /// Maximum gain limit (0x0=0 to 0xF=15).
        limit: u8,
    },
    /// Iris position inquiry response.
    Iris {
        /// Iris position (0x0=Close to 0xC=F1.8).
        position: u8,
    },
    /// Shutter speed inquiry response.
    Shutter {
        /// Shutter position (0x01=1/30 to 0x11=1/10000).
        position: u16,
    },
    /// Brightness inquiry response.
    Brightness {
        /// Brightness position (0x00=0 to 0x11=17).
        position: u16,
    },
    /// Backlight compensation inquiry response.
    Backlight {
        /// Whether backlight compensation is enabled.
        status: bool,
    },

    /// White balance mode inquiry response.
    WhiteBalanceMode {
        /// Current white balance mode.
        mode: WhiteBalanceMode,
    },
    /// Color temperature inquiry response.
    ColorTemperature {
        /// Color temperature in Kelvin.
        temperature: u16,
    },
    /// Red channel tuning inquiry response.
    RedChannel {
        /// Red channel adjustment value (-10 to +10).
        gain: i8,
    },
    /// Blue channel tuning inquiry response.
    BlueChannel {
        /// Blue channel adjustment value (-10 to +10).
        gain: i8,
    },

    /// Image flip inquiry response.
    FlipState {
        /// Whether horizontal flip is enabled.
        horizontal: bool,
        /// Whether vertical flip is enabled.
        vertical: bool,
    },
    /// Black and white mode inquiry response.
    BlackWhite {
        /// Whether black and white mode is enabled.
        on: bool,
    },
    /// 2D noise reduction inquiry response.
    NoiseReduction2D {
        /// 2D noise reduction level.
        level: u8,
    },
    /// 3D noise reduction inquiry response.
    NoiseReduction3D {
        /// 3D noise reduction level.
        level: u8,
    },
    /// Dynamic range control inquiry response.
    DynamicRange {
        /// Dynamic range level (0x0=0 to 0x8=8).
        level: u8,
    },
    /// Camera version information inquiry response.
    Version {
        /// Vendor ID.
        vendor: u16,
        /// Model ID.
        model: u16,
        /// ROM version.
        rom_version: u32,
        /// Maximum socket number.
        max_socket: u8,
    },
    /// Red tally light state inquiry response.
    TallyRed {
        /// Whether the red tally light is on.
        on: bool,
    },
    /// Green tally light state inquiry response.
    TallyGreen {
        /// Whether the green tally light is on.
        on: bool,
    },
    /// Focus mode inquiry response.
    FocusMode {
        /// Current focus mode (Auto or Manual).
        mode: FocusMode,
    },
    /// Menu open/close status inquiry response.
    MenuOpenClose {
        /// Whether the camera menu is open.
        is_open: bool,
    },
    /// Auto focus enable/disable status inquiry response.
    AutoFocus {
        /// Whether auto focus is enabled.
        enabled: bool,
    },
    /// Tally light status inquiry response.
    TallyStatus {
        /// Whether the red tally light is on.
        red_on: bool,
        /// Whether the green tally light is on.
        green_on: bool,
    },
    /// Video resolution inquiry response.
    /// The value represents the resolution mode (camera-specific encoding).
    Resolution(u8),
    /// Night/Day mode inquiry response.
    NightDayMode {
        /// Whether the camera is in night mode.
        is_night: bool,
    },
    /// ND filter position inquiry response.
    NdFilter {
        /// Current ND filter position (0x00=Clear, 0x01=1/4, 0x02=1/8, etc.).
        position: u8,
    },
    /// Picture effect mode inquiry response.
    PictureEffect {
        /// Current picture effect (0x00=Off, 0x01=Negative, 0x02=B&W, etc.).
        effect: u8,
    },
    /// Standby mode inquiry response.
    Standby {
        /// Whether the camera is in standby mode.
        in_standby: bool,
    },
    /// Focus range mode inquiry response.
    FocusRange {
        /// Current focus range setting.
        range: FocusRange,
    },
    /// Iris control mode inquiry response.
    IrisControl {
        /// Whether iris is in auto mode.
        auto: bool,
    },
    /// Defog mode inquiry response.
    DefogMode {
        /// Whether defog is enabled.
        enabled: bool,
    },
    /// Defog level inquiry response.
    DefogLevel {
        /// Current defog strength level (0-8).
        level: u8,
    },
    /// Digital Ptz mode inquiry response.
    DigitalPtz {
        /// Whether digital Ptz is enabled.
        enabled: bool,
    },
    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity {
        /// Sensitivity level (Low, Normal, High).
        sensitivity: AutoWhiteBalanceSensitivity,
    },
    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition {
        /// Exposure compensation position value.
        position: u16,
    },
    /// Red channel tuning inquiry response.
    RedTuning {
        /// Red channel tuning level.
        level: u8,
    },
    /// Blue channel tuning inquiry response.
    BlueTuning {
        /// Blue channel tuning level.
        level: u8,
    },
    /// Gamma curve inquiry response.
    Gamma {
        /// Gamma curve setting (0=Standard, 1-4=different gamma curves).
        value: u8,
    },
    /// Auto trace mode inquiry response.
    AutoTrace {
        /// Whether auto trace is enabled.
        enabled: bool,
    },
    /// Focus unlock state inquiry response.
    FocusUnlock {
        /// Whether focus is unlocked.
        unlocked: bool,
    },
    /// Sharpness position inquiry response.
    SharpnessPosition {
        /// Current sharpness position value.
        position: u16,
    },
    /// Noise reduction level inquiry response.
    NoiseReductionLevel(u8),
    /// Broadcast domain inquiry response.
    BroadcastDomain(u8),
    /// Motion sync mode inquiry response.
    MotionSyncMode {
        /// Current motion sync mode setting.
        mode: MotionSyncMode,
    },
    /// Motion sync speed inquiry response.
    MotionSyncPreset {
        /// Current motion sync speed setting.
        speed: MotionSyncPreset,
    },
    /// Noise reduction mode inquiry response.
    NoiseReductionMode {
        /// Current noise reduction mode setting.
        mode: NoiseReductionMode,
    },
    /// Noise reduction speed inquiry response.
    NoiseReductionSpeed {
        /// Current noise reduction speed setting.
        speed: NoiseReductionSpeed,
    },
    /// Black and white mode inquiry response.
    BlackWhiteMode {
        /// Current black and white mode setting.
        mode: BlackWhiteMode,
    },
    /// USB audio state inquiry response.
    UsbAudio {
        /// Whether USB audio is enabled.
        on: bool,
    },
    /// Two tone mode inquiry response.
    TwoToneMode {
        /// Whether two tone mode is enabled.
        on: bool,
    },
    /// ND filter preset inquiry response.
    NdFilterPreset {
        /// Current ND filter preset number.
        preset: u8,
    },
    /// Digital mode inquiry response.
    Digital {
        /// Whether digital mode is enabled.
        on: bool,
    },
    /// Tally auto adjust inquiry response.
    TallyAutoAdjust {
        /// Whether tally auto adjust is enabled.
        on: bool,
    },
    /// RTMP state inquiry response.
    Rtmp {
        /// Whether RTMP streaming is enabled.
        on: bool,
    },
    /// Zoom out state inquiry response.
    ZoomOut {
        /// Whether zoom out is active.
        active: bool,
    },
    /// Zoom in state inquiry response.
    ZoomIn {
        /// Whether zoom in is active.
        active: bool,
    },
    /// Iris up state inquiry response.
    IrisUp {
        /// Whether iris up is active.
        active: bool,
    },
    /// Iris down state inquiry response.
    IrisDown {
        /// Whether iris down is active.
        active: bool,
    },
    /// Night/day position inquiry response.
    NightDayPosition {
        /// Current night/day position value.
        position: u8,
    },
    /// Focus near/far state inquiry response.
    FocusNearFar {
        /// Whether focus near is active (false = far active).
        near: bool,
    },
    /// Zoom tele/wide state inquiry response.
    ZoomTeleWide {
        /// Whether zoom tele is active (false = wide active).
        tele: bool,
    },
    /// Night/day switch inquiry response.
    NightDaySwitch {
        /// Whether night/day switch is enabled.
        enabled: bool,
    },
}

#[cfg(test)]
mod tests {
    use crate::{
        camera_id::CameraId,
        command::{bytes::VISCA_TERMINATOR, encode::ViscaCommand},
    };

    /// Helper to encode a command and verify it has a terminator
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn assert_command_has_terminator<C: ViscaCommand>(command: C, name: &str) {
        let mut buffer = [0u8; 256];
        let result = command.write_into(CameraId::CAMERA_1, &mut buffer);

        assert!(
            result.is_ok(),
            "Command {} failed to encode: {:?}",
            name,
            result
        );

        let len = result.unwrap();
        assert!(len > 0, "Command {} encoded to empty buffer", name);

        // Check that the command ends with VISCA_TERMINATOR
        assert_eq!(
            buffer[len - 1],
            VISCA_TERMINATOR,
            "Command {} does not end with VISCA_TERMINATOR (0xFF). Last byte: 0x{:02X}",
            name,
            buffer[len - 1]
        );

        // Validate no double terminators
        if len > 1 {
            let mut terminator_count = 0;
            for &byte in buffer.iter().take(len) {
                if byte == VISCA_TERMINATOR {
                    terminator_count += 1;
                }
            }
            assert_eq!(
                terminator_count, 1,
                "Command {} has {} terminators, expected exactly 1",
                name, terminator_count
            );
        }
    }

    #[test]
    fn test_power_commands_have_terminator() {
        use crate::command::power::{PowerOn, PowerStandby};
        assert_command_has_terminator(PowerOn::new(), "PowerOn");
        assert_command_has_terminator(PowerStandby::new(), "PowerStandby");
    }

    #[test]
    fn test_zoom_commands_have_terminator() {
        use crate::command::zoom::Zoom;
        assert_command_has_terminator(Zoom::Stop, "Zoom::Stop");
        assert_command_has_terminator(Zoom::TeleStd, "Zoom::TeleStd");
        assert_command_has_terminator(Zoom::WideStd, "Zoom::WideStd");
    }

    #[test]
    fn test_focus_commands_have_terminator() {
        use crate::command::focus::Focus;
        assert_command_has_terminator(Focus::Stop, "Focus::Stop");
        assert_command_has_terminator(Focus::Auto, "Focus::Auto");
        assert_command_has_terminator(Focus::Manual, "Focus::Manual");
        assert_command_has_terminator(Focus::Far, "Focus::Far");
        assert_command_has_terminator(Focus::Near, "Focus::Near");
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_pan_tilt_commands_have_terminator() {
        use crate::command::pan_tilt::PanTilt;
        assert_command_has_terminator(PanTilt::Home, "PanTilt::Home");
        assert_command_has_terminator(PanTilt::Reset, "PanTilt::Reset");
        // Test Move variant with Stop direction
        use crate::command::pan_tilt::PanTiltDirection;
        use crate::types::{PanSpeed, TiltSpeed};
        assert_command_has_terminator(
            PanTilt::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0).unwrap(),
                tilt_speed: TiltSpeed::new(0).unwrap(),
            },
            "PanTilt::Stop",
        );
    }

    #[test]
    fn test_gain_commands_have_terminator() {
        use crate::command::gain::Gain;
        assert_command_has_terminator(Gain::Reset, "Gain::Reset");
        assert_command_has_terminator(Gain::Up, "Gain::Up");
        assert_command_has_terminator(Gain::Down, "Gain::Down");
    }

    #[test]
    fn test_exposure_commands_have_terminator() {
        use crate::command::exposure::{ExposureCommand, ExposureMode};
        assert_command_has_terminator(
            ExposureCommand {
                mode: ExposureMode::Auto,
            },
            "Exposure::Auto",
        );
        assert_command_has_terminator(
            ExposureCommand {
                mode: ExposureMode::Manual,
            },
            "Exposure::Manual",
        );
    }

    #[test]
    fn test_preset_commands_have_terminator() {
        use crate::command::preset::PresetCommand;
        use crate::command::preset::PresetNumber;

        if let Ok(preset) = PresetNumber::new(1) {
            assert_command_has_terminator(
                PresetCommand {
                    action: crate::command::preset::PresetAction::Recall,
                    preset_number: preset,
                },
                "Preset::Recall(1)",
            );
            assert_command_has_terminator(
                PresetCommand {
                    action: crate::command::preset::PresetAction::Set,
                    preset_number: preset,
                },
                "Preset::Set(1)",
            );
        }
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_white_balance_commands_have_terminator() {
        use crate::command::white_balance::{WhiteBalanceCommand, WhiteBalanceMode};

        assert_command_has_terminator(
            WhiteBalanceCommand {
                mode: WhiteBalanceMode::Auto,
            },
            "WhiteBalance::Auto",
        );
        assert_command_has_terminator(
            WhiteBalanceCommand {
                mode: WhiteBalanceMode::Manual,
            },
            "WhiteBalance::Manual",
        );
    }

    #[test]
    fn test_type_state_prevents_unterminated_commands() {
        use crate::command::bytes::ConstCommandBuilder;

        // Create a builder and terminate it
        let builder = ConstCommandBuilder::<8>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x00)
            .push(0x02)
            .terminate();

        // Verify the terminated command has the terminator
        let bytes = builder.as_bytes();
        assert_eq!(bytes[bytes.len() - 1], VISCA_TERMINATOR);

        // Verify we can't access bytes without terminating (compile-time check)
        // The following would not compile:
        // let unterminated = ConstCommandBuilder::<8>::new().push(0x81);
        // let bytes = unterminated.as_bytes(); // ERROR: method not found
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_all_command_categories_terminate() {
        use crate::command::focus::Focus;
        use crate::command::pan_tilt::PanTilt;
        use crate::command::power::PowerOn;
        use crate::command::zoom::Zoom;

        // Test commands from different categories to ensure
        // the macros are correctly applying termination

        type EncodeFunction = Box<dyn Fn(&mut [u8]) -> Result<usize, crate::Error>>;

        struct TestCase {
            name: &'static str,
            encode: EncodeFunction,
        }

        let test_cases = vec![
            TestCase {
                name: "Quick command (Power)",
                encode: Box::new(|buf| PowerOn::new().write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (PanTilt)",
                encode: Box::new(|buf| PanTilt::Home.write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (Zoom)",
                encode: Box::new(|buf| Zoom::Stop.write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (Focus)",
                encode: Box::new(|buf| Focus::Near.write_into(CameraId::CAMERA_1, buf)),
            },
        ];

        for test_case in test_cases {
            let mut buffer = [0u8; 256];
            let result = (test_case.encode)(&mut buffer);

            assert!(result.is_ok(), "{} failed: {:?}", test_case.name, result);
            let len = result.unwrap();

            assert_eq!(
                buffer[len - 1],
                VISCA_TERMINATOR,
                "{} missing terminator",
                test_case.name
            );
        }
    }
}
