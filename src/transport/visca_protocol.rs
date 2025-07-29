//! VISCA protocol handler using GAT Transport trait.

use crate::{
    command::{encode_visca::EncodeVisca, InquiryResponse, Response, ResponseType},
    transport::core::Transport,
    Error,
};
use std::time::Duration;

/// VISCA protocol constants.
// Removed duplicate - use from const_encoding module
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const ACK_TIMEOUT: Duration = Duration::from_millis(500);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

/// VISCA protocol handler that manages protocol-specific logic.
///
/// This wraps any Transport implementation and adds VISCA protocol handling:
/// - Command formatting and termination
/// - ACK/Completion response handling
/// - Response parsing and validation
/// - Timeout management
#[derive(Debug)]
pub struct ViscaProtocol<T: Transport> {
    transport: T,
}

impl<T: Transport> ViscaProtocol<T> {
    /// Create a new VISCA protocol handler wrapping a transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Get a reference to the underlying transport.
    pub fn inner(&self) -> &T {
        &self.transport
    }

    /// Send a VISCA command and return a future that resolves to the response.
    pub async fn send_command<'a, C>(&'a self, command: &'a C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
                                    // Need camera_id - this is a design issue. ViscaProtocol should have camera_id
                                    // For now, use default Camera 1
        let size = command.encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)?;
        let cmd_bytes = &buffer[..size];

        log::debug!("Sending VISCA command: {cmd_bytes:02X?}");

        // Send command
        self.transport.send(cmd_bytes).await.map_err(Into::into)?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack = self.wait_for_ack(ACK_TIMEOUT).await?;
                match ack {
                    Response::CmdAck => {
                        // Now wait for completion
                        self.wait_for_completion(COMPLETION_TIMEOUT).await
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    _ => Err(Error::ParseError(format!("{ack:?}"))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response
                self.wait_for_response(response_type, DEFAULT_TIMEOUT).await
            }
        }
    }

    /// Wait for an ACK response.
    async fn wait_for_ack(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        Response::parse(&bytes)
    }

    /// Wait for a completion response.
    async fn wait_for_completion(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        let response = Response::parse(&bytes)?;

        match response {
            Response::Completion => Ok(response),
            Response::Error(e) => Err(e),
            _ => Err(Error::ParseError(format!("{response:?}"))),
        }
    }

    /// Wait for a specific type of response.
    async fn wait_for_response(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        let response = Response::parse(&bytes)?;

        // Verify we got the expected response type
        if response.matches_type(expected_type) {
            Ok(response)
        } else {
            Err(Error::ParseError(format!(
                "Expected {expected_type:?}, got {response:?}"
            )))
        }
    }

    /// Receive with timeout handling.
    async fn recv_with_timeout(&self, duration: Duration) -> Result<bytes::Bytes, Error> {
        #[cfg(feature = "tokio")]
        {
            tokio::time::timeout(duration, self.transport.recv())
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(Into::into)
        }

        #[cfg(all(feature = "async", not(feature = "tokio")))]
        {
            // For runtime-agnostic async, we can't implement timeout internally.
            // Users should wrap the entire send_command operation with their runtime's timeout.
            // We document this limitation and provide the duration for informational purposes.
            log::debug!("Timeout of {:?} requested, but no runtime-specific timeout available. Users should wrap operations with their runtime's timeout mechanism.", duration);
            self.transport.recv().await.map_err(Into::into)
        }

        #[cfg(not(feature = "async"))]
        {
            // This shouldn't be reachable in blocking mode as ViscaProtocol is async-only
            let _ = duration;
            self.transport.recv().await.map_err(Into::into)
        }
    }
}

// Helper to check if a Response matches a ResponseType
impl Response {
    fn matches_type(&self, expected: ResponseType) -> bool {
        matches!(
            (self, expected),
            (
                Response::Inquiry(InquiryResponse::Power { .. }),
                ResponseType::Power
            ) | (
                Response::Inquiry(InquiryResponse::PanTiltPosition { .. }),
                ResponseType::PanTiltPosition,
            ) | (
                Response::Inquiry(InquiryResponse::ZoomPosition { .. }),
                ResponseType::ZoomPosition,
            ) | (
                Response::Inquiry(InquiryResponse::FocusPosition { .. }),
                ResponseType::FocusPosition,
            ) | (
                Response::Inquiry(InquiryResponse::FocusNearLimit { .. }),
                ResponseType::FocusNearLimit,
            ) | (
                Response::Inquiry(InquiryResponse::FocusZone { .. }),
                ResponseType::FocusZone,
            ) | (
                Response::Inquiry(InquiryResponse::AutoFocusSensitivity { .. }),
                ResponseType::AutoFocusSensitivity,
            ) | (
                Response::Inquiry(InquiryResponse::ExposureMode { .. }),
                ResponseType::ExposureMode,
            ) | (
                Response::Inquiry(InquiryResponse::ExposureCompensationMode { .. }),
                ResponseType::ExposureCompensationMode,
            ) | (
                Response::Inquiry(InquiryResponse::ExposureCompensation { .. }),
                ResponseType::ExposureCompensation,
            ) | (
                Response::Inquiry(InquiryResponse::Iris { .. }),
                ResponseType::Iris
            ) | (
                Response::Inquiry(InquiryResponse::Shutter { .. }),
                ResponseType::Shutter
            ) | (
                Response::Inquiry(InquiryResponse::Bright { .. }),
                ResponseType::Bright,
            ) | (
                Response::Inquiry(InquiryResponse::GainLevel { .. }),
                ResponseType::Gain
            ) | (
                Response::Inquiry(InquiryResponse::GainLimit { .. }),
                ResponseType::GainLimit,
            ) | (
                Response::Inquiry(InquiryResponse::Backlight { .. }),
                ResponseType::Backlight,
            ) | (
                Response::Inquiry(InquiryResponse::DynamicRange { .. }),
                ResponseType::DynamicRange,
            ) | (
                Response::Inquiry(InquiryResponse::WhiteBalanceMode { .. }),
                ResponseType::WhiteBalanceMode,
            ) | (
                Response::Inquiry(InquiryResponse::ColorTemperature { .. }),
                ResponseType::ColorTemperature,
            ) | (
                Response::Inquiry(InquiryResponse::RedChannel { .. }),
                ResponseType::RedChannel,
            ) | (
                Response::Inquiry(InquiryResponse::BlueChannel { .. }),
                ResponseType::BlueChannel,
            ) | (
                Response::Inquiry(InquiryResponse::Luminance { .. }),
                ResponseType::Luminance,
            ) | (
                Response::Inquiry(InquiryResponse::Contrast { .. }),
                ResponseType::Contrast,
            ) | (
                Response::Inquiry(InquiryResponse::Sharpness { .. }),
                ResponseType::Sharpness,
            ) | (
                Response::Inquiry(InquiryResponse::SharpnessMode { .. }),
                ResponseType::SharpnessMode,
            ) | (
                Response::Inquiry(InquiryResponse::Saturation { .. }),
                ResponseType::Saturation,
            ) | (
                Response::Inquiry(InquiryResponse::Hue { .. }),
                ResponseType::Hue,
            ) | (
                Response::Inquiry(InquiryResponse::NoiseReduction2D { .. }),
                ResponseType::NoiseReduction2D,
            ) | (
                Response::Inquiry(InquiryResponse::NoiseReduction3D { .. }),
                ResponseType::NoiseReduction3D,
            ) | (
                Response::Inquiry(InquiryResponse::ImageFlip { .. }),
                ResponseType::ImageFlip,
            ) | (
                Response::Inquiry(InquiryResponse::BlackWhite { .. }),
                ResponseType::BlackWhite,
            ) | (
                Response::Inquiry(InquiryResponse::Version { .. }),
                ResponseType::Version,
            ) | (
                Response::Inquiry(InquiryResponse::TallyRed { .. }),
                ResponseType::TallyRed,
            ) | (
                Response::Inquiry(InquiryResponse::TallyGreen { .. }),
                ResponseType::TallyGreen,
            ) | (
                Response::Inquiry(InquiryResponse::FocusMode { .. }),
                ResponseType::FocusMode,
            )
        )
    }
}
