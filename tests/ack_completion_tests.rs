#![allow(missing_docs)]
//! Tests for ACK/Completion response handling in the new unified client.
//!
//! These tests verify that the client correctly handles the ACK+Completion
//! response pattern used by VISCA cameras.

#![cfg(feature = "blocking-client")]

mod common;

use common::{MockDevice, MockTransport};
use grafton_visca::{
    command::{
        pan_tilt::PanTiltCommand,
        power::{Power, PowerCommand},
        InquiryCommand,
    },
    Error, InquiryResponse, Response, Transport,
};

#[test]
fn test_ack_then_completion_sequence() {
    // Simulate ACK followed by completion for socket 0
    let transport = MockTransport::new();
    transport.add_ack_completion(0);

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&PanTiltCommand::Home);

    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), Response::Completion));

    // Verify command was sent
    let commands = device.commands_sent();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], vec![0x81, 0x01, 0x06, 0x04, 0xFF]); // Home command
}

#[test]
fn test_command_error_handling() {
    // Simulate ACK followed by error
    let transport = MockTransport::new();
    transport.add_response(vec![0x90, 0x41, 0xFF]); // ACK on socket 1
    transport.add_response(vec![0x90, 0x60, 0x41, 0xFF]); // Command Not Executable error

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&PowerCommand { power: Power::On });

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::CommandNotExecutable));
}

#[test]
fn test_inquiry_direct_response() {
    // Inquiry commands should not receive ACK, just direct response
    let transport = MockTransport::new();
    transport.add_response(vec![0x90, 0x50, 0x02, 0xFF]); // Power ON response

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&InquiryCommand::Power);

    assert!(result.is_ok());
    match result.unwrap() {
        Response::InquiryResponse(InquiryResponse::Power { on }) => {
            assert!(on);
        }
        _ => panic!("Expected Power inquiry response"),
    }
}

#[test]
fn test_pan_tilt_position_inquiry() {
    // Test pan/tilt position inquiry parsing
    let transport = MockTransport::new();
    transport.add_response(vec![
        0x90, 0x50, // Header
        0x01, 0x02, 0x03, 0x04, // Pan position
        0x05, 0x06, 0x07, 0x08, // Tilt position
        0xFF,
    ]);

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&InquiryCommand::PanTiltPosition);

    assert!(result.is_ok());
    match result.unwrap() {
        Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0x1234);
            assert_eq!(tilt, 0x5678);
        }
        _ => panic!("Expected PanTiltPosition inquiry response"),
    }
}

#[test]
fn test_multiple_socket_handling() {
    // Test that different commands can use different sockets
    let transport = MockTransport::new();
    transport.add_ack_completion(0); // First command uses socket 0
    transport.add_ack_completion(1); // Second command uses socket 1

    let mut device = MockDevice::from_transport(transport);

    // First command
    let result1 = device.execute_command(&PanTiltCommand::Home);
    assert!(result1.is_ok());

    // Second command would use socket 1
    let result2 = device.execute_command(&PowerCommand { power: Power::On });
    assert!(result2.is_ok());

    // Verify both commands were sent
    assert_eq!(device.commands_sent().len(), 2);
}

#[test]
fn test_timeout_on_missing_completion() {
    // Simulate ACK but no completion (timeout scenario)
    let transport = MockTransport::new();
    transport.add_response(vec![0x90, 0x40, 0xFF]); // ACK on socket 0
                                                    // No completion - should timeout

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&PanTiltCommand::Home);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Timeout));
}

#[test]
fn test_socket_buffer_full_error() {
    // Simulate command buffer full error
    let transport = MockTransport::new();
    transport.add_response(vec![0x90, 0x60, 0x03, 0xFF]); // Command Buffer Full error (code 0x03)

    let mut device = MockDevice::from_transport(transport);
    let result = device.execute_command(&PanTiltCommand::Home);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::CommandBufferFull));
}

#[test]
fn test_command_history_management() {
    // This test demonstrates the utility of MockTransport's history management methods
    let mut device = MockDevice::with_completion();

    // Execute first command and verify
    device
        .execute_command(&PowerCommand { power: Power::On })
        .unwrap();
    assert_eq!(device.commands_sent().len(), 1);
    assert_eq!(
        device.last_command().unwrap(),
        vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    );

    // Add more responses for next commands
    device.add_response(vec![0x90, 0x41, 0xFF]); // ACK
    device.add_response(vec![0x90, 0x51, 0xFF]); // Completion

    // Execute second command
    device
        .execute_command(&PowerCommand {
            power: Power::Standby,
        })
        .unwrap();
    assert_eq!(device.commands_sent().len(), 2);

    // Verify we can check just the last command without iterating
    assert_eq!(
        device.last_command().unwrap(),
        vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]
    );
}

#[test]
fn test_inquiry_response_queueing() {
    // Demonstrates the queue_inquiry_response utility method
    let mut device = MockDevice::new();

    // Queue multiple inquiry responses
    device.queue_inquiry_response(InquiryResponse::Power { on: true });
    device.queue_inquiry_response(InquiryResponse::ZoomPosition { position: 0x1234 });

    // Execute inquiries and verify responses
    let power_response = device.execute_command(&InquiryCommand::Power).unwrap();
    assert!(matches!(
        power_response,
        Response::InquiryResponse(InquiryResponse::Power { on: true })
    ));

    let zoom_response = device
        .execute_command(&InquiryCommand::ZoomPosition)
        .unwrap();
    assert!(matches!(
        zoom_response,
        Response::InquiryResponse(InquiryResponse::ZoomPosition { position: 0x1234 })
    ));
}
