use super::ViscaResponseType;
use crate::command::ViscaCommand;
use crate::error::ViscaError;
use std::convert::TryFrom;

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
        }
    }
}

#[derive(Debug)]
pub enum PanTiltCommand {
    Home,
    Move {
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            PanTiltCommand::Home => Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF]),
            PanTiltCommand::Move {
                direction,
                pan_speed,
                tilt_speed,
            } => {
                let (dir_byte1, dir_byte2) = direction.to_bytes();
                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x01,
                    (*pan_speed).into(),
                    (*tilt_speed).into(),
                    dir_byte1,
                    dir_byte2,
                    0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PanSpeed(u8);

impl PanSpeed {
    pub const MAX: u8 = 0x18; // Maximum pan speed

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(PanSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Pan speed must be in the range 0x00..=0x{:02X}",
                Self::MAX
            )))
        }
    }
}

impl TryFrom<u8> for PanSpeed {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        PanSpeed::new(value)
    }
}

impl From<PanSpeed> for u8 {
    fn from(speed: PanSpeed) -> Self {
        speed.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    pub const MAX: u8 = 0x14; // Maximum tilt speed

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(TiltSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Tilt speed must be in the range 0x00..=0x{:02X}",
                Self::MAX
            )))
        }
    }
}

impl TryFrom<u8> for TiltSpeed {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        TiltSpeed::new(value)
    }
}

impl From<TiltSpeed> for u8 {
    fn from(speed: TiltSpeed) -> Self {
        speed.0
    }
}
