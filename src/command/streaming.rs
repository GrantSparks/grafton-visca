//! Network and streaming commands for PtzOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PtzOptics Ndi cameras. These are not part of the baseline VISCA standard.

use crate::{types::NdiQuality, visca_command};

visca_command! {
        /// Internal multicast streaming command
    pub struct MulticastStreamingInternal { enabled: bool };
    prefix = [0x0B, 0x01, 0x23];
    param = if *enabled { 0x01 } else { 0x02 };
    max_param_size = 1;
    category = crate::timeout::CommandCategory::Network;
}

/// Multicast streaming control for PtzOptics Ndi cameras
///
/// Vendor-specific command to enable or disable multicast video streaming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum MulticastStreaming {
    /// Enable multicast streaming
    On,
    /// Disable multicast streaming
    Off,
}

impl From<MulticastStreaming> for MulticastStreamingInternal {
    fn from(value: MulticastStreaming) -> Self {
        match value {
            MulticastStreaming::On => MulticastStreamingInternal { enabled: true },
            MulticastStreaming::Off => MulticastStreamingInternal { enabled: false },
        }
    }
}

impl crate::command::encode::ViscaCommand for MulticastStreaming {
    type Response = ();
    const MAX_SIZE: usize = 8; // Conservative estimate
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Network;

    fn write_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        let internal: MulticastStreamingInternal = (*self).into();
        internal.write_into(camera_id, buffer)
    }

    fn response_kind(&self) -> Option<crate::command::InquiryKind> {
        MulticastStreamingInternal { enabled: true }.response_kind()
    }
}

visca_command! {
        /// Internal Ndi quality command
    pub struct NdiQualityCommandInternal { quality: NdiQuality };
    prefix = [0x0B, 0x01, 0x01];
    param = match *quality {
        NdiQuality::High => 0x01,
        NdiQuality::Medium => 0x02,
        NdiQuality::Low => 0x03,
        NdiQuality::Off => 0x04,
    };
    max_param_size = 1;
    category = crate::timeout::CommandCategory::Network;
}

/// Ndi streaming quality control command
///
/// Vendor-specific command for PtzOptics Ndi cameras to set the Ndi stream bandwidth/quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetNdiQuality {
    /// The Ndi quality setting to apply
    pub quality: NdiQuality,
}

impl crate::command::encode::ViscaCommand for SetNdiQuality {
    type Response = ();
    const MAX_SIZE: usize = 8; // Conservative estimate
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Network;

    fn write_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        let internal = NdiQualityCommandInternal {
            quality: self.quality,
        };
        internal.write_into(camera_id, buffer)
    }

    fn response_kind(&self) -> Option<crate::command::InquiryKind> {
        NdiQualityCommandInternal {
            quality: self.quality,
        }
        .response_kind()
    }
}

impl SetNdiQuality {
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
        SetNdiQuality::new(NdiQuality::High),
        &[0x81, 0x0B, 0x01, 0x01, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_medium_encoding,
        SetNdiQuality::new(NdiQuality::Medium),
        &[0x81, 0x0B, 0x01, 0x01, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_low_encoding,
        SetNdiQuality::new(NdiQuality::Low),
        &[0x81, 0x0B, 0x01, 0x01, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        NdiQuality,
        test_ndi_quality_off_encoding,
        SetNdiQuality::new(NdiQuality::Off),
        &[0x81, 0x0B, 0x01, 0x01, 0x04, VISCA_TERMINATOR]
    );
}
