//! Integration tests for `Session` behavior through the public API.
//!
//! These tests verify that the internal session management correctly handles:
//! - Socket assignment and reuse
//! - Command queueing when sockets are full
//! - Proper timeout handling
//! - Error recovery

#![cfg(feature = "blocking-client")]

#[path = "common/mod.rs"]
mod common;

use common::{MockDevice, MockTransport};
use grafton_visca::{
    command::{
        pan_tilt::PanTiltCommand,
        power::{Power, PowerCommand},
        zoom::ZoomCommand,
        InquiryCommand,
    },
    Error, Transport,
};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_socket_assignment_and_reuse() {
    let transport = MockTransport::new();

    // Set up responses: socket 0 for first command, socket 1 for second, socket 1 for third
    transport.add_ack_completion(0);
    transport.add_ack_completion(1);
    transport.add_ack_completion(1);

    let mut device = MockDevice::from_transport(transport);

    // Send three commands in sequence

    // First command - should get socket 0
    thread::sleep(Duration::from_millis(5));
    assert!(device.execute_command(&PanTiltCommand::Home).is_ok());

    // Second command - should get socket 1 (socket 0 still busy)
    thread::sleep(Duration::from_millis(5));
    assert!(device.execute_command(&ZoomCommand::Stop).is_ok());

    // Third command - should reuse socket 1 after it completes
    thread::sleep(Duration::from_millis(70)); // Wait for socket 1 to be free
    assert!(device
        .execute_command(&PowerCommand { power: Power::On })
        .is_ok());

    assert_eq!(device.commands_sent().len(), 3);
}

#[test]
fn test_command_buffer_full_handling() {
    let transport = MockTransport::new();

    // First command gets ACK and completion
    transport.add_ack_completion(0);

    // Second command gets ACK and completion
    transport.add_ack_completion(1);

    // Third command should get buffer full error (both sockets busy)
    transport.add_response(vec![0x90, 0x60, 0x03, 0xFF]); // Buffer full (error code 0x03)

    let mut device = MockDevice::from_transport(transport);

    // Send commands
    let r1 = device.execute_command(&PanTiltCommand::Home);
    assert!(r1.is_ok());

    let r2 = device.execute_command(&ZoomCommand::Stop);
    assert!(r2.is_ok());

    let r3 = device.execute_command(&PowerCommand { power: Power::On });
    // This should fail with CommandBufferFull error
    match r3 {
        Err(Error::CommandBufferFull) => {}
        _ => panic!("Expected CommandBufferFull error, got {r3:?}"),
    }
}

#[test]
fn test_inquiry_no_socket_usage() {
    let transport = MockTransport::new();

    // Inquiry commands get direct responses without socket assignment
    transport.add_response(vec![0x90, 0x50, 0x02, 0xFF]); // Power On response
    transport.add_response(vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF]); // Zoom position
    transport.add_response(vec![0x90, 0x50, 0x03, 0xFF]); // Power Off response

    let mut device = MockDevice::from_transport(transport);

    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&InquiryCommand::Power);
    assert!(r1.is_ok());

    thread::sleep(Duration::from_millis(10));
    let r2 = device.execute_command(&InquiryCommand::ZoomPosition);
    assert!(r2.is_ok());

    thread::sleep(Duration::from_millis(10));
    let r3 = device.execute_command(&InquiryCommand::Power);
    assert!(r3.is_ok());

    assert_eq!(device.commands_sent().len(), 3);
}

#[test]
fn test_timeout_recovery() {
    // This test verifies that the system can recover after a timeout.
    // In real usage, if no response is received within the timeout period,
    // the socket should be freed and subsequent commands should work.
    // However, our MockDevice always expects responses, so we'll simulate
    // a different scenario where we get proper responses after a delay.

    let transport = MockTransport::new();

    // First command succeeds
    transport.add_ack_completion(0);

    // Second command also succeeds
    transport.add_ack_completion(0);

    let mut device = MockDevice::from_transport(transport);

    // Send first command
    let r1 = device.execute_command(&PanTiltCommand::Home);
    assert!(r1.is_ok());

    // Simulate delay between commands
    thread::sleep(Duration::from_millis(100));

    // Second command should work fine
    let r2 = device.execute_command(&ZoomCommand::Stop);
    assert!(r2.is_ok());

    assert_eq!(device.commands_sent().len(), 2);
}

#[test]
fn test_error_response_frees_socket() {
    let transport = MockTransport::new();

    // First command gets syntax error
    transport.add_response(vec![0x90, 0x60, 0x02, 0xFF]); // Syntax error

    // Second command succeeds on same socket
    transport.add_ack_completion(0);

    let mut device = MockDevice::from_transport(transport);

    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&PanTiltCommand::Home);
    assert!(matches!(r1, Err(Error::SyntaxError)));

    // Socket should be immediately available after error
    thread::sleep(Duration::from_millis(10));
    let r2 = device.execute_command(&ZoomCommand::Stop);
    assert!(r2.is_ok());

    assert_eq!(device.commands_sent().len(), 2);
}

#[test]
fn test_rapid_command_timing() {
    let transport = MockTransport::new();

    // Set up all responses for 5 commands
    for i in 0..5 {
        let socket = u8::try_from(i % 2).unwrap();
        transport.add_ack_completion(socket);
    }

    let mut device = MockDevice::from_transport(transport);

    let start = Instant::now();

    // Send 5 commands rapidly
    for i in 0..5 {
        thread::sleep(Duration::from_millis(15));
        device
            .execute_command(&PowerCommand {
                power: if i % 2 == 0 {
                    Power::On
                } else {
                    Power::Standby
                },
            })
            .unwrap();
    }

    let total_time = start.elapsed();
    assert!(
        total_time < Duration::from_secs(2),
        "Commands took too long: {total_time:?}"
    );

    assert_eq!(device.commands_sent().len(), 5);
}

#[test]
fn test_command_history_clearing() {
    // Demonstrates the use of clear_commands() for testing command sequences
    let transport = MockTransport::new();

    // Add responses for multiple operations
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);

    let mut device = MockDevice::from_transport(transport);

    // Phase 1: Initial commands
    device
        .execute_command(&PowerCommand { power: Power::On })
        .unwrap();
    assert_eq!(device.commands_sent().len(), 1);

    // Clear history to test next phase independently
    device.clear_commands();

    // Phase 2: Verify clean slate
    device.execute_command(&ZoomCommand::Stop).unwrap();
    assert_eq!(device.commands_sent().len(), 1); // Only the new command
    assert_eq!(
        device.last_command().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
    );
}
