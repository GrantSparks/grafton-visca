//! Focus control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera focus functionality,
//! including auto/manual modes, directional focus, and direct position control.

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Focus control commands.
///
/// Provides various ways to control camera focus.
#[derive(Debug)]
pub enum FocusCommand {
    /// Stop any focus movement.
    Stop,
    /// Move focus far at standard speed.
    FarStandard,
    /// Move focus near at standard speed.
    NearStandard,
    /// Move focus far at variable speed (0=slowest, 7=fastest).
    FarVariable(u8),
    /// Move focus near at variable speed (0=slowest, 7=fastest).
    NearVariable(u8),
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

impl ViscaCommand for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            FocusCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            FocusCommand::FarStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            FocusCommand::NearStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            FocusCommand::FarVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::NearVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::Direct(position) => {
                let p = ((*position >> 12) & 0x0F) as u8;
                let q = ((*position >> 8) & 0x0F) as u8;
                let r = ((*position >> 4) & 0x0F) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x48, p, q, r, s, 0xFF])
            }
            FocusCommand::Auto => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]),
            FocusCommand::Manual => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]),
            FocusCommand::OnePushTrigger => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]),
            FocusCommand::Infinity => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
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
pub struct FocusZoneCommand {
    /// The focus zone to select.
    pub zone: FocusZone,
}

impl ViscaCommand for FocusZoneCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let zone_byte = match self.zone {
            FocusZone::Top => 0x00,
            FocusZone::Center => 0x01,
            FocusZone::Bottom => 0x02,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x3C, zone_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
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
pub enum AFSensitivity {
    /// High sensitivity - quick focus response to scene changes.
    High,
    /// Normal sensitivity - balanced focus response (default).
    Normal,
    /// Low sensitivity - slower focus response, more stable in changing scenes.
    Low,
}

/// Command to set auto focus sensitivity.
pub struct AFSensitivityCommand {
    /// The sensitivity level to set.
    pub sensitivity: AFSensitivity,
}

impl ViscaCommand for AFSensitivityCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let sens_byte = match self.sensitivity {
            AFSensitivity::High => 0x02,
            AFSensitivity::Normal => 0x01,
            AFSensitivity::Low => 0x00,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x58, sens_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
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
pub struct FocusNearLimitCommand {
    /// The focus position limit (0x0000 to 0xFFFF).
    pub position: u16,
}

impl ViscaCommand for FocusNearLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let p = (self.position >> 12) as u8;
        let q = ((self.position >> 8) & 0x0F) as u8;
        let r = ((self.position >> 4) & 0x0F) as u8;
        let s = (self.position & 0x0F) as u8;
        Ok(vec![0x81, 0x01, 0x04, 0x28, p, q, r, s, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
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
        let cmd = FocusCommand::FarStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_near_standard() {
        let cmd = FocusCommand::NearStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_variable() {
        // Valid speeds
        for speed in 0..=7 {
            let cmd = FocusCommand::FarVariable(speed);
            assert_eq!(
                cmd.to_bytes().unwrap(),
                vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed, 0xFF]
            );
        }

        // Invalid speed
        let cmd = FocusCommand::FarVariable(8);
        assert!(matches!(
            cmd.to_bytes(),
            Err(ViscaError::InvalidParameter(_))
        ));
    }

    #[test]
    fn test_focus_command_near_variable() {
        // Valid speeds
        for speed in 0..=7 {
            let cmd = FocusCommand::NearVariable(speed);
            assert_eq!(
                cmd.to_bytes().unwrap(),
                vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed, 0xFF]
            );
        }

        // Invalid speed
        let cmd = FocusCommand::NearVariable(8);
        assert!(matches!(
            cmd.to_bytes(),
            Err(ViscaError::InvalidParameter(_))
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
    fn test_af_sensitivity_command() {
        let cmd = AFSensitivityCommand {
            sensitivity: AFSensitivity::High,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF]
        );

        let cmd = AFSensitivityCommand {
            sensitivity: AFSensitivity::Normal,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x01, 0xFF]
        );

        let cmd = AFSensitivityCommand {
            sensitivity: AFSensitivity::Low,
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
            AFSensitivityCommand {
                sensitivity: AFSensitivity::High
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
