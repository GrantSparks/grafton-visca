//! VISCA transport session manager.
//!
//! Handles all VISCA protocol logic including socket management, response parsing,
//! and command lifecycle management.

#[cfg(feature = "async")]
use crate::{
    command::response::{parse_response as parse_response_typed, Response, ResponseType},
    error::Error,
    types::SocketId,
    Command,
};

#[cfg(feature = "async")]
use super::RawTransport;

#[cfg(feature = "async")]
/// VISCA transport that handles protocol logic and manages an underlying raw transport.
///
/// This is the main transport type that users interact with. It wraps a RawTransport
/// and handles all VISCA-specific concerns like socket management and response parsing.
#[derive(Debug)]
pub struct ViscaTransport<T: RawTransport> {
    raw: T,
    pending_commands: [Option<PendingCommand>; 2],
}

#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy)]
struct PendingCommand {
    response_type: Option<ResponseType>,
    acknowledged: bool,
}

#[cfg(feature = "async")]
impl<T: RawTransport> ViscaTransport<T> {
    /// Create a new VISCA transport wrapping a raw transport.
    pub fn new(raw: T) -> Self {
        Self {
            raw,
            pending_commands: [None; 2],
        }
    }

    /// Send a VISCA command and wait for the complete response.
    ///
    /// Handles the full command lifecycle:
    /// 1. Assigns an available socket (0 or 1)
    /// 2. Sends command with proper socket encoding
    /// 3. Waits for ACK and completion responses
    /// 4. Returns the final response
    pub async fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Assign socket
        let socket_id = self.assign_socket(command.response_type())?;

        // Send command
        if let Err(e) = self.send_with_socket(command, socket_id).await {
            self.release_socket(socket_id);
            return Err(e);
        }

        // Wait for complete response
        let mut _received_ack = false;
        let final_response = loop {
            let response_data = match self.raw.receive().await {
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
        self.raw.description()
    }

    /// Check if the underlying transport is connected.
    pub fn is_connected(&self) -> bool {
        self.raw.is_connected()
    }

    /// Get a reference to the underlying raw transport.
    pub fn raw(&self) -> &T {
        &self.raw
    }

    /// Get a mutable reference to the underlying raw transport.
    pub fn raw_mut(&mut self) -> &mut T {
        &mut self.raw
    }

    // Private helper methods

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

    async fn send_with_socket(
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

        self.raw.send(&bytes).await
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
