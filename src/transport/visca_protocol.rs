//! VISCA protocol handler using GAT Transport trait.

use crate::{
    command::{InquiryResponse, Response, ResponseType},
    transport::gat_transport::Transport,
    Command, Error,
};
use core::future::Future;
use std::time::Duration;

/// VISCA protocol constants.
const VISCA_TERMINATOR: u8 = 0xFF;
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
    pub fn send_command<'a>(
        &'a self,
        command: &'a dyn Command,
    ) -> impl Future<Output = Result<Response, Error>> + 'a {
        async move {
            // Get command bytes
            let mut cmd_bytes = command.to_bytes()?;

            // Ensure command ends with terminator
            if cmd_bytes.last() != Some(&VISCA_TERMINATOR) {
                cmd_bytes.push(VISCA_TERMINATOR);
            }

            log::debug!("Sending VISCA command: {:02X?}", cmd_bytes);

            // Send command
            self.transport.send(&cmd_bytes).await.map_err(Into::into)?;

            // Handle response based on command type
            match command.response_type() {
                None => {
                    // Action command - wait for ACK then Completion
                    let ack = self.wait_for_ack(ACK_TIMEOUT).await?;
                    match ack {
                        Response::Ack => {
                            // Now wait for completion
                            self.wait_for_completion(COMPLETION_TIMEOUT).await
                        }
                        Response::Completion => {
                            // Some cameras send completion directly
                            Ok(Response::Completion)
                        }
                        _ => Err(Error::ParseError(format!("{:?}", ack))),
                    }
                }
                Some(response_type) => {
                    // Inquiry command - wait for specific response
                    self.wait_for_response(response_type, DEFAULT_TIMEOUT).await
                }
            }
        }
    }

    /// Wait for an ACK response.
    async fn wait_for_ack(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        Response::parse(&bytes.to_vec())
    }

    /// Wait for a completion response.
    async fn wait_for_completion(&self, timeout: Duration) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        let response = Response::parse(&bytes.to_vec())?;

        match response {
            Response::Completion => Ok(response),
            Response::Error(e) => Err(e),
            _ => Err(Error::ParseError(format!("{:?}", response))),
        }
    }

    /// Wait for a specific type of response.
    async fn wait_for_response(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let bytes = self.recv_with_timeout(timeout).await?;
        let response = Response::parse(&bytes.to_vec())?;

        // Verify we got the expected response type
        if response.matches_type(expected_type) {
            Ok(response)
        } else {
            Err(Error::ParseError(format!(
                "Expected {:?}, got {:?}",
                expected_type, response
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

        #[cfg(not(feature = "tokio"))]
        {
            // For blocking transports, timeout is handled in the transport itself
            let _ = duration; // Unused in blocking mode
            self.transport.recv().await.map_err(Into::into)
        }
    }
}

// Helper to check if a Response matches a ResponseType
impl Response {
    fn matches_type(&self, expected: ResponseType) -> bool {
        match (self, expected) {
            (
                Response::InquiryResponse(InquiryResponse::ZoomPosition { .. }),
                ResponseType::ZoomPosition,
            ) => true,
            (
                Response::InquiryResponse(InquiryResponse::FocusPosition { .. }),
                ResponseType::FocusPosition,
            ) => true,
            (
                Response::InquiryResponse(InquiryResponse::PanTiltPosition { .. }),
                ResponseType::PanTiltPosition,
            ) => true,
            (Response::InquiryResponse(InquiryResponse::Power { .. }), ResponseType::Power) => true,
            (
                Response::InquiryResponse(InquiryResponse::WhiteBalance { .. }),
                ResponseType::WhiteBalanceMode,
            ) => true,
            (
                Response::InquiryResponse(InquiryResponse::ExposureMode { .. }),
                ResponseType::ExposureMode,
            ) => true,
            (Response::InquiryResponse(InquiryResponse::Iris { .. }), ResponseType::Iris) => true,
            (Response::InquiryResponse(InquiryResponse::Gain { .. }), ResponseType::Gain) => true,
            (Response::InquiryResponse(InquiryResponse::Shutter { .. }), ResponseType::Shutter) => {
                true
            }
            _ => false,
        }
    }
}
