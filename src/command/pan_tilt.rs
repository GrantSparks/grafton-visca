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
    Reset,
    Move {
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
    AbsolutePosition {
        pan: i16,
        tilt: i16,
        pan_speed: u8,
        tilt_speed: u8,
    },
    RelativePosition {
        pan: i16,
        tilt: i16,
        pan_speed: u8,
        tilt_speed: u8,
    },
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            PanTiltCommand::Home => Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF]),
            PanTiltCommand::Reset => Ok(vec![0x81, 0x01, 0x06, 0x05, 0xFF]),
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
            PanTiltCommand::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                // Convert pan and tilt positions to 4 nibbles each
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);
                
                if *pan_speed > PanSpeed::MAX {
                    return Err(ViscaError::InvalidParameter(
                        format!("Pan speed must be <= 0x{:02X}", PanSpeed::MAX)
                    ));
                }
                if *tilt_speed > TiltSpeed::MAX {
                    return Err(ViscaError::InvalidParameter(
                        format!("Tilt speed must be <= 0x{:02X}", TiltSpeed::MAX)
                    ));
                }
                
                Ok(vec![
                    0x81, 0x01, 0x06, 0x02,
                    *pan_speed, *tilt_speed,
                    pan_bytes[0], pan_bytes[1], pan_bytes[2], pan_bytes[3],
                    tilt_bytes[0], tilt_bytes[1], tilt_bytes[2], tilt_bytes[3],
                    0xFF,
                ])
            }
            PanTiltCommand::RelativePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                // Convert pan and tilt positions to 4 nibbles each
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);
                
                if *pan_speed > PanSpeed::MAX {
                    return Err(ViscaError::InvalidParameter(
                        format!("Pan speed must be <= 0x{:02X}", PanSpeed::MAX)
                    ));
                }
                if *tilt_speed > TiltSpeed::MAX {
                    return Err(ViscaError::InvalidParameter(
                        format!("Tilt speed must be <= 0x{:02X}", TiltSpeed::MAX)
                    ));
                }
                
                Ok(vec![
                    0x81, 0x01, 0x06, 0x03,
                    *pan_speed, *tilt_speed,
                    pan_bytes[0], pan_bytes[1], pan_bytes[2], pan_bytes[3],
                    tilt_bytes[0], tilt_bytes[1], tilt_bytes[2], tilt_bytes[3],
                    0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

// Helper function to convert a 15-bit signed position to 4 nibbles
fn position_to_bytes(position: i16) -> [u8; 4] {
    // VISCA uses 15-bit signed values represented as 4 nibbles
    let unsigned = position as u16;
    [
        ((unsigned >> 12) & 0x0F) as u8,
        ((unsigned >> 8) & 0x0F) as u8,
        ((unsigned >> 4) & 0x0F) as u8,
        (unsigned & 0x0F) as u8,
    ]
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

#[derive(Debug, Copy, Clone)]
pub enum LimitCorner {
    DownLeft = 0,
    UpRight = 1,
}

#[derive(Debug)]
pub enum PanTiltLimitCommand {
    Set {
        corner: LimitCorner,
        pan: i16,
        tilt: i16,
    },
    Clear {
        corner: LimitCorner,
    },
}

impl ViscaCommand for PanTiltLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            PanTiltLimitCommand::Set { corner, pan, tilt } => {
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);
                
                Ok(vec![
                    0x81, 0x01, 0x06, 0x07, 0x00, *corner as u8,
                    pan_bytes[0], pan_bytes[1], pan_bytes[2], pan_bytes[3],
                    tilt_bytes[0], tilt_bytes[1], tilt_bytes[2], tilt_bytes[3],
                    0xFF,
                ])
            }
            PanTiltLimitCommand::Clear { corner } => {
                Ok(vec![
                    0x81, 0x01, 0x06, 0x07, 0x01, *corner as u8,
                    0x07, 0x0F, 0x0F, 0x0F,
                    0x07, 0x0F, 0x0F, 0x0F,
                    0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
