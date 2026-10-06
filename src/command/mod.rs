//! Stable low-level VISCA command extension surface.
//!
//! Most users should prefer the camera-first accessor API. This module is for
//! custom command implementations and integrations that need raw VISCA command
//! control. Public items are re-exported from this module root; the category
//! modules that organize the implementation are crate-private.

// Command modules
pub(crate) mod color;
pub(crate) mod exposure;
pub(crate) mod flip;
pub(crate) mod focus;
pub(crate) mod gain;
pub(crate) mod image;
pub(crate) mod inquiry;
pub(crate) mod inquiry_structs; // Internal module for macro-generated inquiry commands
pub(crate) mod inquiry_types;
pub(crate) mod menu;
pub(crate) mod motion_sync;
pub(crate) mod nd_filter;
pub(crate) mod pan_tilt;
pub(crate) mod power;
pub(crate) mod preset;
pub(crate) mod resolution;
pub(crate) mod response;
/// Authoritative semantic classification ledger for built-in commands.
pub(crate) mod semantics;
pub(crate) mod streaming;
/// Crate-private noun/method mapping for the final static camera surface.
///
/// The mapping is an in-crate audit authority; production request preparation
/// consumes typed request implementations directly.
pub(crate) mod surface;
pub(crate) mod system;
pub(crate) mod tally;
pub(crate) mod typed;
pub(crate) mod variable_speed;
pub(crate) mod white_balance;
pub(crate) mod zoom;

// Command encoding implementation. The stable extension surface is re-exported
// from this module root rather than through implementation submodules.
pub(crate) mod bytes;

// Wire encoding implementation. Semantic request policy lives in `Request`.
pub(crate) mod encode;

#[cfg(test)]
pub(crate) fn test_wire_bytes<C: encode::WireEncode>(
    command: &C,
    camera_id: crate::CameraId,
) -> Result<Vec<u8>, crate::Error> {
    let mut buffer = [0u8; 256];
    let len = command.write_into(camera_id, &mut buffer)?;
    Ok(buffer[..len].to_vec())
}

// Re-export command types
pub use self::{
    bytes::VISCA_TERMINATOR,
    color::*,
    encode::CommandKind,
    exposure::*,
    flip::{Flip, ImageFreeze},
    focus::*,
    gain::{Gain, GainLimitCommand},
    image::*,
    inquiry::*,
    inquiry_structs::{InquiryData, InquiryKind},
    inquiry_types::{FlipState, TallyStatusState, VersionInfo},
    menu::*,
    motion_sync::{SetMotionSyncMode, SetMotionSyncPreset},
    nd_filter::*,
    pan_tilt::*,
    power::*,
    preset::*,
    resolution::{NdFilterPosition, PictureEffectMode},
    response::{
        parse_inquiry_payload, BoolConvention, Nibbles, Nibbles4Or8, Payload, RawInquiryPayload,
        Response,
    },
    streaming::{MulticastStreaming, SetNdiQuality, UsbAudio},
    system::{MotionSyncMode, MotionSyncPreset, SettingsSaveCommand},
    tally::{
        TallyBrightHi, TallyBrightLo, TallyFlash, TallyGreenOff, TallyGreenOn, TallyOff, TallyOn,
        TallyRedOff, TallyRedOn,
    },
    typed::ResponseParser,
    variable_speed::*,
    white_balance::*,
    zoom::{DigitalZoom, Zoom},
};

#[cfg(test)]
mod tests {
    use crate::camera_id::CameraId;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::encode::WireEncode;

    /// Helper to encode a command and verify it has a terminator
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn assert_command_has_terminator<C: WireEncode>(command: C, name: &str) {
        let mut buffer = [0u8; 256];
        let result = command.write_into(CameraId::CAMERA_1, &mut buffer);

        assert!(
            result.is_ok(),
            "Command {name} failed to encode: {result:?}"
        );

        let len = result.unwrap();
        assert!(len > 0, "Command {name} encoded to empty buffer");

        // Check that the command ends with VISCA_TERMINATOR
        assert_eq!(
            buffer[len - 1],
            VISCA_TERMINATOR,
            "Command {name} does not end with VISCA_TERMINATOR (0xFF). Last byte: 0x{last_byte:02X}",
            last_byte = buffer[len - 1]
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
                "Command {name} has {terminator_count} terminators, expected exactly 1"
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
    #[allow(clippy::expect_used)]
    fn test_preset_commands_have_terminator() {
        use crate::command::preset::PresetCommand;
        use crate::command::preset::PresetNumber;

        // Not `if let Ok(..)`: a preset range that stopped admitting 1 would
        // silently skip both assertions instead of failing.
        let preset = PresetNumber::new(1).expect("preset 1 is inside every supported preset range");
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

            assert!(
                result.is_ok(),
                "{test_case_name} failed: {result:?}",
                test_case_name = test_case.name
            );
            let len = result.unwrap();

            assert_eq!(
                buffer[len - 1],
                VISCA_TERMINATOR,
                "{test_case_name} missing terminator",
                test_case_name = test_case.name
            );
        }
    }

    /// Guard the wire-byte rows in `docs/visca_reference.md` against the actual
    /// encoders.
    ///
    /// `docs/visca_reference.md` is cited in `README.md` as the evidence base for
    /// built-in profile decisions, yet nothing tied its documented bytes to the
    /// code. Issue #688 found the red/blue tuning rows and the NDI-mode row
    /// carrying wire bytes that contradicted the encoders. Each case below asserts
    /// both that the documented hex prefix still appears verbatim in the reference
    /// and that the live encoder emits a frame starting with those exact bytes, so
    /// the two cannot silently drift apart again.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn visca_reference_wire_rows_match_encoders() {
        use crate::command::color::{
            BlueGain, BlueTuningCommand, HueCommand, RedGain, RedTuningCommand, SaturationCommand,
        };
        use crate::command::streaming::{MulticastStreaming, SetNdiQuality, UsbAudio};
        use crate::types::{
            BlueChannel, BlueTuning, HueLevel, NdiQuality, RedChannel, RedTuning, SaturationLevel,
        };

        const REFERENCE: &str = include_str!("../../docs/visca_reference.md");

        fn parse_hex(spec: &str) -> Vec<u8> {
            spec.split_whitespace()
                .map(|h| u8::from_str_radix(h, 16).unwrap())
                .collect()
        }

        // (row label, documented hex prefix that must appear verbatim in the
        //  reference, live wire frame from the encoder).
        let cases = [
            (
                "red tuning direct",
                "81 01 04 43 00 00 00",
                super::test_wire_bytes(
                    &RedTuningCommand::new(RedTuning::NEUTRAL),
                    CameraId::CAMERA_1,
                )
                .unwrap(),
            ),
            (
                "blue tuning direct",
                "81 01 04 44 00 00 00",
                super::test_wire_bytes(
                    &BlueTuningCommand::new(BlueTuning::NEUTRAL),
                    CameraId::CAMERA_1,
                )
                .unwrap(),
            ),
            (
                "red gain direct (shares 04 43 with red tuning)",
                "81 01 04 43 00 00",
                super::test_wire_bytes(
                    &RedGain::SetValue(RedChannel::new(0x00).unwrap()),
                    CameraId::CAMERA_1,
                )
                .unwrap(),
            ),
            (
                "blue gain direct (shares 04 44 with blue tuning)",
                "81 01 04 44 00 00",
                super::test_wire_bytes(
                    &BlueGain::SetValue(BlueChannel::new(0x00).unwrap()),
                    CameraId::CAMERA_1,
                )
                .unwrap(),
            ),
            (
                "NDI mode",
                "81 0B 01 01 01",
                super::test_wire_bytes(&SetNdiQuality::new(NdiQuality::High), CameraId::CAMERA_1)
                    .unwrap(),
            ),
            (
                "multicast mode",
                "81 0B 01 23",
                super::test_wire_bytes(&MulticastStreaming::On, CameraId::CAMERA_1).unwrap(),
            ),
            (
                "USB audio / UAC",
                "81 2A 02 A0 04",
                super::test_wire_bytes(&UsbAudio::On, CameraId::CAMERA_1).unwrap(),
            ),
            (
                "saturation direct",
                "81 01 04 49 00 00 00",
                super::test_wire_bytes(
                    &SaturationCommand::new(SaturationLevel::MIN),
                    CameraId::CAMERA_1,
                )
                .unwrap(),
            ),
            (
                "hue direct",
                "81 01 04 4F 00 00 00",
                super::test_wire_bytes(&HueCommand::new(HueLevel::MIN), CameraId::CAMERA_1)
                    .unwrap(),
            ),
        ];

        for (label, documented, frame) in &cases {
            assert!(
                REFERENCE.contains(*documented),
                "docs/visca_reference.md is missing the documented wire bytes `{documented}` for {label}"
            );
            let prefix = parse_hex(documented);
            assert!(
                frame.starts_with(&prefix),
                "{label}: encoder emits {frame:02X?}, which does not start with the documented `{documented}` in docs/visca_reference.md"
            );
        }
    }

    /// Every encoder takes its address byte from the frame writer, so framing
    /// one command for camera addresses 1–7 must differ only in byte 0
    /// (`0x8n`). Covers every step-encoder family (each step and its direct
    /// value), the drive encoders, and the macro-generated unit and parameter
    /// commands (#810, #811).
    #[test]
    #[allow(clippy::expect_used)]
    fn every_encoder_family_varies_only_in_the_address_byte() {
        use crate::command::{
            BlueGain, Brightness, ColorTemperature, ExposureCompensation, Focus, Gain, Iris,
            OnePushTriggerCommand, PowerOn, RedGain, SettingsSaveCommand, Sharpness, SharpnessMode,
            Shutter, SpotlightOn, TallyFlash, TallyRedOn, WhiteBalanceCommand, WhiteBalanceMode,
            Zoom,
        };
        use crate::types::{
            BlueChannel, BrightnessLevel, ColorTemp, ExposureCompensationLevel, GainLevel,
            IrisLevel, RedChannel, SharpnessLevel, ShutterSpeed, ZoomSpeed,
        };

        fn frames_differ_only_in_address(command: &dyn WireEncode, name: &str) {
            let encode = |camera: u8| {
                let camera_id = CameraId::new(camera).expect("valid camera");
                let mut buffer = [0u8; 32];
                let len = command
                    .write_into(camera_id, &mut buffer)
                    .expect("encoder must frame");
                buffer[..len].to_vec()
            };
            let first = encode(1);
            for camera in 1..=7 {
                let frame = encode(camera);
                assert_eq!(frame[0], 0x80 | camera, "{name} at camera {camera}");
                assert_eq!(frame[1..], first[1..], "{name} body at camera {camera}");
            }
        }

        let commands: Vec<(&str, Box<dyn WireEncode>)> = vec![
            (
                "ExposureCompensation::Reset",
                Box::new(ExposureCompensation::Reset),
            ),
            (
                "ExposureCompensation::Up",
                Box::new(ExposureCompensation::Up),
            ),
            (
                "ExposureCompensation::Down",
                Box::new(ExposureCompensation::Down),
            ),
            (
                "ExposureCompensation::On",
                Box::new(ExposureCompensation::On),
            ),
            (
                "ExposureCompensation::SetLevel",
                Box::new(ExposureCompensation::SetLevel(
                    ExposureCompensationLevel::new(-3).expect("level"),
                )),
            ),
            ("Iris::Up", Box::new(Iris::Up)),
            (
                "Iris::SetAperture",
                Box::new(Iris::SetAperture(IrisLevel::new(0x0A).expect("iris"))),
            ),
            ("Shutter::Down", Box::new(Shutter::Down)),
            (
                "Shutter::SetSpeed",
                Box::new(Shutter::SetSpeed(ShutterSpeed::new(0x11))),
            ),
            ("Brightness::Reset", Box::new(Brightness::Reset)),
            (
                "Brightness::SetLevel",
                Box::new(Brightness::SetLevel(BrightnessLevel::new(0x11))),
            ),
            ("Gain::Up", Box::new(Gain::Up)),
            (
                "Gain::SetValue",
                Box::new(Gain::SetValue(GainLevel::new(0x0C).expect("gain"))),
            ),
            ("ColorTemperature::Down", Box::new(ColorTemperature::Down)),
            (
                "ColorTemperature::SetTemperature",
                Box::new(ColorTemperature::SetTemperature(
                    ColorTemp::new(0x37).expect("temp"),
                )),
            ),
            ("RedGain::Reset", Box::new(RedGain::Reset)),
            (
                "RedGain::SetValue",
                Box::new(RedGain::SetValue(RedChannel::new(0xA5).expect("red"))),
            ),
            ("BlueGain::Up", Box::new(BlueGain::Up)),
            (
                "BlueGain::SetValue",
                Box::new(BlueGain::SetValue(BlueChannel::new(0x5A).expect("blue"))),
            ),
            ("Sharpness::Down", Box::new(Sharpness::Down)),
            (
                "Sharpness::Mode",
                Box::new(Sharpness::Mode(SharpnessMode::Manual)),
            ),
            (
                "Sharpness::SetLevel",
                Box::new(Sharpness::SetLevel {
                    value: SharpnessLevel::new(0x0F).expect("sharpness"),
                }),
            ),
            (
                "Zoom::TeleVariable",
                Box::new(Zoom::TeleVariable(ZoomSpeed::new(7).expect("zoom"))),
            ),
            ("Focus::Near", Box::new(Focus::Near)),
            ("PowerOn", Box::new(PowerOn::new())),
            ("SpotlightOn", Box::new(SpotlightOn::new())),
            ("TallyRedOn", Box::new(TallyRedOn::new())),
            ("TallyFlash", Box::new(TallyFlash::new())),
            (
                "OnePushTriggerCommand",
                Box::new(OnePushTriggerCommand::new()),
            ),
            ("SettingsSaveCommand", Box::new(SettingsSaveCommand::new())),
            (
                "WhiteBalanceCommand",
                Box::new(WhiteBalanceCommand::new(WhiteBalanceMode::ColorTemperature)),
            ),
        ];
        for (name, command) in &commands {
            frames_differ_only_in_address(command.as_ref(), name);
        }
    }
}
