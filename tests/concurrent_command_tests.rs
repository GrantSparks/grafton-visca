use grafton_visca::command::{PanTiltCommand, ZoomCommand};
use grafton_visca::{ViscaCommand, ViscaError, ViscaResponse, ViscaSession, ViscaTransport};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Mock transport that can simulate responses for multiple commands
struct ConcurrentMockTransport {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ConcurrentMockTransport {
    fn new(responses: Vec<Vec<u8>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            sent_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_sent_commands(&self) -> Vec<Vec<u8>> {
        self.sent_commands.lock().unwrap().clone()
    }
}

impl ViscaTransport for ConcurrentMockTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        let bytes = command.to_bytes()?;
        self.sent_commands.lock().unwrap().push(bytes);
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut responses = self.responses.lock().unwrap();
        let mut batch = Vec::new();

        // Return up to 2 responses at a time to simulate batched responses
        for _ in 0..2 {
            if let Some(response) = responses.pop_front() {
                batch.push(response);
            } else {
                break;
            }
        }

        if batch.is_empty() {
            Err(ViscaError::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "No more responses",
            )))
        } else {
            Ok(batch)
        }
    }
}

#[test]
fn test_session_socket_management() {
    let mut session = ViscaSession::new();

    // Should be able to send two commands
    let socket1 = session.assign_socket(None).unwrap();
    let socket2 = session.assign_socket(None).unwrap();

    assert_ne!(socket1, socket2);
    assert!(session.is_full());

    // Third command should fail
    let result = session.assign_socket(None);
    assert!(matches!(result, Err(ViscaError::CommandBufferFull)));

    // After releasing one socket, should be able to send another
    session.release_socket(socket1);
    assert!(!session.is_full());

    let socket3 = session.assign_socket(None).unwrap();
    assert_eq!(socket3, socket1); // Should reuse the freed socket
}

#[test]
fn test_interleaved_responses() {
    let mut session = ViscaSession::new();

    // Assign two sockets
    let socket_a = session.assign_socket(None).unwrap();
    let socket_b = session.assign_socket(None).unwrap();

    // Simulate interleaved ACKs and completions
    let responses = vec![
        vec![0x90, 0x40 | socket_b, 0xFF], // ACK for B
        vec![0x90, 0x40 | socket_a, 0xFF], // ACK for A
        vec![0x90, 0x50 | socket_b, 0xFF], // Completion for B
        vec![0x90, 0x50 | socket_a, 0xFF], // Completion for A
    ];

    // Process all responses and verify correct socket assignment
    for (i, response) in responses.iter().enumerate() {
        let result = session.process_response(response).unwrap();
        assert!(result.is_some());

        let (socket_id, parsed) = result.unwrap();

        match i {
            0 => {
                assert_eq!(socket_id, socket_b);
                assert!(matches!(parsed, ViscaResponse::Ack));
                assert!(session.is_acknowledged(socket_b));
            }
            1 => {
                assert_eq!(socket_id, socket_a);
                assert!(matches!(parsed, ViscaResponse::Ack));
                assert!(session.is_acknowledged(socket_a));
            }
            2 => {
                assert_eq!(socket_id, socket_b);
                assert!(matches!(parsed, ViscaResponse::Completion));
            }
            3 => {
                assert_eq!(socket_id, socket_a);
                assert!(matches!(parsed, ViscaResponse::Completion));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_error_on_one_socket_doesnt_affect_other() {
    let mut session = ViscaSession::new();

    // Assign two sockets
    let socket_a = session.assign_socket(None).unwrap();
    let socket_b = session.assign_socket(None).unwrap();

    // Socket A gets an error, socket B completes successfully
    let responses = vec![
        vec![0x90, 0x40 | socket_a, 0xFF],       // ACK for A
        vec![0x90, 0x40 | socket_b, 0xFF],       // ACK for B
        vec![0x90, 0x60 | socket_a, 0x41, 0xFF], // Error for A (Not Executable)
        vec![0x90, 0x50 | socket_b, 0xFF],       // Completion for B
    ];

    let mut results = Vec::new();
    for response in responses {
        if let Ok(Some(result)) = session.process_response(&response) {
            results.push(result);
        }
    }

    assert_eq!(results.len(), 4);

    // Verify socket A got error
    let (socket, response) = &results[2];
    assert_eq!(*socket, socket_a);
    assert!(matches!(
        response,
        ViscaResponse::Error(ViscaError::CommandNotExecutable)
    ));

    // Verify socket B completed successfully
    let (socket, response) = &results[3];
    assert_eq!(*socket, socket_b);
    assert!(matches!(response, ViscaResponse::Completion));
}

#[test]
fn test_response_for_unknown_socket_ignored() {
    let mut session = ViscaSession::new();

    // Only assign socket 0
    session.assign_socket(None).unwrap();

    // Response for socket 1 (not assigned) should be ignored
    let response = vec![0x90, 0x41, 0xFF]; // ACK for socket 1
    let result = session.process_response(&response).unwrap();

    // Should return None (ignored)
    assert!(result.is_none());
}

#[test]
fn test_multiple_errors_in_sequence() {
    let mut session = ViscaSession::new();
    let socket = session.assign_socket(None).unwrap();

    // Multiple errors can occur if camera state changes
    let responses = vec![
        vec![0x90, 0x40 | socket, 0xFF],       // ACK
        vec![0x90, 0x60 | socket, 0x02, 0xFF], // Syntax Error
    ];

    for response in responses {
        let result = session.process_response(&response).unwrap();
        assert!(result.is_some());
    }
}

#[test]
fn test_socket_reuse_after_completion() {
    let mut session = ViscaSession::new();

    // First command uses socket 0
    let socket1 = session.assign_socket(None).unwrap();
    assert_eq!(socket1, 0);

    // Process ACK and completion
    session.process_response(&vec![0x90, 0x40, 0xFF]).unwrap();
    session.process_response(&vec![0x90, 0x50, 0xFF]).unwrap();

    // Release socket 0
    session.release_socket(0);

    // Next command should reuse socket 0
    let socket2 = session.assign_socket(None).unwrap();
    assert_eq!(socket2, 0);
}
