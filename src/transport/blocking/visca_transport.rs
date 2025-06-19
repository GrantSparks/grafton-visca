//! VISCA protocol transport wrapper.
//!
//! Handles VISCA protocol specifics like ACK/Completion responses,
//! sequence numbering, and response parsing.

use std::time::Duration;

use crate::{
    command::{
        response::{parse_response, Response},
        ResponseType,
    },
    error::Error,
    Command,
};

use super::Transport;

/// VISCA protocol constants.
const VISCA_TERMINATOR: u8 = 0xFF;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const ACK_TIMEOUT: Duration = Duration::from_millis(500);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

/// VISCA transport wrapper that handles protocol-specific logic.
///
/// This wraps any raw transport and adds VISCA protocol handling:
/// - Command formatting with sequence numbers
/// - ACK/Completion response handling
/// - Response parsing and validation
#[derive(Debug)]
pub struct ViscaTransport<T: Transport> {
    transport: T,
    sequence: u8,
}

impl<T: Transport> ViscaTransport<T> {
    /// Create a new VISCA transport wrapping a raw transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            sequence: 1,
        }
    }

    /// Get a reference to the underlying transport.
    pub fn inner(&self) -> &T {
        &self.transport
    }

    /// Get a mutable reference to the underlying transport.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Get the next sequence number (1-7, wrapping).
    fn next_sequence(&mut self) -> u8 {
        let seq = self.sequence;
        self.sequence = if seq >= 7 { 1 } else { seq + 1 };
        seq
    }

    /// Send a VISCA command and wait for the response.
    pub fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Get command bytes
        let mut cmd_bytes = command.to_bytes()?;

        // For VISCA over IP, we don't modify the camera address
        // The camera address should already be in the command bytes from to_bytes()

        // Ensure command ends with terminator
        if cmd_bytes.last() != Some(&VISCA_TERMINATOR) {
            cmd_bytes.push(VISCA_TERMINATOR);
        }

        log::debug!("Sending VISCA command: {:02X?}", cmd_bytes);

        // Send command
        self.transport.send(&cmd_bytes)?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack = self.wait_for_ack(ACK_TIMEOUT)?;
                match ack {
                    Response::Ack => {
                        // Now wait for completion
                        self.wait_for_completion(COMPLETION_TIMEOUT)
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    other => Ok(other),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for data response
                self.wait_for_inquiry(DEFAULT_TIMEOUT, &response_type)
            }
        }
    }

    /// Wait for ACK response.
    fn wait_for_ack(&mut self, timeout: Duration) -> Result<Response, Error> {
        let data = match self.transport.receive(timeout) {
            Ok(data) => data,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::TimedOut => {
                return Err(Error::Timeout);
            }
            Err(e) => return Err(e),
        };

        if data.is_empty() {
            return Err(Error::InvalidResponseLength);
        }

        log::debug!("Received VISCA response: {:02X?}", data);

        // Basic ACK/Completion/Error parsing
        if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
            match data[1] {
                0x40..=0x4F => Ok(Response::Ack),
                0x50..=0x5F => Ok(Response::Completion),
                0x60..=0x6F => {
                    // Error response - error code is in data[2]
                    if data.len() >= 4 {
                        Err(Error::from_code(data[2]))
                    } else {
                        Err(Error::InvalidResponseLength)
                    }
                }
                _ => Ok(Response::Unknown(data)),
            }
        } else {
            Err(Error::InvalidResponse {
                expected: "Valid VISCA response".to_string(),
                actual: data,
            })
        }
    }

    /// Wait for completion response.
    fn wait_for_completion(&mut self, timeout: Duration) -> Result<Response, Error> {
        self.wait_for_ack(timeout)
    }

    /// Wait for inquiry response with specific data type.
    fn wait_for_inquiry(
        &mut self,
        timeout: Duration,
        response_type: &ResponseType,
    ) -> Result<Response, Error> {
        let data = match self.transport.receive(timeout) {
            Ok(data) => data,
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::TimedOut => {
                return Err(Error::Timeout);
            }
            Err(e) => return Err(e),
        };

        if data.is_empty() {
            return Err(Error::InvalidResponseLength);
        }

        log::debug!("Received VISCA inquiry response: {:02X?}", data);

        // Parse the response with the specific type
        match parse_response(&data, response_type) {
            Ok(response) => Ok(response),
            Err(e) => {
                log::error!("Failed to parse VISCA response: {:?}", e);
                Err(Error::InvalidResponse {
                    expected: format!("Valid {:?} response", response_type),
                    actual: data,
                })
            }
        }
    }

    /// Send raw bytes (for testing or custom commands).
    pub fn send_raw(&mut self, data: &[u8]) -> Result<(), Error> {
        self.transport.send(data)
    }

    /// Receive raw bytes (for testing or custom commands).
    pub fn receive_raw(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
        self.transport.receive(timeout)
    }

    /// Check if the transport is connected.
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }
}
