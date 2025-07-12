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
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{FocusPosition, SpeedLevel}};

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
    Infinity}

impl Focus {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    // }
}

impl EncodeVisca for Focus {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x08;
        buffer[4] = 0x00;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Movement
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
    Bottom}

/// Command to set the focus zone.
#[derive(Debug, Copy, Clone)]
pub(crate) struct FocusZoneCommand {
    /// The focus zone to select.
    pub zone: FocusZone}

impl EncodeVisca for FocusZoneCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        let zone_byte = match self.zone {
            FocusZone::Top => 0x00,
            FocusZone::Center => 0x01,
            FocusZone::Bottom => 0x02};
        
        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x50;
        buffer[3] = zone_byte;
        buffer[4] = 0xFF;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
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
    Low}

/// Command to set auto focus sensitivity.
#[derive(Debug, Copy, Clone)]
pub(crate) struct AutoFocusSensitivityCommand {
    /// The sensitivity level to set.
    pub sensitivity: AutoFocusSensitivity}

impl EncodeVisca for AutoFocusSensitivityCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        let sens_byte = match self.sensitivity {
            AutoFocusSensitivity::High => 0x02,
            AutoFocusSensitivity::Normal => 0x01,
            AutoFocusSensitivity::Low => 0x00};
        
        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x58;
        buffer[4] = sens_byte;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
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
    pub position: FocusPosition}

impl EncodeVisca for FocusNearLimitCommand {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        let pos_val = self.position.value();
        let p0 = ((pos_val >> 12) & 0x0F) as u8;
        let p1 = ((pos_val >> 8) & 0x0F) as u8;
        let p2 = ((pos_val >> 4) & 0x0F) as u8;
        let p3 = (pos_val & 0x0F) as u8;
        
        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x28;
        buffer[4] = p0;
        buffer[5] = p1;
        buffer[6] = p2;
        buffer[7] = p3;
        buffer[8] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Focus Lock command (PTZOptics specific).
///
/// Controls whether the camera locks focus at the current position.
#[derive(Debug, Copy, Clone)]
pub enum FocusLock {
    /// Enable focus lock
    On,
    /// Disable focus lock
    Off}

impl EncodeVisca for FocusLock {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x0A;
        buffer[2] = 0x04;
        buffer[3] = 0x68;
        buffer[4] = 0x02;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Push AF command (FR7 specific).
///
/// Controls the Push Auto Focus feature which temporarily activates
/// auto focus when pressed.
#[derive(Debug, Copy, Clone)]
pub enum PushAF {
    /// Press Push AF button (activate temporary auto focus)
    Press,
    /// Release Push AF button
    Release}

impl EncodeVisca for PushAF {
    type Response = ();
    const MAX_SIZE: usize = 8;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x01;
        buffer[4] = 0x0A;
        buffer[5] = 0x00;
        buffer[6] = 0x01;
        buffer[7] = 0xFF;
        
        Ok(Self::MAX_SIZE)
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

    #[test]
    fn test_focus_command_stop() {
        let cmd = Focus::Stop;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_far_standard() {
        let cmd = Focus::Far;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_near_standard() {
        let cmd = Focus::Near;
        assert_eq!(
            cmd.try_into_vec()
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
            let cmd = Focus::FarWithSpeed(speed);
            assert_eq!(
                cmd.try_into_vec()
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
                cmd.try_into_vec()
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
        let cmd = Focus::Position(
            FocusPosition::new(0x1234).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = Focus::Position(
            FocusPosition::new(0xF000).unwrap_or_else(|e| panic!("Valid focus position: {e:?}")),
        );
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x48, 0x0F, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_auto() {
        let cmd = Focus::Auto;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_manual() {
        let cmd = Focus::Manual;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_one_push_trigger() {
        let cmd = Focus::OnePushTrigger;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_focus_command_infinity() {
        let cmd = Focus::Infinity;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_focus_zone_command() {
        let cmd = FocusZoneCommand {
            zone: FocusZone::Top};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x00, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Center};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x01, 0xFF]
        );

        let cmd = FocusZoneCommand {
            zone: FocusZone::Bottom};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xAA, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_auto_focus_sensitivity_command() {
        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::High};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Normal};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x01, 0xFF]
        );

        let cmd = AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Low};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x58, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_focus_near_limit_command() {
        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1234)
                .unwrap_or_else(|e| panic!("Valid focus position: {e:?}"))};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );

        let cmd = FocusNearLimitCommand {
            position: FocusPosition::new(0x1000)
                .unwrap_or_else(|e| panic!("Valid focus position: {e:?}"))};
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x00, 0x00, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_command_categories() {
        assert_eq!(
            Focus::Stop.timeout_kind(),
            CommandCategory::Movement
        );
        assert_eq!(
            Focus::Auto.timeout_kind(),
            CommandCategory::Movement
        );
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
