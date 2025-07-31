//! Blocking-specific implementations for the generic Camera.

use std::borrow::Cow;
use std::time::Duration;

use crate::{
    capabilities::Profile,
    command::{encode_visca::EncodeVisca, Response, ResponseType},
    error::Error,
    transport::UnifiedTransport,
};

use super::Camera;

impl<P, T> Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Send a command synchronously (blocking) - implementation.
    pub(crate) fn send_command_blocking_impl<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command: {framed_bytes:02X?}");

        // Send command using blocking transport
        self.transport.send_blocking(&framed_bytes)?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_response = self.wait_for_response_blocking(P::ACK_TIMEOUT)?;
                match ack_response {
                    Response::CmdAck => {
                        // Wait for completion
                        self.wait_for_response_blocking(P::COMPLETION_TIMEOUT)
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Unexpected response: {ack_response:?}"
                    )))),
                }
            }
            Some(expected_type) => {
                // Inquiry command - wait for specific response with type
                self.wait_for_response_with_type_blocking(expected_type, P::COMPLETION_TIMEOUT)
            }
        }
    }

    /// Wait for a response with timeout (blocking version)
    fn wait_for_response_blocking(&self, timeout: Duration) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {bytes:02X?}");

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // Parse using the generic Response parser
                    match Response::parse(&response_bytes) {
                        Ok(response) => return Ok(response),
                        Err(e) => {
                            log::warn!("Failed to parse response: {e:?}");
                            // Continue waiting for a valid response
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: timeout,
                            command: Cow::Borrowed("wait_for_response_blocking"),
                        });
                    }
                    // Continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Wait for a specific type of response with timeout (blocking version).
    fn wait_for_response_with_type_blocking(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {bytes:02X?}");

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // First try to parse as a regular response
                    match Response::parse(&response_bytes) {
                        Ok(Response::CmdAck) => {
                            // Skip ACK for inquiry commands and wait for the actual response
                            log::debug!("Skipping ACK response for inquiry command");
                            continue;
                        }
                        Ok(Response::Error(e)) => return Err(e),
                        Ok(Response::Completion) => {
                            // Unexpected completion for inquiry
                            return Err(Error::UnexpectedResponseType);
                        }
                        Ok(other) => {
                            // This shouldn't happen with parse() but handle it
                            return Ok(other);
                        }
                        Err(_) => {
                            // If regular parse fails, it might be an inquiry response
                            // Try parsing with the expected type
                            match Response::parse_with_type(&response_bytes, &expected_type) {
                                Ok(response) => return Ok(response),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: timeout,
                            command: Cow::Borrowed("wait_for_response_with_type_blocking"),
                        });
                    }
                    // If we haven't exceeded our timeout, continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }
}
