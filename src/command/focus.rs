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

/// Focus control commands.
///
/// Provides various ways to control camera focus.
#[derive(Debug, Copy, Clone)]
pub enum FocusCommand {
    /// Stop any focus movement.
    Stop,
    /// Move focus far at standard speed.
    FocusFarStandard,
    /// Move focus near at standard speed.
    FocusNearStandard,
    /// Move focus far at variable speed.
    FarVariable(FocusSpeed),
    /// Move focus near at variable speed.
    NearVariable(FocusSpeed),
    /// Set focus to specific position (0x0000 to 0xFFFF).
    Direct(u16),
    /// Enable auto focus mode.
    Auto,
    /// Enable manual focus mode.
    Manual,
    /// Trigger one-push auto focus (focus once then return to manual).
    OnePushTrigger,
    /// Set focus to infinity.
    Infinity,
}

impl Command for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            Self::FocusFarStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            Self::FocusNearStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            Self::FarVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed.value(), 0xFF])
            }
            Self::NearVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed.value(), 0xFF])
            }
            Self::Direct(position) => {
                let p = ((*position >> 12) & 0x0F) as u8;
                let q = ((*position >> 8) & 0x0F) as u8;
                let r = ((*position >> 4) & 0x0F) as u8;
                let s = (*position & 0x0F) as u8;
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
            Self::Direct(position) => {
                let (min, max) = model.focus_range();
                if *position < min || *position > max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "FocusDirect".to_string(),
                        reason: format!(
                            "Position 0x{position:04X} out of range [0x{min:04X}, 0x{max:04X}] for {model:?}"
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
#[derive(Debug, Copy, Clone)]
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
pub struct FocusZoneCommand {
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
        Ok(vec![0x81, 0x01, 0x04, 0x3C, zone_byte, 0xFF])
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
#[derive(Debug, Copy, Clone)]
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
pub struct AutoFocusSensitivityCommand {
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

// Use the visca_param_command! macro for FocusNearLimitCommand
/// Command to set the focus near limit.
///
/// Sets the minimum focus distance to prevent the camera from
/// focusing on objects too close to the lens.
#[derive(Debug, Clone, Copy)]
pub struct FocusNearLimitCommand {
    /// The focus position limit (0x0000 to 0xFFFF).
    pub position: u16,
}

impl Command for FocusNearLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let p0 = ((self.position >> 12) & 0x0F) as u8;
        let p1 = ((self.position >> 8) & 0x0F) as u8;
        let p2 = ((self.position >> 4) & 0x0F) as u8;
        let p3 = (self.position & 0x0F) as u8;
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
                // G2 uses same range as Focus Direct: 0x1000-0xF000
                if self.position < 0x1000 || self.position > 0xF000 {
                    return Err(Error::ModelValidation {
                        model,
                        command: "FocusNearLimit".to_string(),
                        reason: format!(
                            "Position {:#06X} not supported on G2 cameras (range is 0x1000-0xF000)",
                            self.position
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_command_stop() {
        let cmd = FocusCommand::Stop;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_standard() {
        let cmd = FocusCommand::FocusFarStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_near_standard() {
        let cmd = FocusCommand::FocusNearStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val).unwrap();
            let cmd = FocusCommand::FarVariable(speed);
            assert_eq!(
                cmd.to_bytes().unwrap(),
                vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed_val, 0xFF]
            );
        }
    }

    #[test]
    fn test_focus_command_near_variable() {
        // Valid speeds
        for speed_val in 0..=7 {
            let speed = FocusSpeed::new(speed_val).unwrap();
            let cmd = FocusCommand::NearVariable(speed);
            assert_eq!(
                cmd.to_bytes().unwrap(),
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
    fn test_focus_command_direct() {
        let cmd = FocusCommand::Direct(0x1234);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x48, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusCommand::Direct(0xFFFF);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x48, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_auto() {
        let cmd = FocusCommand::Auto;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_manual() {
        let cmd = FocusCommand::Manual;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_one_push_trigger() {
        let cmd = FocusCommand::OnePushTrigger;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_infinity() {
        let cmd = FocusCommand::Infinity;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_zone_command() {
        let cmd = FocusZoneCommand {
            zone: FocusZone::Top,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3C, 0x00, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Center,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3C, 0x01, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Bottom,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3C, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_auto_focus_sensitivity_command() {
        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::High,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Normal,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x01, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Low,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_near_limit_command() {
        let cmd = FocusNearLimitCommand { position: 0x1234 };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusNearLimitCommand { position: 0x0000 };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x28, 0x00, 0x00, 0x00, 0x00, 0xFF]
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
            FocusNearLimitCommand { position: 0 }.command_category(),
            CommandCategory::Quick
        );
    }
}
