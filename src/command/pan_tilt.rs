use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PanTiltDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Stop,
    Home,
}

impl PanTiltDirection {
    pub fn to_bytes(self) -> (u8, u8) {
        match self {
            PanTiltDirection::Up => (0x03, 0x01),
            PanTiltDirection::Down => (0x03, 0x02),
            PanTiltDirection::Left => (0x01, 0x03),
            PanTiltDirection::Right => (0x02, 0x03),
            PanTiltDirection::UpLeft => (0x01, 0x01),
            PanTiltDirection::UpRight => (0x02, 0x01),
            PanTiltDirection::DownLeft => (0x01, 0x02),
            PanTiltDirection::DownRight => (0x02, 0x02),
            PanTiltDirection::Stop => (0x03, 0x03),
            PanTiltDirection::Home => (0x04, 0x04), // Special case for the Home command
        }
    }
}

pub struct PanTiltCommand {
    pub direction: PanTiltDirection,
    pub pan_speed: PanSpeed,
    pub tilt_speed: TiltSpeed,
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let (dir_byte1, dir_byte2) = self.direction.to_bytes();
        if self.direction == PanTiltDirection::Home {
            Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF])
        } else {
            Ok(vec![
                0x81,
                0x01,
                0x06,
                0x01,
                self.pan_speed.get_value(),
                self.tilt_speed.get_value(),
                dir_byte1,
                dir_byte2,
                0xFF,
            ])
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PanSpeed(u8);

impl PanSpeed {
    pub const STOP: PanSpeed = PanSpeed(0x00);
    pub const LOW_SPEED: PanSpeed = PanSpeed(0x01);
    pub const HIGH_SPEED: PanSpeed = PanSpeed(0x18);

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if Self::is_valid_value(value) {
            Ok(PanSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(
                "Pan speed must be in the range 0x00..=0x18".into(),
            ))
        }
    }

    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    pub fn get_value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    pub const STOP: TiltSpeed = TiltSpeed(0x00);
    pub const LOW_SPEED: TiltSpeed = TiltSpeed(0x01);
    pub const HIGH_SPEED: TiltSpeed = TiltSpeed(0x14);

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if Self::is_valid_value(value) {
            Ok(TiltSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(
                "Tilt speed must be in the range 0x00..=0x14".into(),
            ))
        }
    }

    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    pub fn get_value(&self) -> u8 {
        self.0
    }
}
