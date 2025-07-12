//! Focus control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera focus functionality,
//! including auto/manual modes, directional focus, and direct position control.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{Command, ResponseType},
    constants::{CameraConstants, CameraModel},
    error::Error,
    timeout::CommandCategory,
    types::{FocusPosition, SpeedLevel},
};

crate::visca_bounded_param! {
    /// Variable focus speed.
    ///
    /// Valid range: 0 to 7 where 0 is the slowest and 7 is the fastest.
    FocusSpeed: u8 {
        min: 0,
        max: 7,
        error_msg: "Focus speed must be in the range 0..=7"
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
pub enum FocusCommand {
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

impl FocusCommand {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    // }
}

impl Command for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            Self::Far => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            Self::Near => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            Self::FarWithSpeed(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed.value(), 0xFF])
            }
            Self::NearWithSpeed(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed.value(), 0xFF])
            }
            Self::Position(position) => {
                let pos_val = position.value();
                let p = ((pos_val >> 12) & 0x0F) as u8;
                let q = ((pos_val >> 8) & 0x0F) as u8;
                let r = ((pos_val >> 4) & 0x0F) as u8;
                let s = (pos_val & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x48, p, q, r, s, 0xFF])
            }
            Self::Auto => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]),
            Self::Manual => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]),
            Self::OnePushTrigger => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]),
            Self::Infinity => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::Position(position) => {
                let (min, max) = model.focus_range();
                if position.value() < min || position.value() > max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "FocusPosition".to_string(),
                        reason: format!(
                            "Position 0x{:04X} out of range [0x{min:04X}, 0x{max:04X}] for {model:?}", position.value()
                        ),
                    });
                }
                Ok(())
            }
            // Other focus commands are generally supported by all models
            _ => Ok(()),
        }
    }
}

/// Focus Zone selection.
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

/// Command to set the focus zone.
#[derive(Debug, Copy, Clone)]
pub(crate) struct FocusZoneCommand {
    /// The focus zone to select.
    pub zone: FocusZone,
}

impl Command for FocusZoneCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let zone_byte = match self.zone {
            FocusZone::Top => 0x00,
            FocusZone::Center => 0x01,
            FocusZone::Bottom => 0x02,
        };
        Ok(vec![0x81, 0x01, 0x04, 0xAA, zone_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
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

/// Command to set auto focus sensitivity.
#[derive(Debug, Copy, Clone)]
pub(crate) struct AutoFocusSensitivityCommand {
    /// The sensitivity level to set.
    pub sensitivity: AutoFocusSensitivity,
}

impl Command for AutoFocusSensitivityCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let sens_byte = match self.sensitivity {
            AutoFocusSensitivity::High => 0x02,
            AutoFocusSensitivity::Normal => 0x01,
            AutoFocusSensitivity::Low => 0x00,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x58, sens_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Command to set the focus near limit.
///
/// Sets the minimum focus distance to prevent the camera from
/// focusing on objects too close to the lens.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusNearLimitCommand {
    /// The focus position limit.
    pub position: FocusPosition,
}

impl Command for FocusNearLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let pos_val = self.position.value();
        let p0 = ((pos_val >> 12) & 0x0F) as u8;
        let p1 = ((pos_val >> 8) & 0x0F) as u8;
        let p2 = ((pos_val >> 4) & 0x0F) as u8;
        let p3 = (pos_val & 0x0F) as u8;
        Ok(vec![0x81, 0x01, 0x04, 0x28, p0, p1, p2, p3, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                // G2 uses same range as Focus Position: 0x1000-0xF000
                if self.position.value() < 0x1000 || self.position.value() > 0xF000 {
                    return Err(Error::ModelValidation {
                        model,
                        command: "FocusNearLimit".to_string(),
                        reason: format!(
                            "Position {:#06X} not supported on G2 cameras (range is 0x1000-0xF000)",
                            self.position.value()
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Focus Lock command (PTZOptics specific).
///
/// Controls whether the camera locks focus at the current position.
#[derive(Debug, Copy, Clone)]
pub enum FocusLockCommand {
    /// Enable focus lock
    On,
    /// Disable focus lock
    Off,
}

impl Command for FocusLockCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::On => Ok(vec![0x81, 0x0A, 0x04, 0x68, 0x02, 0xFF]),
            Self::Off => Ok(vec![0x81, 0x0A, 0x04, 0x68, 0x03, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => Ok(()), // Supported
            _ => Err(Error::ModelValidation {
                model,
                command: "FocusLock".to_string(),
                reason: "Focus Lock is only supported on PTZOptics cameras".to_string(),
            }),
        }
    }
}

/// Push AF command (FR7 specific).
///
/// Controls the Push Auto Focus feature which temporarily activates
/// auto focus when pressed.
#[derive(Debug, Copy, Clone)]
pub enum PushAFCommand {
    /// Press Push AF button (activate temporary auto focus)
    Press,
    /// Release Push AF button
    Release,
}

impl Command for PushAFCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Press => Ok(vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x01, 0xFF]),
            Self::Release => Ok(vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x00, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::SonyFR7 => Ok(()), // Supported
            _ => Err(Error::ModelValidation {
                model,
                command: "PushAF".to_string(),
                reason: "Push AF is only supported on Sony FR7 cameras".to_string(),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_command_stop() {
        let cmd = FocusCommand::Stop;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_standard() {
        let cmd = FocusCommand::Far;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_near_standard() {
        let cmd = FocusCommand::Near;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = FocusCommand::FarWithSpeed(speed);
            assert_eq!(
                cmd.to_bytes()
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
            let cmd = FocusCommand::NearWithSpeed(speed);
            assert_eq!(
                cmd.to_bytes()
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
            Err(Error::InvalidParameter(_))
        ));
    }

    #[test]
    fn test_focus_command_position() {
        let cmd = FocusCommand::Position(
            FocusPosition::new(0x1234).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusCommand::Position(
            FocusPosition::new(0xF000).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x0F, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_auto() {
        let cmd = FocusCommand::Auto;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_manual() {
        let cmd = FocusCommand::Manual;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_one_push_trigger() {
        let cmd = FocusCommand::OnePushTrigger;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_infinity() {
        let cmd = FocusCommand::Infinity;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_zone_command() {
        let cmd = FocusZoneCommand {
            zone: FocusZone::Top,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x00, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Center,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x01, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Bottom,
        };
        assert_eq!(
            cmd.to_bytes()
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
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Normal,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x01, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Low,
        };
        assert_eq!(
            cmd.to_bytes()
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
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1000)
                .unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_command_categories() {
        assert_eq!(
            FocusCommand::Stop.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            FocusCommand::Auto.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            FocusZoneCommand {
                zone: FocusZone::Top
            }
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            AutoFocusSensitivityCommand {
                sensitivity: AutoFocusSensitivity::High
            }
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            FocusNearLimitCommand {
                position: FocusPosition::new(0x1000)
                    .unwrap_or_else(|e| panic!("Valid focus position: {e:?}"))
            }
            .command_category(),
            CommandCategory::Quick
        );
    }
}
