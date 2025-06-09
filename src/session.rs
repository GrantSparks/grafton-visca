use std::collections::HashMap;

use log::{debug, error};

use crate::{
    command::response::{parse_visca_response as parse_response_typed, Response},
    error::Error as ViscaError,
    types::SocketId,
    ViscaResponse, ViscaResponseType,
};

/// Parse a VISCA response, optionally with a specific expected type.
fn parse_visca_response(
    data: &[u8],
    response_type: Option<ViscaResponseType>,
) -> Result<ViscaResponse, ViscaError> {
    if data.len() < 3 || data[0] != 0x90 || data[data.len() - 1] != 0xFF {
        return Err(ViscaError::InvalidResponseFormat);
    }

    match data[1] {
        0x40..=0x4F => Ok(ViscaResponse::Ack),
        0x50..=0x5F => {
            if data.len() == 3 {
                Ok(ViscaResponse::Completion)
            } else if let Some(rtype) = response_type {
                parse_response_typed(data, &rtype)
            } else {
                // Return a generic inquiry response without parsing
                Ok(ViscaResponse::Unknown(data.to_vec()))
            }
        }
        0x60..=0x6F => {
            if data.len() >= 3 {
                Err(ViscaError::from_code(data[2]))
            } else {
                Err(ViscaError::InvalidResponseFormat)
            }
        }
        _ => Ok(ViscaResponse::Unknown(data.to_vec())),
    }
}

/// Represents a command that is currently being processed by the camera
#[derive(Debug, Clone, Copy)]
pub struct PendingCommand {
    /// The expected response type for inquiry commands
    pub response_type: Option<ViscaResponseType>,
    /// Whether we've received an ACK for this command
    pub acknowledged: bool,
}

/// Manages the state of VISCA commands and their responses
#[derive(Debug)]
pub struct Session {
    /// Maps socket IDs to pending commands
    pending_commands: HashMap<SocketId, PendingCommand>,
}

/// Deprecated type alias for backward compatibility
#[deprecated(since = "0.5.0", note = "Use `Session` instead")]
pub type ViscaSession = Session;

impl Session {
    /// Creates a new VISCA session with no pending commands.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_commands: HashMap::new(),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Assigns a socket to a new command.
    ///
    /// Returns the socket ID if successful.
    ///
    /// # Errors
    /// Returns `ViscaError::CommandBufferFull` if both sockets are already in use.
    pub fn assign_socket(
        &mut self,
        response_type: Option<ViscaResponseType>,
    ) -> Result<SocketId, ViscaError> {
        use std::collections::hash_map::Entry;

        // Try to find a free socket (0 or 1)
        for socket_id in [SocketId::SOCKET_0, SocketId::SOCKET_1] {
            if let Entry::Vacant(e) = self.pending_commands.entry(socket_id) {
                let _ = e.insert(PendingCommand {
                    response_type,
                    acknowledged: false,
                });
                debug!("Assigned {socket_id} for new command");
                return Ok(socket_id);
            }
        }

        // Both sockets are in use
        Err(ViscaError::CommandBufferFull)
    }

    /// Releases a socket after command completion
    pub fn release_socket(&mut self, socket_id: SocketId) {
        if self.pending_commands.remove(&socket_id).is_some() {
            debug!("Released {socket_id}");
        }
    }

    /// Processes a response frame and returns the parsed result
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidResponseFormat` if the response frame format is invalid.
    /// Returns `ViscaError::UnexpectedResponseType` if response data is received for a non-inquiry command.
    /// Returns parsing errors if the response payload cannot be parsed.
    pub fn process_response(
        &mut self,
        response: &[u8],
    ) -> Result<Option<(SocketId, ViscaResponse)>, ViscaError> {
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(ViscaError::InvalidResponseFormat);
        }

        match response[1] {
            // ACK response
            0x40..=0x4F => {
                let socket_raw = response[1] & 0x0F;
                let socket_id = SocketId::new(socket_raw)?;

                if let Some(pending) = self.pending_commands.get_mut(&socket_id) {
                    pending.acknowledged = true;
                    debug!("ACK received for {socket_id}");
                    Ok(Some((socket_id, ViscaResponse::Ack)))
                } else {
                    error!("Received ACK for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Completion or inquiry response
            0x50..=0x5F => {
                let socket_raw = response[1] & 0x0F;
                let socket_id = SocketId::new(socket_raw)?;

                self.pending_commands.get(&socket_id).map_or_else(
                    || {
                        error!("Received completion for unknown {socket_id}");
                        Ok(None)
                    },
                    |pending| {
                        if response.len() == 3 {
                            // Simple completion with no data
                            debug!("Completion received for {socket_id}");
                            Ok(Some((socket_id, Response::Completion)))
                        } else {
                            // Completion with data payload (inquiry response)
                            pending.response_type.map_or_else(
                                || {
                                    // Unexpected data response for non-inquiry command
                                    error!(
                                        "Received data response for non-inquiry command on {socket_id}"
                                    );
                                    Err(ViscaError::UnexpectedResponseType)
                                },
                                |response_type| match parse_response_typed(response, &response_type) {
                                    Ok(parsed) => {
                                        debug!(
                                            "Inquiry response received for {socket_id}: {parsed:?}"
                                        );
                                        Ok(Some((socket_id, parsed)))
                                    }
                                    Err(e) => {
                                        error!("Failed to parse inquiry response: {e}");
                                        Err(e)
                                    }
                                },
                            )
                        }
                    },
                )
            }

            // Error response
            0x60..=0x6F => {
                let socket_raw = response[1] & 0x0F;
                let socket_id = SocketId::new(socket_raw)?;

                if response.len() >= 4 {
                    let error_code = response[2];
                    let error = ViscaError::from_code(error_code);
                    error!("Error response for {socket_id}: {error}");
                    Ok(Some((socket_id, Response::Error(error))))
                } else {
                    Err(ViscaError::InvalidResponseFormat)
                }
            }

            _ => {
                error!("Unknown response type: {:#02X}", response[1]);
                Err(ViscaError::InvalidResponseFormat)
            }
        }
    }

    /// Clears all pending commands
    pub fn clear_all(&mut self) {
        self.pending_commands.clear();
        debug!("Cleared all pending commands");
    }

    /// Gets the count of currently pending commands
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_commands.len()
    }

    /// Marks a socket as acknowledged (ACK received).
    pub fn mark_acknowledged(&mut self, socket_id: SocketId) {
        if let Some(cmd) = self.pending_commands.get_mut(&socket_id) {
            cmd.acknowledged = true;
            debug!("{socket_id} acknowledged");
        }
    }

    /// Process a raw response and update session state.
    pub fn handle_response(&mut self, data: &[u8]) -> Response {
        // Parse the basic response structure
        let response = match parse_visca_response(data, None) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse response: {e}");
                return Response::Error(e);
            }
        };

        // Handle ACK - we need to extract socket ID from the raw data
        if matches!(&response, Response::Ack) {
            // ACK responses have format 0x90 0x4X 0xFF where X is the socket ID
            if data.len() >= 2 {
                let socket_raw = data[1] & 0x0F;
                if let Ok(socket_id) = SocketId::new(socket_raw) {
                    self.mark_acknowledged(socket_id);
                }
            }
        }

        // Handle completion or error responses - extract socket ID from raw data
        match &response {
            Response::Completion
            | Response::InquiryResponse(_)
            | Response::Error(_) => {
                // Completion and inquiry responses have format 0x90 0x5X ... 0xFF where X is the socket ID
                // Error responses have format 0x90 0x6X ... 0xFF where X is the socket ID
                if data.len() >= 2 {
                    let socket_raw = data[1] & 0x0F;
                    if let Ok(socket_id) = SocketId::new(socket_raw) {
                        self.release_socket(socket_id);
                    }
                }
            }
            _ => {}
        }

        // Check if this is an inquiry response we're expecting
        if let Response::InquiryResponse(_) = &response {
            if data.len() >= 2 {
                let socket_raw = data[1] & 0x0F;
                if let Ok(socket_id) = SocketId::new(socket_raw) {
                    // If we have a expected response type, try to parse with it
                    if let Some(expected_type) = self
                        .pending_commands
                        .get(&socket_id)
                        .and_then(|cmd| cmd.response_type)
                    {
                        match parse_response_typed(data, &expected_type) {
                            Ok(parsed) => return parsed,
                            Err(e) => {
                                error!("Failed to parse typed response: {e}");
                            }
                        }
                    }
                }
            }
        }

        response
    }

    /// Gets the expected response type for a socket.
    #[must_use]
    pub fn get_response_type(&self, socket_id: SocketId) -> Option<ViscaResponseType> {
        self.pending_commands
            .get(&socket_id)
            .and_then(|cmd| cmd.response_type)
    }

    /// Checks if a socket is currently in use.
    #[must_use]
    pub fn is_socket_busy(&self, socket_id: SocketId) -> bool {
        self.pending_commands.contains_key(&socket_id)
    }

    /// Gets information about a pending command on a socket.
    #[must_use]
    pub fn get_pending_command(&self, socket_id: SocketId) -> Option<&PendingCommand> {
        self.pending_commands.get(&socket_id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new();
        assert_eq!(session.pending_count(), 0);
    }

    #[test]
    fn test_socket_assignment() {
        let mut session = Session::new();

        // First command should get socket 0
        let socket1 = session.assign_socket(None).unwrap();
        assert_eq!(socket1, SocketId::SOCKET_0);

        // Second command should get socket 1
        let socket2 = session.assign_socket(None).unwrap();
        assert_eq!(socket2, SocketId::SOCKET_1);

        // Third command should fail
        let result = session.assign_socket(None);
        assert!(matches!(result, Err(ViscaError::CommandBufferFull)));
    }

    #[test]
    fn test_socket_release() {
        let mut session = Session::new();

        let socket = session.assign_socket(None).unwrap();
        assert_eq!(session.pending_count(), 1);

        session.release_socket(socket);
        assert_eq!(session.pending_count(), 0);

        // Should be able to assign again
        let new_socket = session.assign_socket(None).unwrap();
        assert_eq!(new_socket, SocketId::SOCKET_0);
    }

    #[test]
    fn test_ack_response_processing() {
        let mut session = Session::new();
        let socket = session.assign_socket(None).unwrap();

        // ACK response for socket 0
        let ack_response = [0x90, 0x40, 0xFF];
        let result = session.process_response(&ack_response).unwrap();

        assert!(result.is_some());
        let (response_socket, response) = result.unwrap();
        assert_eq!(response_socket, socket);
        assert!(matches!(response, Response::Ack));

        // Verify the socket is marked as acknowledged
        let pending = session.get_pending_command(socket).unwrap();
        assert!(pending.acknowledged);
    }

    #[test]
    fn test_completion_response_processing() {
        let mut session = Session::new();
        let socket = session.assign_socket(None).unwrap();

        // Completion response for socket 0
        let completion_response = [0x90, 0x50, 0xFF];
        let result = session.process_response(&completion_response).unwrap();

        assert!(result.is_some());
        let (response_socket, response) = result.unwrap();
        assert_eq!(response_socket, socket);
        assert!(matches!(response, Response::Completion));
    }

    #[test]
    fn test_error_response_processing() {
        let mut session = Session::new();
        let _ = session.assign_socket(None).unwrap();

        // Error response for socket 0 (syntax error)
        let error_response = [0x90, 0x60, 0x02, 0xFF];
        let result = session.process_response(&error_response).unwrap();

        assert!(result.is_some());
        let (_socket, response) = result.unwrap();
        assert!(matches!(response, Response::Error(_)));
    }

    #[test]
    fn test_invalid_response_format() {
        let mut session = Session::new();

        // Invalid start byte
        let invalid1 = [0x80, 0x50, 0xFF];
        assert!(matches!(
            session.process_response(&invalid1),
            Err(ViscaError::InvalidResponseFormat)
        ));

        // Missing terminator
        let invalid2 = [0x90, 0x50, 0x00];
        assert!(matches!(
            session.process_response(&invalid2),
            Err(ViscaError::InvalidResponseFormat)
        ));

        // Too short
        let invalid3 = [0x90, 0xFF];
        assert!(matches!(
            session.process_response(&invalid3),
            Err(ViscaError::InvalidResponseFormat)
        ));
    }

    #[test]
    fn test_clear_all() {
        let mut session = Session::new();

        let _ = session.assign_socket(None).unwrap();
        let _ = session.assign_socket(None).unwrap();
        assert_eq!(session.pending_count(), 2);

        session.clear_all();
        assert_eq!(session.pending_count(), 0);
    }
}
