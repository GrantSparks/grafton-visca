//! Network and streaming commands for PTZOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PTZOptics NDI cameras. These are not part of the baseline VISCA standard.

use crate::{
    capabilities::{CameraFeature, CommandFeatures},
    types::NDIQuality,
    visca_bool_command, visca_param_command,
};

visca_bool_command! {
    /// Internal multicast streaming command
    struct MulticastStreamingInternal {
        prefix: [0x80, 0x0B, 0x01, 0x23],
        on: 0x01,
        off: 0x02,
        address: 0x80,
        response: None,
    }
}

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

impl From<MulticastStreaming> for MulticastStreamingInternal {
    fn from(value: MulticastStreaming) -> Self {
        match value {
            MulticastStreaming::On => MulticastStreamingInternal::new(true),
            MulticastStreaming::Off => MulticastStreamingInternal::new(false),
        }
    }
}

impl CommandFeatures for MulticastStreamingInternal {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

impl crate::command::encode_visca::EncodeVisca for MulticastStreaming {
    type Response = ();
    const MAX_SIZE: usize = MulticastStreamingInternal::MAX_SIZE;

    fn encode_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        let internal: MulticastStreamingInternal = (*self).into();
        internal.encode_into(camera_id, buffer)
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        MulticastStreamingInternal::new(true).response_type()
    }

    fn timeout_kind(&self) -> crate::timeout::CommandCategory {
        MulticastStreamingInternal::new(true).timeout_kind()
    }
}

impl CommandFeatures for MulticastStreaming {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

visca_param_command! {
    /// Internal NDI quality command
    struct NDIQualityCommandInternal {
        quality: NDIQuality,
    }
    prefix = [0x80, 0x0B, 0x01, 0x01];
    param_byte = match quality {
        NDIQuality::High => 0x01,
        NDIQuality::Medium => 0x02,
        NDIQuality::Low => 0x03,
        NDIQuality::Off => 0x04,
    };
    timeout = Network;
    address = 0x80;
    response = None;
}

/// NDI streaming quality control command
///
/// Vendor-specific command for PTZOptics NDI cameras to set the NDI stream bandwidth/quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NDIQualityCommand {
    /// The NDI quality setting to apply
    pub quality: NDIQuality,
}

impl CommandFeatures for NDIQualityCommandInternal {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

impl crate::command::encode_visca::EncodeVisca for NDIQualityCommand {
    type Response = ();
    const MAX_SIZE: usize = NDIQualityCommandInternal::MAX_SIZE;

    fn encode_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        let internal = NDIQualityCommandInternal {
            quality: self.quality,
        };
        internal.encode_into(camera_id, buffer)
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        NDIQualityCommandInternal {
            quality: self.quality,
        }
        .response_type()
    }

    fn timeout_kind(&self) -> crate::timeout::CommandCategory {
        NDIQualityCommandInternal {
            quality: self.quality,
        }
        .timeout_kind()
    }
}

impl CommandFeatures for NDIQualityCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDI]
    }
}

impl NDIQualityCommand {
    /// Create a new NDI quality command
    pub fn new(quality: NDIQuality) -> Self {
        Self { quality }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::{command::encode_visca::EncodeVisca, CameraId};

    #[test]
    fn test_multicast_on_encoding() {
        let cmd = MulticastStreaming::On;
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x23, 0x01, 0xFF]);
    }

    #[test]
    fn test_multicast_off_encoding() {
        let cmd = MulticastStreaming::Off;
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x23, 0x02, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_high_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::High);
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x01, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_medium_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Medium);
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x02, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_low_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Low);
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x03, 0xFF]);
    }

    #[test]
    fn test_ndi_quality_off_encoding() {
        let cmd = NDIQualityCommand::new(NDIQuality::Off);
        let mut buffer = [0u8; 6];
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encode should succeed with sufficient buffer");
        assert_eq!(len, 6);
        assert_eq!(&buffer[..len], &[0x81, 0x0B, 0x01, 0x01, 0x04, 0xFF]);
    }
}
