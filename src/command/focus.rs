//! Focus control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera focus functionality,
//! including auto/manual modes, directional focus, and direct position control.
//!
//! # VISCA Compliance
//! Most commands in this module are part of the baseline VISCA specification.
//!
//! ## Vendor-Specific Commands
//! - `FocusLock` - PTZOptics specific
//! - `PushAF` - Sony FR7 specific

// Standard library imports
use std::borrow::Cow;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{FocusPosition, SpeedLevel},
    visca_command,
};

/// Focus mode setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    /// Automatic focus mode.
    Auto,
    /// Manual focus mode.
    Manual,
}

impl TryFrom<u8> for FocusMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(FocusMode::Auto),
            0x03 => Ok(FocusMode::Manual),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x02 (Auto) or 0x03 (Manual)"),
                actual: vec![value],
            }),
        }
    }
}

/// Focus range setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRange {
    /// Normal focus range.
    Normal,
    /// 10x focus range.
    Range10x,
    /// 4.3x focus range.
    Range4_3x,
    /// 2.1x focus range.
    Range2_1x,
    /// 1x focus range.
    Range1x,
    /// 0.35x focus range.
    Range0_35x,
}

impl TryFrom<u8> for FocusRange {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(FocusRange::Normal),
            0x01 => Ok(FocusRange::Range10x),
            0x02 => Ok(FocusRange::Range4_3x),
            0x03 => Ok(FocusRange::Range2_1x),
            0x04 => Ok(FocusRange::Range1x),
            0x05 => Ok(FocusRange::Range0_35x),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x00-0x05 (focus range)"),
                actual: vec![value],
            }),
        }
    }
}

crate::visca_bounded_param! {
    /// Variable focus speed.
    ///
    /// Valid range: 0 to 7 where 0 is the slowest and 7 is the fastest.
    FocusSpeed: u8 {
        min: 0,
        max: 7
    }
}

impl From<SpeedLevel> for FocusSpeed {
    fn from(level: SpeedLevel) -> Self {
        Self(level.to_focus_speed())
    }
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
}

impl Focus {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    // }
}

impl EncodeVisca for Focus {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::CommandBuilder;

        match self {
            Self::Stop | Self::Far | Self::Near => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(&[0x81, 0x01, 0x04, 0x08])
                    .push(match self {
                        Self::Stop => 0x00,
                        Self::Far => 0x02,
                        Self::Near => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::FarWithSpeed(_) | Self::NearWithSpeed(_) => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(&[0x81, 0x01, 0x04, 0x08])
                    .push(match self {
                        Self::FarWithSpeed(s) => 0x20 | s.value(),
                        Self::NearWithSpeed(s) => 0x30 | s.value(),
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::Position(position) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(&[0x81, 0x01, 0x04, 0x48])
                    .push_visca_u16(position.value())
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::Auto | Self::Manual => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(&[0x81, 0x01, 0x04, 0x38])
                    .push(match self {
                        Self::Auto => 0x02,
                        Self::Manual => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::OnePushTrigger | Self::Infinity => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(&[0x81, 0x01, 0x04, 0x18])
                    .push(match self {
                        Self::OnePushTrigger => 0x01,
                        Self::Infinity => 0x02,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

/// Focus Zone selection (baseline VISCA).
///
/// Determines which area of the image the camera uses for auto focus.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FocusZone {
    /// Focus on the top area of the image.
    Top,
    /// Focus on the center area of the image (default).
    Center,
    /// Focus on the bottom area of the image.
    Bottom,
}

impl TryFrom<u8> for FocusZone {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(FocusZone::Top),
            0x01 => Ok(FocusZone::Center),
            0x02 => Ok(FocusZone::Bottom),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x00 (Top), 0x01 (Center), or 0x02 (Bottom)"),
                actual: vec![value],
            }),
        }
    }
}

crate::visca_builder! {
    /// Command to set the focus zone.
    pub(crate) struct FocusZoneCommand {
        /// The focus zone to select.
        zone: FocusZone,
    }
    builder<6> => |builder, zone| {
        let zone_byte = match *zone {
            FocusZone::Top => 0x00,
            FocusZone::Center => 0x01,
            FocusZone::Bottom => 0x02,
        };
        let _ = builder.append(&[0x81, 0x01, 0x04, 0xAA]);
        let _ = builder.push(zone_byte);
        let _ = builder.push(0xFF);
    }
    timeout = Quick;
}

/// Auto Focus Sensitivity levels.
///
/// Controls how responsive the auto focus system is to changes in the scene.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AutoFocusSensitivity {
    /// High sensitivity - quick focus response to scene changes.
    High,
    /// Normal sensitivity - balanced focus response (default).
    Normal,
    /// Low sensitivity - slower focus response, more stable in changing scenes.
    Low,
}

impl TryFrom<u8> for AutoFocusSensitivity {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(AutoFocusSensitivity::High),
            0x01 => Ok(AutoFocusSensitivity::Normal),
            0x00 => Ok(AutoFocusSensitivity::Low),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x00 (Low), 0x01 (Normal), or 0x02 (High)"),
                actual: vec![value],
            }),
        }
    }
}

crate::visca_builder! {
    /// Command to set auto focus sensitivity.
    pub(crate) struct AutoFocusSensitivityCommand {
        /// The sensitivity level to set.
        sensitivity: AutoFocusSensitivity,
    }
    builder<6> => |builder, sensitivity| {
        let sens_byte = match *sensitivity {
            AutoFocusSensitivity::High => 0x02,
            AutoFocusSensitivity::Normal => 0x01,
            AutoFocusSensitivity::Low => 0x00,
        };
        let _ = builder.append(&[0x81, 0x01, 0x04, 0x58]);
        let _ = builder.push(sens_byte);
        let _ = builder.push(0xFF);
    }
    timeout = Quick;
}

crate::visca_builder! {
    /// Command to set the focus near limit.
    ///
    /// Sets the minimum focus distance to prevent the camera from
    /// focusing on objects too close to the lens.
    pub(crate) struct FocusNearLimitCommand {
        /// The focus position limit.
        position: FocusPosition,
    }
    builder<9> => |builder, position| {
        let pos_val = position.value();
        let p0 = ((pos_val >> 12) & 0x0F) as u8;
        let p1 = ((pos_val >> 8) & 0x0F) as u8;
        let p2 = ((pos_val >> 4) & 0x0F) as u8;
        let p3 = (pos_val & 0x0F) as u8;

        let _ = builder.append(&[0x81, 0x01, 0x04, 0x28]);
        let _ = builder.push(p0);
        let _ = builder.push(p1);
        let _ = builder.push(p2);
        let _ = builder.push(p3);
        let _ = builder.push(0xFF);
    }
    timeout = Quick;
}

visca_command! {
    /// Focus Lock command.
    ///
    /// Controls whether the camera locks focus at the current position.
    ///
    /// **Vendor-Specific**: This command is specific to PTZOptics cameras.
    category = "Quick",
    enum FocusLock {
        /// Enable focus lock
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::focus::LOCK_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable focus lock
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::focus::LOCK_PREFIX)
                .push(0x03)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

/// Push AF command.
///
/// Controls the Push Auto Focus feature which temporarily activates
/// auto focus when pressed.
///
/// **Vendor-Specific**: This command is specific to Sony FR7 cameras.
#[derive(Debug, Copy, Clone)]
pub enum PushAF {
    /// Press Push AF button (activate temporary auto focus)
    Press,
    /// Release Push AF button
    Release,
}

impl EncodeVisca for PushAF {
    type Response = ();
    const MAX_SIZE: usize = 8;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::CommandBuilder;

        let mut builder = CommandBuilder::<8>::new();
        builder
            .append(&[0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00])
            .push(match self {
                Self::Press => 0x01,
                Self::Release => 0x00,
            })
            .with_camera_id(camera_id)
            .finalize();
        builder.copy_to(buffer)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::command::encode_visca::EncodeVisca;
    use crate::visca_test;

    visca_test!(
        Focus,
        test_focus_command_stop,
        Focus::Stop,
        &[0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
    );

    visca_test!(
        Focus,
        test_focus_command_far_standard,
        Focus::Far,
        &[0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
    );

    visca_test!(
        Focus,
        test_focus_command_near_standard,
        Focus::Near,
        &[0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
    );

    #[test]
    fn test_focus_command_far_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Focus::FarWithSpeed(speed);
            assert_eq!(
                cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed_val, 0xFF]
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
                cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed_val, 0xFF]
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
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_focus_command_position() {
        let cmd = Focus::Position(
            FocusPosition::new(0x1234).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = Focus::Position(
            FocusPosition::new(0xF000).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x0F, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    visca_test!(
        Focus,
        test_focus_command_auto,
        Focus::Auto,
        &[0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
    );

    visca_test!(
        Focus,
        test_focus_command_manual,
        Focus::Manual,
        &[0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
    );

    visca_test!(
        Focus,
        test_focus_command_one_push_trigger,
        Focus::OnePushTrigger,
        &[0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
    );

    visca_test!(
        Focus,
        test_focus_command_infinity,
        Focus::Infinity,
        &[0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
    );

    #[test]
    fn test_focus_zone_command() {
        let cmd = FocusZoneCommand {
            zone: FocusZone::Top,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x00, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Center,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x01, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Bottom,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_auto_focus_sensitivity_command() {
        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::High,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Normal,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x01, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Low,
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_near_limit_command() {
        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1234)
                .unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1000)
                .unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_command_categories() {
        assert_eq!(Focus::Stop.timeout_kind(), CommandCategory::Movement);
        assert_eq!(Focus::Auto.timeout_kind(), CommandCategory::Movement);
        assert_eq!(
            FocusZoneCommand {
                zone: FocusZone::Top
            }
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            AutoFocusSensitivityCommand {
                sensitivity: AutoFocusSensitivity::High
            }
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            FocusNearLimitCommand {
                position: FocusPosition::new(0x1000)
                    .unwrap_or_else(|e| panic!("Valid focus position: {e:?}"))
            }
            .timeout_kind(),
            CommandCategory::Quick
        );
    }
}
