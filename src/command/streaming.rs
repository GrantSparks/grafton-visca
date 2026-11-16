//! Network and streaming commands for PtzOptics cameras
//!
//! This module contains vendor-specific commands for controlling network and streaming features
//! on PtzOptics Ndi cameras. These are not part of the baseline VISCA standard.

use crate::wire::WireValue;

use crate::{
    command::bytes::{constants::streaming, FrameWriter},
    macros::consolidated::visca_command,
    types::NdiQuality,
};

/// Enables or disables multicast streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MulticastStreamingCommand {
    enabled: bool,
}

impl MulticastStreamingCommand {
    /// Creates a multicast-streaming setting.
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
    /// Returns the requested multicast state.
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

impl crate::command::encode::WireEncode for MulticastStreamingCommand {
    fn write_into(
        &self,
        camera_id: crate::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&streaming::MULTICAST)
            // R1's multicast register uses 01=on, 02=off rather than the
            // standard VISCA 02/03 boolean convention.
            .byte(if self.enabled { 0x01 } else { 0x02 })
            .finish()
    }
}

visca_command! {
    /// Ndi streaming quality control command.
    #[derive(PartialEq, Eq)]
    pub struct SetNdiQuality { quality: NdiQuality };
    prefix = streaming::NDI_QUALITY;
    param = (*quality).to_wire();
    max_param_size = 1;
}

impl SetNdiQuality {
    /// Create a new Ndi quality command
    pub fn new(quality: NdiQuality) -> Self {
        Self { quality }
    }
}

crate::macros::consolidated::on_off_command! {
    /// Enables or disables USB audio output.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UsbAudioCommand {
        enabled: bool,
    };
    prefix = streaming::USB_AUDIO;
    convention = crate::wire::BoolConvention::OnIs02;
}

impl UsbAudioCommand {
    /// Creates a USB audio setting.
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
    /// Returns the requested USB audio state.
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MulticastStreamingCommand,
        test_multicast_on_encoding,
        MulticastStreamingCommand::new(true),
        &[0x81, 0x0B, 0x01, 0x23, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        MulticastStreamingCommand,
        test_multicast_off_encoding,
        MulticastStreamingCommand::new(false),
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

    visca_test!(
        UsbAudioCommand,
        test_usb_audio_on_encoding,
        UsbAudioCommand::new(true),
        &[0x81, 0x2A, 0x02, 0xA0, 0x04, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        UsbAudioCommand,
        test_usb_audio_off_encoding,
        UsbAudioCommand::new(false),
        &[0x81, 0x2A, 0x02, 0xA0, 0x04, 0x03, VISCA_TERMINATOR]
    );
}
