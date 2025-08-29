//! Network and streaming commands for PtzOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PtzOptics Ndi cameras. These are not part of the baseline VISCA standard.

use crate::{macros::internal::*, types::NdiQuality};

visca_bool_command! {
    /// Internal multicast streaming command
    struct MulticastStreamingInternal {
        prefix: crate::command::bytes::constants::streaming::MULTICAST_PREFIX,
        on: 0x01,
        off: 0x02,
        address: 0x81,
        response: None,
    }
}

/// Multicast streaming control for PtzOptics Ndi cameras
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

impl crate::command::encode_visca::ViscaEncode for MulticastStreaming {
    type ViscaResponse = ();
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

    fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
        MulticastStreamingInternal::new(true).response_type()
    }
}

visca_param_command! {
    /// Internal Ndi quality command
    struct NdiQualityCommandInternal {
        quality: NdiQuality,
    }
    prefix = crate::command::bytes::constants::streaming::NDI_QUALITY_PREFIX;
    param_byte = match quality {
        NdiQuality::High => 0x01,
        NdiQuality::Medium => 0x02,
        NdiQuality::Low => 0x03,
        NdiQuality::Off => 0x04,
    };
    timeout = Network;
    address = 0x81;
    response = None;
}

/// Ndi streaming quality control command
///
/// Vendor-specific command for PtzOptics Ndi cameras to set the Ndi stream bandwidth/quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NdiQualityCmd {
    /// The Ndi quality setting to apply
    pub quality: NdiQuality,
}

impl crate::command::encode_visca::ViscaEncode for NdiQualityCmd {
    type ViscaResponse = ();
    const MAX_SIZE: usize = NdiQualityCommandInternal::MAX_SIZE;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Network;

    fn encode_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        let internal = NdiQualityCommandInternal {
            quality: self.quality,
        };
        internal.encode_into(camera_id, buffer)
    }

    fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
        NdiQualityCommandInternal {
            quality: self.quality,
        }
        .response_type()
    }
}

impl NdiQualityCmd {
    /// Create a new Ndi quality command
    pub fn new(quality: NdiQuality) -> Self {
        Self { quality }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MulticastStreaming,
        test_multicast_on_encoding,
        MulticastStreaming::On,
        &[0x81, 0x0B, 0x01, 0x23, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        MulticastStreaming,
        test_multicast_off_encoding,
        MulticastStreaming::Off,
        &[0x81, 0x0B, 0x01, 0x23, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_high_encoding,
        NdiQualityCmd::new(NdiQuality::High),
        &[0x81, 0x0B, 0x01, 0x01, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_medium_encoding,
        NdiQualityCmd::new(NdiQuality::Medium),
        &[0x81, 0x0B, 0x01, 0x01, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_low_encoding,
        NdiQualityCmd::new(NdiQuality::Low),
        &[0x81, 0x0B, 0x01, 0x01, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_off_encoding,
        NdiQualityCmd::new(NdiQuality::Off),
        &[0x81, 0x0B, 0x01, 0x01, 0x04, VISCA_TERMINATOR]
    );
}
