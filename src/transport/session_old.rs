//! Unified VISCA transport session manager that works with both blocking and async transports.
//!
//! This module provides a single ViscaTransport type that can work with both
//! blocking Transport and async AsyncTransport implementations.

use crate::{
    command::response::{parse_response as parse_response_typed, Response, ResponseType},
    error::Error,
    types::SocketId,
    Command,
};

/// VISCA transport that handles protocol logic and manages an underlying transport.
///
/// This transport can work with both blocking and async transports, providing
/// a unified interface for VISCA protocol handling.
#[derive(Debug)]
pub struct ViscaTransport<T> {
    inner: T,
    pending_commands: [Option<PendingCommand>; 2],
}

#[derive(Debug, Clone, Copy)]
struct PendingCommand {
    response_type: Option<ResponseType>,
    acknowledged: bool,
}

// Common implementation for all transports
impl<T> ViscaTransport<T> {
    /// Create a new VISCA transport wrapping a transport.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            pending_commands: [None; 2],
        }
    }

    /// Get the inner transport.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get a reference to the underlying transport.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the underlying transport.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    // Private helper methods shared between blocking and async

    fn assign_socket(&mut self, response_type: Option<ResponseType>) -> Result<SocketId, Error> {
        // Try socket 0 first, then socket 1
        for (idx, slot) in self.pending_commands.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(PendingCommand {
                    response_type,
                    acknowledged: false,
                });
                let socket_id = if idx == 0 {
                    SocketId::SOCKET_0
                } else {
                    SocketId::SOCKET_1
                };
                log::debug!("Assigned {socket_id} for new command (type: {response_type:?})");
                return Ok(socket_id);
            }
        }
        log::debug!("Cannot assign socket - both sockets are in use");
        Err(Error::CommandBufferFull)
    }

    fn release_socket(&mut self, socket_id: SocketId) {
        let idx = socket_id.value() as usize;
        if idx < 2 {
            self.pending_commands[idx] = None;
            log::debug!("Released {socket_id}");
        }
    }

    fn process_response(&mut self, response: &[u8]) -> Result<Option<(SocketId, Response)>, Error> {
        // Validate basic response format
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(Error::InvalidResponseFormat);
        }

        let response_type = response[1] & 0xF0;
        let socket_raw = response[1] & 0x0F;
        let socket_id = SocketId::new(socket_raw)?;
        let idx = socket_id.value() as usize;

        match response_type {
            // ACK response
            0x40 => {
                if idx < 2 {
                    if let Some(pending) = &mut self.pending_commands[idx] {
                        pending.acknowledged = true;
                        log::debug!("ACK received for {socket_id}");
                        Ok(Some((socket_id, Response::Ack)))
                    } else {
                        log::error!("Received ACK for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received ACK for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Completion response
            0x50 => {
                if idx < 2 {
                    if let Some(pending) = &self.pending_commands[idx] {
                        if response.len() == 3 {
                            log::debug!("Completion received for {socket_id}");
                            Ok(Some((socket_id, Response::Completion)))
                        } else {
                            // Completion with data (inquiry response)
                            match pending.response_type {
                                None => {
                                    log::error!(
                                        "Received data response for non-inquiry command on {socket_id}"
                                    );
                                    Err(Error::UnexpectedResponseType)
                                }
                                Some(resp_type) => match parse_response_typed(response, &resp_type)
                                {
                                    Ok(parsed) => {
                                        log::debug!(
                                            "Inquiry response received for {socket_id}: {parsed:?}"
                                        );
                                        Ok(Some((socket_id, parsed)))
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse inquiry response: {e}");
                                        Err(e)
                                    }
                                },
                            }
                        }
                    } else {
                        log::error!("Received completion for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received completion for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Error response
            0x60 => {
                if response.len() >= 4 {
                    let error_code = response[2];
                    let error = Error::from_code(error_code);
                    log::error!("Error response for {socket_id}: {error}");
                    Ok(Some((socket_id, Response::Error(error))))
                } else {
                    Err(Error::InvalidResponseFormat)
                }
            }

            _ => {
                log::error!("Unknown response type: {:#02X}", response[1]);
                Err(Error::InvalidResponseFormat)
            }
        }
    }
}

// Blocking implementation
impl<T> ViscaTransport<T>
where
    T: crate::transport::blocking::Transport,
{
    /// Send a VISCA command and wait for the complete response (blocking).
    pub fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Assign socket
        let socket_id = self.assign_socket(command.response_type())?;

        // Send command
        if let Err(e) = self.send_with_socket(command, socket_id) {
            self.release_socket(socket_id);
            return Err(e);
        }

        // Wait for complete response
        let mut _received_ack = false;
        let timeout = std::time::Duration::from_secs(10); // Default timeout
        let final_response = loop {
            let response_data = match self.inner.receive(timeout) {
                Ok(data) => data,
                Err(e) => {
                    self.release_socket(socket_id);
                    return Err(e);
                }
            };

            match self.process_response(&response_data)? {
                Some((resp_socket_id, response)) if resp_socket_id == socket_id => {
                    match response {
                        Response::Ack => {
                            _received_ack = true;
                            // Continue waiting for completion
                        }
                        Response::Completion | Response::Error(_) => {
                            break Some(response);
                        }
                        // Inquiry responses with data
                        _ => {
                            break Some(response);
                        }
                    }
                }
                Some(_) => {
                    // Response for different socket, ignore
                    continue;
                }
                None => {
                    // Invalid response, ignore
                    continue;
                }
            }
        };

        self.release_socket(socket_id);
        final_response.ok_or(Error::InvalidResponse {
            expected: "Valid response".to_string(),
            actual: vec![],
        })
    }

    /// Get a description of the underlying transport.
    pub fn description(&self) -> &str {
        self.inner.description()
    }

    /// Check if the underlying transport is connected.
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn send_with_socket(
        &mut self,
        command: &dyn Command,
        socket_id: SocketId,
    ) -> Result<(), Error> {
        let bytes = command.to_bytes()?;

        // For VISCA over IP, socket management is done at protocol level
        // Don't modify the camera address byte

        log::debug!(
            "Sending command with socket {}: {bytes:02X?}",
            socket_id.value()
        );

        self.inner.send(&bytes)
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<T> ViscaTransport<T>
where
    T: crate::transport::AsyncTransport + std::fmt::Debug,
{
    /// Send a VISCA command and wait for the complete response (async).
    pub async fn send_command_async(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Assign socket
        let socket_id = self.assign_socket(command.response_type())?;

        // Send command
        if let Err(e) = self.send_with_socket_async(command, socket_id).await {
            self.release_socket(socket_id);
            return Err(e);
        }

        // Wait for complete response
        let mut _received_ack = false;
        let final_response = loop {
            let response_data = match self.inner.receive().await {
                Ok(data) => data,
                Err(e) => {
                    self.release_socket(socket_id);
                    return Err(e);
                }
            };

            match self.process_response(&response_data)? {
                Some((resp_socket_id, response)) if resp_socket_id == socket_id => {
                    match response {
                        Response::Ack => {
                            _received_ack = true;
                            // Continue waiting for completion
                        }
                        Response::Completion | Response::Error(_) => {
                            break Some(response);
                        }
                        // Inquiry responses with data
                        _ => {
                            break Some(response);
                        }
                    }
                }
                Some(_) => {
                    // Response for different socket, ignore
                    continue;
                }
                None => {
                    // Invalid response, ignore
                    continue;
                }
            }
        };

        self.release_socket(socket_id);
        final_response.ok_or(Error::InvalidResponse {
            expected: "Valid response".to_string(),
            actual: vec![],
        })
    }

    /// Get a description of the underlying transport.
    pub fn description_async(&self) -> String {
        format!("{:?}", self.inner)
    }

    /// Check if the underlying transport is connected.
    pub fn is_connected_async(&self) -> bool {
        true // AsyncTransport doesn't have an is_connected method, assume connected
    }

    async fn send_with_socket_async(
        &mut self,
        command: &dyn Command,
        socket_id: SocketId,
    ) -> Result<(), Error> {
        let bytes = command.to_bytes()?;

        // For VISCA over IP, socket management is done at protocol level
        // Don't modify the camera address byte

        log::debug!(
            "Sending command with socket {}: {bytes:02X?}",
            socket_id.value()
        );

        self.inner.send(&bytes).await
    }
}
