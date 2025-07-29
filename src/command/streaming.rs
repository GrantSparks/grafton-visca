//! Network and streaming commands for PTZOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PTZOptics NDI cameras. These are not part of the baseline VISCA standard.

use crate::{
    camera_id::CameraId,
    capabilities::{CameraFeature, CommandFeatures},
    command::{encode_visca::EncodeVisca, response::ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::NDIQuality,
};

/// Multicast streaming control for PTZOptics NDI cameras
///
/// Vendor-specific command to enable or disable multicast video streaming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MulticastStreaming {
    /// Enable multicast streaming
    On,
    /// Disable multicast streaming
    Off,
}

impl From<MulticastStreaming> for bool {
    fn from(value: MulticastStreaming) -> Self {
        matches!(value, MulticastStreaming::On)
    }
}

impl EncodeVisca for MulticastStreaming {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
        &self,
        camera_id: CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        // Commands use 0x0B prefix (extended command space)
        // Format: 81 0B 01 23 0p FF where p=1 for On, p=2 for Off
        let mode = match self {
            MulticastStreaming::On => 0x01,
            MulticastStreaming::Off => 0x02,
        };
        
        buffer[0] = 0x80 | camera_id.id();
        buffer[1] = 0x0B;
        buffer[2] = 0x01;
        buffer[3] = 0x23;
        buffer[4] = mode;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Standard ACK/Completion response
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Network
    }
}

impl CommandFeatures for MulticastStreaming {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

/// NDI streaming quality control command
///
/// Vendor-specific command for PTZOptics NDI cameras to set the NDI stream bandwidth/quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NDIQualityCommand {
    /// The NDI quality setting to apply
    pub quality: NDIQuality,
}

impl NDIQualityCommand {
    /// Create a new NDI quality command
    pub fn new(quality: NDIQuality) -> Self {
        Self { quality }
    }
}

impl EncodeVisca for NDIQualityCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
        &self,
        camera_id: CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        // Format: 81 0B 01 01 0p FF
        // where p=1 (High), 2 (Medium), 3 (Low), 4 (Off)
        let quality_value = match self.quality {
            NDIQuality::High => 0x01,
            NDIQuality::Medium => 0x02,
            NDIQuality::Low => 0x03,
            NDIQuality::Off => 0x04,
        };
        
        buffer[0] = 0x80 | camera_id.id();
        buffer[1] = 0x0B;
        buffer[2] = 0x01;
        buffer[3] = 0x01;
        buffer[4] = quality_value;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Standard ACK/Completion response
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Network
    }
}

impl CommandFeatures for NDIQualityCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multicast_on_encoding() {
        let cmd = MulticastStreaming::On;
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x23, 0x01, 0xFF]);
    }

    #[test]
    fn test_multicast_off_encoding() {
        let cmd = MulticastStreaming::Off;
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x23, 0x02, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_high_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::High);
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x01, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_medium_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Medium);
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x02, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_low_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Low);
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x03, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_off_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Off);
        let mut buffer = [0u8; 6];
        let len = cmd.encode_into(CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x04, 0xFF]);
    }
}