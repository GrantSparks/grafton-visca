//! Focus control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera focus functionality,
//! including auto/manual modes, directional focus, and direct position control.
//!
//! # VISCA Compliance
//! Most commands in this module are part of the baseline VISCA specification.
//!
//! ## Vendor-Specific Commands
//! - `FocusLock` - PtzOptics specific
//! - `PushAF` - Sony FR7 specific

use grafton_visca_macros::ViscaEnum;

use crate::{
    command::{
        bytes::{constants::focus, nibbles, FrameWriter, Step},
        encode::WireEncode,
    },
    error::Error,
    types::{FocusPosition, FocusSpeed},
    visca_command,
};

/// Focus mode setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum FocusMode {
    /// Automatic focus mode.
    Auto = 0x02,
    /// Manual focus mode.
    Manual = 0x03,
}

/// Focus range setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum FocusRange {
    /// Normal focus range.
    Normal = 0x00,
    /// 10x focus range.
    Range10x = 0x01,
    /// 4.3x focus range.
    Range4_3x = 0x02,
    /// 2.1x focus range.
    Range2_1x = 0x03,
    /// 1x focus range.
    Range1x = 0x04,
    /// 0.35x focus range.
    Range0_35x = 0x05,
}

/// Focus control commands.
///
/// Provides various ways to control camera focus.
#[derive(Debug, Copy, Clone)]
pub enum Focus {
    /// Stop any focus movement.
    Stop,
    /// Move focus far at standard speed.
    Far,
    /// Move focus near at standard speed.
    Near,
    /// Move focus far at variable speed.
    FarWithSpeed(FocusSpeed),
    /// Move focus near at variable speed.
    NearWithSpeed(FocusSpeed),
    /// Set focus to specific position.
    Position(FocusPosition),
    /// Enable auto focus mode.
    Auto,
    /// Enable manual focus mode.
    Manual,
    /// Trigger one-push auto focus (focus once then return to manual).
    OnePushTrigger,
    /// Set focus to infinity.
    Infinity,
    /// Toggle between auto and manual focus modes.
    ///
    /// **Vendor-Specific**: PTZOptics cameras.
    Toggle,
    /// Snap focus (one-push AF while in manual mode).
    ///
    /// Triggers a single autofocus operation, then returns to manual focus mode.
    /// **Vendor-Specific**: Some vendor/firmware command references document
    /// this opcode, but built-in profiles only expose it when their profile
    /// metadata reports one-push focus support.
    Snap,
}

impl WireEncode for Focus {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let frame = FrameWriter::new(camera_id, buffer);
        match self {
            Self::Stop => frame.bytes(&focus::DRIVE).byte(Step::Reset.byte()),
            Self::Far => frame.bytes(&focus::DRIVE).byte(Step::Up.byte()),
            Self::Near => frame.bytes(&focus::DRIVE).byte(Step::Down.byte()),
            Self::FarWithSpeed(speed) => frame
                .bytes(&focus::DRIVE)
                .byte(Step::Up.at_speed(speed.value())),
            Self::NearWithSpeed(speed) => frame
                .bytes(&focus::DRIVE)
                .byte(Step::Down.at_speed(speed.value())),
            Self::Position(position) => frame.bytes(&focus::DIRECT).nibbles::<4>(position.value()),
            Self::Auto => frame.bytes(&focus::MODE).byte(u8::from(FocusMode::Auto)),
            Self::Manual => frame.bytes(&focus::MODE).byte(u8::from(FocusMode::Manual)),
            Self::Snap => frame.bytes(&focus::MODE).byte(0x04),
            Self::Toggle => frame.bytes(&focus::MODE).byte(0x10),
            Self::OnePushTrigger => frame.bytes(&focus::ONE_PUSH).byte(0x01),
            Self::Infinity => frame.bytes(&focus::ONE_PUSH).byte(0x02),
        }
        .finish()
    }
}

/// Focus Zone selection (`CAM_AFZone`, `8x 01 04 AA 0p FF`).
///
/// Determines which area of the image the camera weights for auto focus. The
/// focus-zone inquiry (`8x 09 04 AA FF`) replies `y0 50 0p FF` with the same
/// values, so every variant read back from a camera can be set again unchanged.
///
/// The variant set is evidence-driven rather than fixed by one specification:
/// `Top`, `Center` and `Bottom` are the documented `AF Zone weight select`
/// values, and [`FocusZone::Zone03`] is a value from the PTZOptics G2 bench
/// (#795) that no vendor source names yet. The enum is `#[non_exhaustive]` so
/// further evidenced values can be added without a breaking change; match it
/// with a wildcard arm.
///
/// Decoding accepts every variant from any camera. Sending is gated per
/// profile: the typed setter refuses a value outside
/// [`Capabilities::focus_zones`](crate::capabilities::Capabilities::focus_zones)
/// before any I/O.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum FocusZone {
    /// Weight auto focus toward the top area of the image (`p = 0`).
    Top = 0x00,
    /// Weight auto focus toward the center area of the image (`p = 1`, default).
    Center = 0x01,
    /// Weight auto focus toward the bottom area of the image (`p = 2`).
    Bottom = 0x02,
    /// Focus-zone value `p = 3`, not named by any vendor source.
    ///
    /// On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM
    /// 6.3.51THI, 6.3.76THI and 6.4.18SHI; 2026-10-04; #795) the cameras report
    /// it from the focus-zone inquiry, accept `8x 01 04 AA 03 FF` with ACK and
    /// completion, and read it back afterwards. Which image area it weights is
    /// not documented, so the library makes no claim about it beyond
    /// preserving the value: read it, store it, and set it back. Only the
    /// `PtzOpticsG2` and `PtzOptics30X` profiles admit it for sending.
    Zone03 = 0x03,
}

visca_command! {
    /// Command to set the focus zone.
    pub struct FocusZoneCommand {
        zone: FocusZone,
    };
    prefix = focus::ZONE;
    param = u8::from(*zone);
    max_param_size = 1;
}

impl FocusZoneCommand {
    /// Create a new focus zone command.
    pub fn new(zone: FocusZone) -> Self {
        Self { zone }
    }
}

/// Auto Focus Sensitivity levels.
///
/// Controls how responsive the auto focus system is to changes in the scene.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum AutoFocusSensitivity {
    /// Low sensitivity - slower focus response, more stable in changing scenes.
    Low = 0x03,
    /// Normal sensitivity - balanced focus response (default).
    Normal = 0x02,
    /// High sensitivity - quick focus response to scene changes.
    High = 0x01,
}

visca_command! {
    /// Command to set auto focus sensitivity.
    pub struct AutoFocusSensitivityCommand {
        sensitivity: AutoFocusSensitivity,
    };
    prefix = focus::AF_SENSITIVITY;
    param = u8::from(*sensitivity);
    max_param_size = 1;
}

impl AutoFocusSensitivityCommand {
    /// Create a new auto focus sensitivity command.
    pub fn new(sensitivity: AutoFocusSensitivity) -> Self {
        Self { sensitivity }
    }
}

visca_command! {
    /// Command to set the focus near limit.
    ///
    /// Sets the minimum focus distance to prevent the camera from
    /// focusing on objects too close to the lens.
    pub struct FocusNearLimitCommand {
        position: FocusPosition,
    };
    prefix = focus::NEAR_LIMIT;
    param = nibbles::<4>(position.value().into());
    max_param_size = 4;
}

impl FocusNearLimitCommand {
    /// Create a new focus near limit command.
    pub fn new(position: FocusPosition) -> Self {
        Self { position }
    }
}

/// Focus Lock command.
///
/// Controls whether the camera locks focus at the current position.
///
/// **Vendor-Specific**: This command is specific to PtzOptics cameras.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum FocusLock {
    /// Enable focus lock
    On,
    /// Disable focus lock
    Off,
}

impl WireEncode for FocusLock {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&focus::LOCK)
            .byte(match self {
                Self::On => 0x02,
                Self::Off => 0x03,
            })
            .finish()
    }
}

/// Push AF command.
///
/// Controls the Push Auto Focus feature which temporarily activates
/// auto focus when pressed.
///
/// **Vendor-Specific**: This command is specific to Sony FR7 cameras.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PushAF {
    /// Press Push AF button (activate temporary auto focus)
    Press,
    /// Release Push AF button
    Release,
}

impl WireEncode for PushAF {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&focus::PUSH_AF)
            .byte(match self {
                Self::Press => 0x01,
                Self::Release => 0x00,
            })
            .finish()
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    unused_qualifications
)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        Focus,
        test_focus_command_stop,
        Focus::Stop,
        &[0x81, 0x01, 0x04, 0x08, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_far_standard,
        Focus::Far,
        &[0x81, 0x01, 0x04, 0x08, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_near_standard,
        Focus::Near,
        &[0x81, 0x01, 0x04, 0x08, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_focus_command_far_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Focus::FarWithSpeed(speed);
            assert_eq!(
                crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                    .unwrap(),
                vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed_val, VISCA_TERMINATOR]
            );
        }
    }

    #[test]
    fn test_focus_command_near_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Focus::NearWithSpeed(speed);
            assert_eq!(
                crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                    .unwrap(),
                vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed_val, VISCA_TERMINATOR]
            );
        }
    }

    #[test]
    fn test_focus_speed_validation() {
        // Valid speeds
        assert!(FocusSpeed::new(0).is_ok());
        assert!(FocusSpeed::new(7).is_ok());

        // Invalid speeds
        assert!(matches!(
            FocusSpeed::new(8),
            Err(Error::ParameterOutOfRange {
                parameter: "FocusSpeed",
                value: 8,
                min: 0,
                max: 7,
            })
        ));
    }

    #[test]
    fn test_focus_command_position() {
        let cmd = Focus::Position(FocusPosition::new(0x1234));
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![
                0x81,
                0x01,
                0x04,
                0x48,
                0x01,
                0x02,
                0x03,
                0x04,
                VISCA_TERMINATOR
            ]
        );

        let cmd = Focus::Position(FocusPosition::new(0xF000));
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![
                0x81,
                0x01,
                0x04,
                0x48,
                0x0F,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }

    visca_test!(
        Focus,
        test_focus_command_auto,
        Focus::Auto,
        &[0x81, 0x01, 0x04, 0x38, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_manual,
        Focus::Manual,
        &[0x81, 0x01, 0x04, 0x38, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_toggle,
        Focus::Toggle,
        &[0x81, 0x01, 0x04, 0x38, 0x10, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_snap,
        Focus::Snap,
        &[0x81, 0x01, 0x04, 0x38, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_one_push_trigger,
        Focus::OnePushTrigger,
        &[0x81, 0x01, 0x04, 0x18, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        Focus,
        test_focus_command_infinity,
        Focus::Infinity,
        &[0x81, 0x01, 0x04, 0x18, 0x02, VISCA_TERMINATOR]
    );

    #[test]
    fn test_focus_zone_command() {
        let cmd = FocusZoneCommand {
            zone: FocusZone::Top,
        };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0xAA, 0x00, VISCA_TERMINATOR]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Center,
        };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0xAA, 0x01, VISCA_TERMINATOR]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Bottom,
        };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0xAA, 0x02, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_auto_focus_sensitivity_command() {
        for (sensitivity, wire_value) in [
            (AutoFocusSensitivity::High, 0x01),
            (AutoFocusSensitivity::Normal, 0x02),
            (AutoFocusSensitivity::Low, 0x03),
        ] {
            let cmd = AutoFocusSensitivityCommand::new(sensitivity);
            assert_eq!(
                crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                    .unwrap(),
                vec![0x81, 0x01, 0x04, 0x58, wire_value, VISCA_TERMINATOR]
            );

            let decoded = crate::command::parse_inquiry_payload(
                &[wire_value],
                &crate::command::InquiryKind::AutoFocusSensitivity,
            )
            .expect("AF sensitivity command value must decode as the same setting");
            assert!(matches!(
                decoded,
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::AutoFocusSensitivity {
                        sensitivity: decoded_sensitivity,
                    }
                ) if decoded_sensitivity == sensitivity
            ));
        }
    }

    #[test]
    fn test_focus_near_limit_command() {
        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1234),
        };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![
                0x81,
                0x01,
                0x04,
                0x28,
                0x01,
                0x02,
                0x03,
                0x04,
                VISCA_TERMINATOR
            ]
        );

        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1000),
        };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![
                0x81,
                0x01,
                0x04,
                0x28,
                0x01,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }
}
