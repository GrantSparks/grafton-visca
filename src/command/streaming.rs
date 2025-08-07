//! Network and streaming commands for PTZOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PTZOptics NDI cameras. These are not part of the baseline VISCA standard.

use crate::macros::internal::*;

use crate::types::NDIQuality;

visca_bool_command! {
    /// Internal multicast streaming command
    struct MulticastStreamingInternal {
        prefix: crate::command::const_encoding::constants::streaming::MULTICAST_PREFIX,
        on: 0x01,
        off: 0x02,
        address: 0x81,
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

impl crate::command::encode_visca::EncodeVisca for MulticastStreaming {
    type Response = ();
    const MAX_SIZE: usize = MulticastStreamingInternal::MAX_SIZE;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Network;

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
}

visca_param_command! {
    /// Internal NDI quality command
    struct NDIQualityCommandInternal {
        quality: NDIQuality,
    }
    prefix = crate::command::const_encoding::constants::streaming::NDI_QUALITY_PREFIX;
    param_byte = match quality {
        NDIQuality::High => 0x01,
        NDIQuality::Medium => 0x02,
        NDIQuality::Low => 0x03,
        NDIQuality::Off => 0x04,
    };
    timeout = Network;
    address = 0x81;
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

impl crate::command::encode_visca::EncodeVisca for NDIQualityCommand {
    type Response = ();
    const MAX_SIZE: usize = NDIQualityCommandInternal::MAX_SIZE;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Network;

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
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MulticastStreaming,
        test_multicast_on_encoding,
        MulticastStreaming::On,
        &[0x81, 0x0B, 0x01, 0x23, 0x01,  VISCA_TERMINATOR]
    );

    visca_test!(
        MulticastStreaming,
        test_multicast_off_encoding,
        MulticastStreaming::Off,
        &[0x81, 0x0B, 0x01, 0x23, 0x02,  VISCA_TERMINATOR]
    );

    visca_test!(
        NDIQualityCommand,
        test_ndi_quality_high_encoding,
        NDIQualityCommand::new(NDIQuality::High),
        &[0x81, 0x0B, 0x01, 0x01, 0x01,  VISCA_TERMINATOR]
    );

    visca_test!(
        NDIQualityCommand,
        test_ndi_quality_medium_encoding,
        NDIQualityCommand::new(NDIQuality::Medium),
        &[0x81, 0x0B, 0x01, 0x01, 0x02,  VISCA_TERMINATOR]
    );

    visca_test!(
        NDIQualityCommand,
        test_ndi_quality_low_encoding,
        NDIQualityCommand::new(NDIQuality::Low),
        &[0x81, 0x0B, 0x01, 0x01, 0x03,  VISCA_TERMINATOR]
    );

    visca_test!(
        NDIQualityCommand,
        test_ndi_quality_off_encoding,
        NDIQualityCommand::new(NDIQuality::Off),
        &[0x81, 0x0B, 0x01, 0x01, 0x04,  VISCA_TERMINATOR]
    );
}
