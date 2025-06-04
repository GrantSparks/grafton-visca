use grafton_visca::command::{InquiryCommand, PanTiltCommand};
use grafton_visca::{
    send_command_and_wait, ViscaCommand, ViscaError, ViscaResponse, ViscaTransport,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Mock transport for testing ACK/Completion handling
struct MockTransport {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl MockTransport {
    fn new(responses: Vec<Vec<u8>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
        }
    }
}

impl ViscaTransport for MockTransport {
    fn send_command(&mut self, _command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut responses = self.responses.lock().unwrap();
        if let Some(response) = responses.pop_front() {
            Ok(vec![response])
        } else {
            Err(ViscaError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No more responses",
            )))
        }
    }
}

#[test]
fn test_ack_then_completion_sequence() {
    // Simulate ACK followed by completion for socket 0
    let responses = vec![
        vec![0x90, 0x40, 0xFF], // ACK on socket 0
        vec![0x90, 0x50, 0xFF], // Completion on socket 0
    ];

    let mut transport = MockTransport::new(responses);
    let command = PanTiltCommand::Home;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), ViscaResponse::Completion));
}

#[test]
fn test_command_error_handling() {
    // Simulate ACK followed by error
    let responses = vec![
        vec![0x90, 0x40, 0xFF],       // ACK on socket 0
        vec![0x90, 0x60, 0x41, 0xFF], // Command Not Executable error
    ];

    let mut transport = MockTransport::new(responses);
    let command = PanTiltCommand::Home;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ViscaError::CommandNotExecutable
    ));
}

#[test]
fn test_inquiry_response_handling() {
    // Simulate ACK followed by inquiry response
    let responses = vec![
        vec![0x90, 0x40, 0xFF], // ACK on socket 0
        vec![
            0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
        ], // Pan/Tilt position response
    ];

    let mut transport = MockTransport::new(responses);
    let command = InquiryCommand::PanTiltPosition;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_ok());

    match result.unwrap() {
        ViscaResponse::InquiryResponse(inquiry) => {
            if let grafton_visca::ViscaInquiryResponse::PanTiltPosition { pan, tilt } = inquiry {
                assert_eq!(pan, 0);
                assert_eq!(tilt, 0);
            } else {
                panic!("Expected PanTiltPosition inquiry response");
            }
        }
        _ => panic!("Expected inquiry response"),
    }
}

#[test]
fn test_immediate_error_without_ack() {
    // Simulate immediate syntax error without ACK
    let responses = vec![
        vec![0x90, 0x60, 0x02, 0xFF], // Syntax error
    ];

    let mut transport = MockTransport::new(responses);
    let command = PanTiltCommand::Home;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ViscaError::SyntaxError));
}

#[test]
fn test_buffer_full_error() {
    // Simulate command buffer full error (before any ACK)
    let responses = vec![
        vec![0x90, 0x60, 0x03, 0xFF], // Command Buffer Full
    ];

    let mut transport = MockTransport::new(responses);
    let command = PanTiltCommand::Home;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ViscaError::CommandBufferFull));
}

#[test]
fn test_direct_completion_without_ack() {
    // Some cameras might send completion directly without ACK for certain commands
    let responses = vec![
        vec![0x90, 0x50, 0xFF], // Direct completion on socket 0
    ];

    let mut transport = MockTransport::new(responses);
    let command = PanTiltCommand::Home;

    let result = send_command_and_wait(&mut transport, &command);
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), ViscaResponse::Completion));
}
