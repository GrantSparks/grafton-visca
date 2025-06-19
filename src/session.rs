//! VISCA session management for tracking command-response state.
//!
//! The Session module provides stateful tracking of VISCA commands and their responses.
//! VISCA cameras support up to 2 concurrent commands through separate "sockets" (command slots),
//! and this module manages the allocation and tracking of these resources.

use std::collections::HashMap;

use log::{debug, error};

use crate::{
    command::response::{parse_response as parse_response_typed, Response, ResponseType},
    error::Error,
    types::SocketId,
};

/// Represents a command that is currently being processed by the camera.
///
/// VISCA commands follow a two-phase response pattern:
/// 1. ACK - Acknowledges receipt of the command
/// 2. Completion - Indicates command execution finished (with optional data for inquiries)
#[derive(Debug, Clone, Copy)]
pub struct PendingCommand {
    /// The expected response type for inquiry commands.
    /// This is `None` for control commands and `Some(ResponseType)` for inquiry commands.
    pub response_type: Option<ResponseType>,
    /// Whether we've received an ACK for this command.
    /// Used to track the two-phase response pattern.
    pub acknowledged: bool,
}

/// Manages the state of VISCA commands and their responses.
///
/// The Session tracks which commands are pending on which sockets and matches
/// incoming responses to their corresponding commands. It does not handle any
/// network communication - that responsibility belongs to the Transport layer.
///
/// # Example Flow
/// ```ignore
/// // 1. Assign a socket for a new command
/// let socket_id = session.assign_socket(None)?;
///
/// // 2. Send command via transport (handled externally)
/// 
/// // 3. Process responses as they arrive
/// match session.process_response(&response_bytes)? {
///     Some((socket, Response::Ack)) => { /* Command acknowledged */ }
///     Some((socket, Response::Completion)) => { /* Command completed */ }
///     Some((socket, Response::Error(e))) => { /* Command failed */ }
///     None => { /* Response for unknown command */ }
/// }
///
/// // 4. Release the socket when done
/// session.release_socket(socket_id);
/// ```
#[derive(Debug)]
pub struct Session {
    /// Maps socket IDs to pending commands.
    /// VISCA supports exactly 2 concurrent commands (sockets 0 and 1).
    pending_commands: HashMap<SocketId, PendingCommand>,
}

impl Session {
    /// Creates a new VISCA session with no pending commands.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_commands: HashMap::new(),
        }
    }

    /// Returns the number of currently pending commands.
    ///
    /// This will be between 0 and 2, as VISCA supports a maximum of 2 concurrent commands.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_commands.len()
    }

    /// Checks if a specific socket has a pending command.
    #[must_use]
    #[allow(dead_code)]
    pub fn has_pending(&self, socket_id: SocketId) -> bool {
        self.pending_commands.contains_key(&socket_id)
    }

    /// Checks if all sockets are currently in use.
    #[must_use]
    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        self.pending_commands.len() >= 2
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
    /// This method finds an available socket (0 or 1) and reserves it for a new command.
    /// For inquiry commands, the expected response type should be provided to enable
    /// proper response parsing.
    ///
    /// # Arguments
    /// * `response_type` - The expected response type for inquiry commands, or `None` for control commands
    ///
    /// # Returns
    /// The assigned socket ID if successful.
    ///
    /// # Errors
    /// Returns `Error::CommandBufferFull` if both sockets are already in use.
    pub fn assign_socket(
        &mut self,
        response_type: Option<ResponseType>,
    ) -> Result<SocketId, Error> {
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
        Err(Error::CommandBufferFull)
    }

    /// Releases a socket after command completion.
    ///
    /// This should be called when a command has completed (successfully or with error)
    /// to free up the socket for future commands.
    ///
    /// # Arguments
    /// * `socket_id` - The socket to release
    pub fn release_socket(&mut self, socket_id: SocketId) {
        if self.pending_commands.remove(&socket_id).is_some() {
            debug!("Released {socket_id}");
        }
    }

    /// Processes a VISCA response frame and returns the parsed result.
    ///
    /// This method handles all types of VISCA responses:
    /// - ACK (0x4X) - Command acknowledgment
    /// - Completion (0x5X) - Command completion with optional data
    /// - Error (0x6X) - Command error with error code
    ///
    /// The response is matched to pending commands based on the socket ID encoded
    /// in the response. For inquiry commands with data, the response is parsed
    /// according to the expected response type.
    ///
    /// # Arguments
    /// * `response` - The raw response bytes from the camera
    ///
    /// # Returns
    /// - `Ok(Some((socket_id, response)))` - Successfully processed response
    /// - `Ok(None)` - Response for unknown command (already completed/released)
    /// - `Err(_)` - Invalid response format or parsing error
    ///
    /// # Errors
    /// - `Error::InvalidResponseFormat` - Response frame format is invalid
    /// - `Error::UnexpectedResponseType` - Data received for non-inquiry command
    /// - `Error::InvalidSocketId` - Socket ID in response is invalid
    /// - Parsing errors from response data parsing
    pub fn process_response(
        &mut self,
        response: &[u8],
    ) -> Result<Option<(SocketId, Response)>, Error> {
        // Validate basic response format: [0x90, TYPE|SOCKET, ...data..., 0xFF]
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(Error::InvalidResponseFormat);
        }

        let response_type = response[1] & 0xF0;
        let socket_raw = response[1] & 0x0F;
        let socket_id = SocketId::new(socket_raw)?;

        match response_type {
            // ACK response (0x40)
            0x40 => self.handle_ack(socket_id),

            // Completion response (0x50)
            0x50 => self.handle_completion(socket_id, response),

            // Error response (0x60)
            0x60 => self.handle_error(socket_id, response),

            _ => {
                error!("Unknown response type: {:#02X}", response[1]);
                Err(Error::InvalidResponseFormat)
            }
        }
    }

    /// Handles ACK response processing
    fn handle_ack(&mut self, socket_id: SocketId) -> Result<Option<(SocketId, Response)>, Error> {
        if let Some(pending) = self.pending_commands.get_mut(&socket_id) {
            pending.acknowledged = true;
            debug!("ACK received for {socket_id}");
            Ok(Some((socket_id, Response::Ack)))
        } else {
            error!("Received ACK for unknown {socket_id}");
            Ok(None)
        }
    }

    /// Handles completion response processing
    fn handle_completion(
        &self,
        socket_id: SocketId,
        response: &[u8],
    ) -> Result<Option<(SocketId, Response)>, Error> {
        let Some(pending) = self.pending_commands.get(&socket_id) else {
            error!("Received completion for unknown {socket_id}");
            return Ok(None);
        };

        if response.len() == 3 {
            // Simple completion with no data
            debug!("Completion received for {socket_id}");
            Ok(Some((socket_id, Response::Completion)))
        } else {
            // Completion with data payload (inquiry response)
            match pending.response_type {
                None => {
                    error!("Received data response for non-inquiry command on {socket_id}");
                    Err(Error::UnexpectedResponseType)
                }
                Some(response_type) => match parse_response_typed(response, &response_type) {
                    Ok(parsed) => {
                        debug!("Inquiry response received for {socket_id}: {parsed:?}");
                        Ok(Some((socket_id, parsed)))
                    }
                    Err(e) => {
                        error!("Failed to parse inquiry response: {e}");
                        Err(e)
                    }
                },
            }
        }
    }

    /// Handles error response processing
    fn handle_error(
        &self,
        socket_id: SocketId,
        response: &[u8],
    ) -> Result<Option<(SocketId, Response)>, Error> {
        if response.len() >= 4 {
            let error_code = response[2];
            let error = Error::from_code(error_code);
            error!("Error response for {socket_id}: {error}");
            Ok(Some((socket_id, Response::Error(error))))
        } else {
            Err(Error::InvalidResponseFormat)
        }
    }

    // Test helper methods
    #[cfg(test)]
    fn get_pending_command(&self, socket: SocketId) -> Option<&PendingCommand> {
        self.pending_commands.get(&socket)
    }

    #[cfg(test)]
    fn clear_all(&mut self) {
        self.pending_commands.clear();
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
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
        let socket1 = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(socket1, SocketId::SOCKET_0);

        // Second command should get socket 1
        let socket2 = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(socket2, SocketId::SOCKET_1);

        // Third command should fail
        let result = session.assign_socket(None);
        assert!(matches!(result, Err(Error::CommandBufferFull)));
    }

    #[test]
    fn test_socket_release() {
        let mut session = Session::new();

        let socket = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(session.pending_count(), 1);

        session.release_socket(socket);
        assert_eq!(session.pending_count(), 0);

        // Should be able to assign again
        let new_socket = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(new_socket, SocketId::SOCKET_0);
    }

    #[test]
    fn test_ack_response_processing() {
        let mut session = Session::new();
        let socket = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        // ACK response for socket 0
        let ack_response = [0x90, 0x40, 0xFF];
        let result = session
            .process_response(&ack_response)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        assert!(result.is_some());
        let (response_socket, response) =
            result.unwrap_or_else(|| panic!("Expected Some response from ACK processing"));
        assert_eq!(response_socket, socket);
        assert!(matches!(response, Response::Ack));

        // Verify the socket is marked as acknowledged
        let pending = session
            .get_pending_command(socket)
            .unwrap_or_else(|| panic!("Expected pending command for socket"));
        assert!(pending.acknowledged);
    }

    #[test]
    fn test_completion_response_processing() {
        let mut session = Session::new();
        let socket = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        // Completion response for socket 0
        let completion_response = [0x90, 0x50, 0xFF];
        let result = session
            .process_response(&completion_response)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        assert!(result.is_some());
        let (response_socket, response) =
            result.unwrap_or_else(|| panic!("Expected Some response from ACK processing"));
        assert_eq!(response_socket, socket);
        assert!(matches!(response, Response::Completion));
    }

    #[test]
    fn test_error_response_processing() {
        let mut session = Session::new();
        let _ = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        // Error response for socket 0 (syntax error)
        let error_response = [0x90, 0x60, 0x02, 0xFF];
        let result = session
            .process_response(&error_response)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

        assert!(result.is_some());
        let (_socket, response) =
            result.unwrap_or_else(|| panic!("Expected Some response from error processing"));
        assert!(matches!(response, Response::Error(_)));
    }

    #[test]
    fn test_invalid_response_format() {
        let mut session = Session::new();

        // Invalid start byte
        let invalid1 = [0x80, 0x50, 0xFF];
        assert!(matches!(
            session.process_response(&invalid1),
            Err(Error::InvalidResponseFormat)
        ));

        // Missing terminator
        let invalid2 = [0x90, 0x50, 0x00];
        assert!(matches!(
            session.process_response(&invalid2),
            Err(Error::InvalidResponseFormat)
        ));

        // Too short
        let invalid3 = [0x90, 0xFF];
        assert!(matches!(
            session.process_response(&invalid3),
            Err(Error::InvalidResponseFormat)
        ));
    }

    #[test]
    fn test_clear_all() {
        let mut session = Session::new();

        let _ = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let _ = session
            .assign_socket(None)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(session.pending_count(), 2);

        session.clear_all();
        assert_eq!(session.pending_count(), 0);
    }
}
