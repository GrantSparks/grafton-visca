//! VISCA protocol handler for both async and blocking transports.

use crate::{
    command::{encode_visca::EncodeVisca, InquiryResponse, Response, ResponseType},
    timeout::TimeoutConfig,
    Error,
};
use std::borrow::Cow;
use std::time::Duration;

#[cfg(feature = "async")]
use crate::{runtime::SharedRuntime, transport::AsyncTransport};

use crate::transport::BlockingTransport;

// Note: ACK timeout is now configured via TimeoutConfig.ack_timeout

/// VISCA protocol handler that manages protocol-specific logic.
///
/// This wraps any Transport implementation and adds VISCA protocol handling:
/// - Command formatting and termination
/// - ACK/Completion response handling
/// - Response parsing and validation
/// - Timeout management based on command categories
pub struct ViscaProtocol<T> {
    transport: T,
    timeout_config: TimeoutConfig,
    #[cfg(feature = "async")]
    runtime: SharedRuntime,
}

impl<T> std::fmt::Debug for ViscaProtocol<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("ViscaProtocol");
        debug.field("transport", &"<Transport>");
        debug.field("timeout_config", &self.timeout_config);
        #[cfg(feature = "async")]
        debug.field("runtime", &"SharedRuntime");
        debug.finish()
    }
}

// Common methods for both async and blocking
impl<T> ViscaProtocol<T> {
    /// Get a reference to the underlying transport.
    pub fn inner(&self) -> &T {
        &self.transport
    }

    /// Get the current timeout configuration.
    pub const fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set a new timeout configuration.
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<T: AsyncTransport> ViscaProtocol<T> {
    /// Create a new VISCA protocol handler wrapping an async transport.
    /// Requires a runtime for timeout operations.
    pub fn new(transport: T, runtime: SharedRuntime) -> Self {
        Self {
            transport,
            timeout_config: TimeoutConfig::default(),
            runtime,
        }
    }

    /// Create a new VISCA protocol handler with custom timeout configuration.
    pub fn new_with_timeout_config(
        transport: T,
        runtime: SharedRuntime,
        timeout_config: TimeoutConfig,
    ) -> Self {
        Self {
            transport,
            timeout_config,
            runtime,
        }
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

        // Validate commands have proper terminator (critical safety invariant)
        assert!(
            size == 0 || cmd_bytes[size - 1] == crate::command::const_encoding::VISCA_TERMINATOR,
            "VISCA command missing 0xFF terminator. Command bytes: {:02X?}",
            cmd_bytes
        );

        // Get the appropriate timeout for this command category
        let command_timeout = self.timeout_config.get_timeout(command.timeout_kind());

        log::debug!(
            "Sending VISCA command (category: {:?}, timeout: {:?}): {:02X?}",
            command.timeout_kind(),
            command_timeout,
            cmd_bytes
        );

        // Send command using AsyncTransport
        self.transport.send(cmd_bytes).await?;

        // Handle response based on command type following VISCA protocol spec:
        // - Commands (action): Controller → Camera: 8x 01... FF
        //                     Camera → Controller: ACK (9x 4y FF) then Completion (9x 5y FF)
        // - Inquiries:        Controller → Camera: 8x 09... FF
        //                     Camera → Controller: Data Reply (9x 50 <data> FF) - no ACK
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                // ACK should come quickly (within ack_timeout), but completion may take much longer
                let ack = self.wait_for_ack(self.timeout_config.ack_timeout).await?;
                match ack {
                    Response::CmdAck => {
                        // ACK received, command accepted into socket
                        // Now wait for completion using command-specific timeout
                        // This timeout is generous to handle long operations like full-range movements
                        self.wait_for_completion(command_timeout).await
                    }
                    Response::Completion => {
                        // Some cameras send completion directly without ACK
                        Ok(Response::Completion)
                    }
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Expected ACK or Completion, got: {ack:?}"
                    )))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response with command-specific timeout
                // Inquiries don't send ACK, just the data reply directly
                self.wait_for_response(response_type, command_timeout).await
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
            _ => Err(Error::ParseError(Cow::Owned(format!("{response:?}")))),
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
            Err(Error::ParseError(Cow::Owned(format!(
                "Expected {expected_type:?}, got {response:?}"
            ))))
        }
    }

    /// Receive with timeout handling.
    async fn recv_with_timeout(&self, duration: Duration) -> Result<bytes::Bytes, Error> {
        let runtime = &self.runtime;

        // Use the provided Runtime for timeout with AsyncTransport
        crate::runtime::timeout_with_runtime(runtime.as_ref(), duration, self.transport.recv())
            .await?
    }
}

// Blocking implementation
impl<T: BlockingTransport> ViscaProtocol<T> {
    /// Create a new VISCA protocol handler wrapping a blocking transport with default timeout config.
    #[cfg(not(feature = "async"))]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            timeout_config: TimeoutConfig::default(),
        }
    }

    /// Create a new VISCA protocol handler with custom timeout configuration.
    #[cfg(not(feature = "async"))]
    pub fn new_with_timeout_config(transport: T, timeout_config: TimeoutConfig) -> Self {
        Self {
            transport,
            timeout_config,
        }
    }

    /// Send a VISCA command and return the response (blocking).
    pub fn send_command_blocking<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
                                    // Need camera_id - this is a design issue. ViscaProtocol should have camera_id
                                    // For now, use default Camera 1
        let size = command.encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)?;
        let cmd_bytes = &buffer[..size];

        // Validate commands have proper terminator (critical safety invariant)
        assert!(
            size == 0 || cmd_bytes[size - 1] == crate::command::const_encoding::VISCA_TERMINATOR,
            "VISCA command missing 0xFF terminator. Command bytes: {:02X?}",
            cmd_bytes
        );

        // Get the appropriate timeout for this command category
        let command_timeout = self.timeout_config.get_timeout(command.timeout_kind());

        log::debug!(
            "Sending VISCA command (category: {:?}, timeout: {:?}): {:02X?}",
            command.timeout_kind(),
            command_timeout,
            cmd_bytes
        );

        // Send command using BlockingTransport
        self.transport.send_blocking(cmd_bytes)?;

        // Handle response based on command type following VISCA protocol spec:
        // - Commands (action): Controller → Camera: 8x 01... FF
        //                     Camera → Controller: ACK (9x 4y FF) then Completion (9x 5y FF)
        // - Inquiries:        Controller → Camera: 8x 09... FF
        //                     Camera → Controller: Data Reply (9x 50 <data> FF) - no ACK
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                // ACK should come quickly (within ack_timeout), but completion may take much longer
                let ack = self.wait_for_ack_blocking(self.timeout_config.ack_timeout)?;
                match ack {
                    Response::CmdAck => {
                        // ACK received, command accepted into socket
                        // Now wait for completion using command-specific timeout
                        // This timeout is generous to handle long operations like full-range movements
                        self.wait_for_completion_blocking(command_timeout)
                    }
                    Response::Completion => {
                        // Some cameras send completion directly without ACK
                        Ok(Response::Completion)
                    }
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Expected ACK or Completion, got: {ack:?}"
                    )))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response with command-specific timeout
                // Inquiries don't send ACK, just the data reply directly
                self.wait_for_response_blocking(response_type, command_timeout)
            }
        }
    }

    /// Wait for an ACK response (blocking).
    fn wait_for_ack_blocking(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.transport.recv_blocking_with_timeout(timeout)?;
        Response::parse(&bytes)
    }

    /// Wait for a completion response (blocking).
    fn wait_for_completion_blocking(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.transport.recv_blocking_with_timeout(timeout)?;
        let response = Response::parse(&bytes)?;

        match response {
            Response::Completion => Ok(response),
            Response::Error(e) => Err(e),
            _ => Err(Error::ParseError(Cow::Owned(format!("{response:?}")))),
        }
    }

    /// Wait for a specific type of response (blocking).
    fn wait_for_response_blocking(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let bytes = self.transport.recv_blocking_with_timeout(timeout)?;
        let response = Response::parse(&bytes)?;

        // Verify we got the expected response type
        if response.matches_type(expected_type) {
            Ok(response)
        } else {
            Err(Error::ParseError(Cow::Owned(format!(
                "Expected {expected_type:?}, got {response:?}"
            ))))
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
