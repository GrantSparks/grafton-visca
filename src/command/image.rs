use crate::{command::ViscaCommand, error::ViscaError, timeout::CommandCategory, ViscaResponseType};

pub struct BacklightCommand {
    pub status: bool,
}

impl ViscaCommand for BacklightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let status_byte = if self.status { 0x02 } else { 0x03 };
        Ok(vec![0x81, 0x01, 0x04, 0x33, status_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// 2D Noise Reduction command
#[derive(Debug, Copy, Clone)]
pub enum NoiseReduction2DCommand {
    Off,
    Level(u8), // 1-5
}

impl ViscaCommand for NoiseReduction2DCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            NoiseReduction2DCommand::Off => vec![0x81, 0x01, 0x04, 0x53, 0x00, 0xFF],
            NoiseReduction2DCommand::Level(level) => {
                if *level < 1 || *level > 5 {
                    return Err(ViscaError::InvalidParameter(
                        "2D Noise Reduction level must be between 1 and 5".into(),
                    ));
                }
                vec![0x81, 0x01, 0x04, 0x53, *level, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// 3D Noise Reduction command
#[derive(Debug, Copy, Clone)]
pub enum NoiseReduction3DCommand {
    Off,
    Level(u8), // 1-8
}

impl ViscaCommand for NoiseReduction3DCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            NoiseReduction3DCommand::Off => vec![0x81, 0x01, 0x04, 0x54, 0x00, 0xFF],
            NoiseReduction3DCommand::Level(level) => {
                if *level < 1 || *level > 8 {
                    return Err(ViscaError::InvalidParameter(
                        "3D Noise Reduction level must be between 1 and 8".into(),
                    ));
                }
                vec![0x81, 0x01, 0x04, 0x54, *level, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// Black and White Mode command
#[derive(Debug, Copy, Clone)]
pub struct BlackWhiteCommand {
    pub on: bool,
}

impl ViscaCommand for BlackWhiteCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let mode = if self.on { 0x04 } else { 0x00 };
        Ok(vec![0x81, 0x01, 0x04, 0x01, mode, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// Combined Image Flip command (Horizontal + Vertical)
#[derive(Debug, Copy, Clone)]
pub enum ImageFlipMode {
    Off,
    Horizontal,
    Vertical,
    Both,
}

pub struct ImageFlipCombinedCommand {
    pub mode: ImageFlipMode,
}

impl ViscaCommand for ImageFlipCombinedCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let mode_byte = match self.mode {
            ImageFlipMode::Off => 0x00,
            ImageFlipMode::Horizontal => 0x01,
            ImageFlipMode::Vertical => 0x02,
            ImageFlipMode::Both => 0x03,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x61, mode_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}
