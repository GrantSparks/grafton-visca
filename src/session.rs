// Standard library imports
use std::collections::HashMap;

// Third-party imports
use log::{debug, error};

// Crate imports
use crate::{
    command::response::parse_visca_response, ViscaError, ViscaResponse, ViscaResponseType,
};

/// Represents a command that is currently being processed by the camera
#[derive(Debug)]
pub struct PendingCommand {
    /// The expected response type for inquiry commands
    pub response_type: Option<ViscaResponseType>,
    /// Whether we've received an ACK for this command
    pub acknowledged: bool,
}

/// Manages the state of VISCA commands and their responses
pub struct ViscaSession {
    /// Maps socket IDs (0 or 1) to pending commands
    pending_commands: HashMap<u8, PendingCommand>,
}

impl ViscaSession {
    pub fn new() -> Self {
        Self {
            pending_commands: HashMap::new(),
        }
    }
}

impl Default for ViscaSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ViscaSession {
    /// Assigns a socket to a new command.
    ///
    /// Returns the socket ID (0 or 1) if successful.
    pub fn assign_socket(
        &mut self,
        response_type: Option<ViscaResponseType>,
    ) -> Result<u8, ViscaError> {
        use std::collections::hash_map::Entry;

        // Try to find a free socket (0 or 1)
        for socket_id in 0..=1 {
            if let Entry::Vacant(e) = self.pending_commands.entry(socket_id) {
                e.insert(PendingCommand {
                    response_type,
                    acknowledged: false,
                });
                debug!("Assigned socket {socket_id} for new command");
                return Ok(socket_id);
            }
        }

        // Both sockets are in use
        Err(ViscaError::CommandBufferFull)
    }

    /// Releases a socket after command completion
    pub fn release_socket(&mut self, socket_id: u8) {
        if self.pending_commands.remove(&socket_id).is_some() {
            debug!("Released socket {socket_id}");
        }
    }

    /// Processes a response frame and returns the parsed result
    pub fn process_response(
        &mut self,
        response: &[u8],
    ) -> Result<Option<(u8, ViscaResponse)>, ViscaError> {
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(ViscaError::InvalidResponseFormat);
        }

        match response[1] {
            // ACK response
            0x40..=0x4F => {
                let socket_id = response[1] & 0x0F;

                if let Some(pending) = self.pending_commands.get_mut(&socket_id) {
                    pending.acknowledged = true;
                    debug!("ACK received for socket {socket_id}");
                    Ok(Some((socket_id, ViscaResponse::Ack)))
                } else {
                    error!("Received ACK for unknown socket {socket_id}");
                    Ok(None)
                }
            }

            // Completion or inquiry response
            0x50..=0x5F => {
                let socket_id = response[1] & 0x0F;

                self.pending_commands.get(&socket_id).map_or_else(
                    || {
                        error!("Received completion for unknown socket {socket_id}");
                        Ok(None)
                    },
                    |pending| {
                        if response.len() == 3 {
                            // Simple completion with no data
                            debug!("Completion received for socket {socket_id}");
                            Ok(Some((socket_id, ViscaResponse::Completion)))
                        } else {
                            // Completion with data payload (inquiry response)
                            pending.response_type.map_or_else(
                                || {
                                    // Unexpected data response for non-inquiry command
                                    error!(
                                        "Received data response for non-inquiry command on socket {socket_id}"
                                    );
                                    Err(ViscaError::UnexpectedResponseType)
                                },
                                |response_type| match parse_visca_response(response, &response_type) {
                                    Ok(parsed) => {
                                        debug!(
                                            "Inquiry response received for socket {socket_id}: {parsed:?}"
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
                let socket_id = response[1] & 0x0F;
                let error_code = if response.len() > 2 {
                    response[2]
                } else {
                    0xFF
                };
                let error = ViscaError::from_code(error_code);

                error!("Error response on socket {socket_id}: {error:?}");
                Ok(Some((socket_id, ViscaResponse::Error(error))))
            }

            _ => {
                error!("Unknown response type: {response:02X?}");
                Ok(Some((0xFF, ViscaResponse::Unknown(response.to_vec()))))
            }
        }
    }

    /// Checks if a socket has been acknowledged
    pub fn is_acknowledged(&self, socket_id: u8) -> bool {
        self.pending_commands
            .get(&socket_id)
            .is_some_and(|cmd| cmd.acknowledged)
    }

    /// Gets the number of pending commands
    pub fn pending_count(&self) -> usize {
        self.pending_commands.len()
    }

    /// Checks if all sockets are in use
    pub fn is_full(&self) -> bool {
        self.pending_commands.len() >= 2
    }

    /// Checks if a socket is complete (not in use)
    pub fn is_complete(&self, socket_id: u8) -> bool {
        !self.pending_commands.contains_key(&socket_id)
    }

    /// Gets a list of currently pending socket IDs
    pub fn get_pending_sockets(&self) -> Vec<u8> {
        self.pending_commands.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_assignment() {
        let mut session = ViscaSession::new();

        // Should assign socket 0 first
        assert_eq!(session.assign_socket(None).unwrap(), 0);
        assert_eq!(session.pending_count(), 1);

        // Should assign socket 1 next
        assert_eq!(session.assign_socket(None).unwrap(), 1);
        assert_eq!(session.pending_count(), 2);
        assert!(session.is_full());

        // Should fail when both sockets are in use
        assert!(matches!(
            session.assign_socket(None),
            Err(ViscaError::CommandBufferFull)
        ));

        // Release socket 0 and try again
        session.release_socket(0);
        assert_eq!(session.pending_count(), 1);
        assert!(!session.is_full());
        assert_eq!(session.assign_socket(None).unwrap(), 0);
    }

    #[test]
    fn test_ack_processing() {
        let mut session = ViscaSession::new();
        session.assign_socket(None).unwrap();

        // Process ACK for socket 0
        let ack_response = vec![0x90, 0x40, 0xFF];
        let result = session.process_response(&ack_response).unwrap();

        assert!(result.is_some());
        let (socket_id, response) = result.unwrap();
        assert_eq!(socket_id, 0);
        assert!(matches!(response, ViscaResponse::Ack));
        assert!(session.is_acknowledged(0));
    }

    #[test]
    fn test_completion_processing() {
        let mut session = ViscaSession::new();
        session.assign_socket(None).unwrap();

        // Process completion for socket 0
        let completion_response = vec![0x90, 0x50, 0xFF];
        let result = session.process_response(&completion_response).unwrap();

        assert!(result.is_some());
        let (socket_id, response) = result.unwrap();
        assert_eq!(socket_id, 0);
        assert!(matches!(response, ViscaResponse::Completion));
    }

    #[test]
    fn test_error_processing() {
        let mut session = ViscaSession::new();
        session.assign_socket(None).unwrap();

        // Process syntax error
        let error_response = vec![0x90, 0x60, 0x02, 0xFF];
        let result = session.process_response(&error_response).unwrap();

        assert!(result.is_some());
        let (socket_id, response) = result.unwrap();
        assert_eq!(socket_id, 0);
        assert!(matches!(
            response,
            ViscaResponse::Error(ViscaError::SyntaxError)
        ));
    }

    #[test]
    fn test_overlapping_commands() {
        let mut session = ViscaSession::new();

        // Send two commands
        let socket_a = session.assign_socket(None).unwrap();
        let socket_b = session.assign_socket(None).unwrap();
        assert_ne!(socket_a, socket_b);

        // Process ACK for socket B first
        let ack_b = vec![0x90, 0x40 | socket_b, 0xFF];
        let result_b = session.process_response(&ack_b).unwrap().unwrap();
        assert_eq!(result_b.0, socket_b);
        assert!(matches!(result_b.1, ViscaResponse::Ack));

        // Process ACK for socket A
        let ack_a = vec![0x90, 0x40 | socket_a, 0xFF];
        let result_a = session.process_response(&ack_a).unwrap().unwrap();
        assert_eq!(result_a.0, socket_a);
        assert!(matches!(result_a.1, ViscaResponse::Ack));

        // Process completion for socket B
        let completion_b = vec![0x90, 0x50 | socket_b, 0xFF];
        let result_b = session.process_response(&completion_b).unwrap().unwrap();
        assert_eq!(result_b.0, socket_b);
        assert!(matches!(result_b.1, ViscaResponse::Completion));

        // Process completion for socket A
        let completion_a = vec![0x90, 0x50 | socket_a, 0xFF];
        let result_a = session.process_response(&completion_a).unwrap().unwrap();
        assert_eq!(result_a.0, socket_a);
        assert!(matches!(result_a.1, ViscaResponse::Completion));
    }

    #[test]
    fn test_release_socket() {
        let mut session = ViscaSession::new();

        // Assign both sockets
        let socket_0 = session.assign_socket(None).unwrap();
        let socket_1 = session.assign_socket(None).unwrap();
        assert_eq!(socket_0, 0);
        assert_eq!(socket_1, 1);
        assert!(session.is_full());

        // Release a non-existent socket (should do nothing)
        session.release_socket(2);
        assert!(session.is_full());

        // Release socket 0
        session.release_socket(0);
        assert!(!session.is_full());
        assert_eq!(session.pending_count(), 1);

        // Should be able to assign socket 0 again
        let new_socket = session.assign_socket(None).unwrap();
        assert_eq!(new_socket, 0);
    }

    #[test]
    fn test_is_complete() {
        let mut session = ViscaSession::new();

        // No pending commands should be complete
        assert!(session.is_complete(0));
        assert!(session.is_complete(1));

        // Assign a socket
        session.assign_socket(None).unwrap();
        assert!(!session.is_complete(0));
        assert!(session.is_complete(1)); // Socket 1 not assigned

        // Release socket 0
        session.release_socket(0);
        assert!(session.is_complete(0));
    }

    #[test]
    fn test_get_pending_sockets() {
        let mut session = ViscaSession::new();

        // Initially no pending sockets
        assert_eq!(session.get_pending_sockets(), vec![]);

        // Assign socket 0
        session.assign_socket(None).unwrap();
        assert_eq!(session.get_pending_sockets(), vec![0]);

        // Assign socket 1
        session.assign_socket(None).unwrap();
        let mut pending = session.get_pending_sockets();
        pending.sort_unstable();
        assert_eq!(pending, vec![0, 1]);

        // Release socket 0
        session.release_socket(0);
        assert_eq!(session.get_pending_sockets(), vec![1]);
    }

    #[test]
    fn test_response_for_unknown_socket() {
        let mut session = ViscaSession::new();

        // Process response for a socket that was never assigned
        let response = vec![0x90, 0x50, 0xFF]; // Completion for socket 0
        let result = session.process_response(&response).unwrap();

        // Should return None since socket 0 was never assigned
        assert!(result.is_none());
    }

    #[test]
    fn test_inquiry_response_processing() {
        let mut session = ViscaSession::new();

        // Assign socket with inquiry response type
        session
            .assign_socket(Some(ViscaResponseType::ZoomPosition))
            .unwrap();

        // Process inquiry response
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x04, 0xFF];
        let result = session.process_response(&response).unwrap();

        assert!(result.is_some());
        let (socket_id, response) = result.unwrap();
        assert_eq!(socket_id, 0);

        if let ViscaResponse::InquiryResponse(inquiry) = response {
            assert!(matches!(
                inquiry,
                crate::command::ViscaInquiryResponse::ZoomPosition { .. }
            ));
        } else {
            panic!("Expected inquiry response");
        }
    }

    #[test]
    fn test_invalid_response_format() {
        let mut session = ViscaSession::new();
        session.assign_socket(None).unwrap();

        // Invalid response (doesn't start with 0x90)
        let invalid_response = vec![0x80, 0x50, 0xFF];
        let result = session.process_response(&invalid_response);

        assert!(matches!(result, Err(ViscaError::InvalidResponseFormat)));

        // Empty response
        let empty_response = vec![];
        let result = session.process_response(&empty_response);

        assert!(matches!(result, Err(ViscaError::InvalidResponseFormat)));
    }
}
