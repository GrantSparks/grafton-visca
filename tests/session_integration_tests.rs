//! Integration tests for ViscaSession behavior through the public API.
//!
//! These tests verify that the internal session management correctly handles:
//! - Socket assignment and reuse
//! - Command queueing when sockets are full
//! - Proper timeout handling
//! - Error recovery

use grafton_visca::{
    command::{
        pan_tilt::PanTiltCommand,
        power::{Power, PowerCommand},
        zoom::ZoomCommand,
        InquiryCommand,
    },
    ViscaCommand, ViscaDevice, ViscaError, ViscaInquiryResponse, ViscaResponse,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Test device that simulates realistic VISCA timing and socket behavior
struct TimedMockDevice {
    responses: Arc<Mutex<Vec<(Duration, Vec<u8>)>>>,
    start_time: Instant,
    commands_sent: Arc<Mutex<Vec<(Instant, Vec<u8>)>>>,
}

impl TimedMockDevice {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
            commands_sent: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn add_response_at(&mut self, delay: Duration, response: Vec<u8>) {
        self.responses.lock().unwrap().push((delay, response));
    }
    
    fn add_ack_completion(&mut self, socket: u8, ack_delay: Duration, completion_delay: Duration) {
        self.add_response_at(ack_delay, vec![0x90, 0x40 | socket, 0xFF]);
        self.add_response_at(completion_delay, vec![0x90, 0x50 | socket, 0xFF]);
    }
    
    fn command_count(&self) -> usize {
        self.commands_sent.lock().unwrap().len()
    }
    
    fn command_timing(&self) -> Vec<Duration> {
        self.commands_sent.lock().unwrap()
            .iter()
            .map(|(time, _)| time.duration_since(self.start_time))
            .collect()
    }
}

impl ViscaDevice for TimedMockDevice {
    fn execute_command(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Record when command was sent
        let now = Instant::now();
        self.commands_sent.lock().unwrap().push((now, command.to_bytes()?));
        
        // Get the next response based on timing
        let elapsed = now.duration_since(self.start_time);
        
        let response = {
            let mut responses = self.responses.lock().unwrap();
            responses.iter()
                .position(|(delay, _)| *delay <= elapsed)
                .and_then(|idx| Some(responses.remove(idx).1))
        };
        
        if let Some(resp) = response {
            // Parse based on response type
            if resp[0] == 0x90 && (resp[1] & 0xF0) == 0x40 {
                // This is an ACK, need to wait for completion
                let comp_response = {
                    let elapsed = Instant::now().duration_since(self.start_time);
                    let mut responses = self.responses.lock().unwrap();
                    responses.iter()
                        .position(|(delay, _)| *delay <= elapsed)
                        .and_then(|idx| Some(responses.remove(idx).1))
                };
                
                if let Some(comp) = comp_response {
                    if let Some(resp_type) = command.response_type() {
                        grafton_visca::command::response::parse_visca_response(&comp, &resp_type)
                    } else {
                        Ok(ViscaResponse::Completion)
                    }
                } else {
                    Err(ViscaError::Timeout)
                }
            } else {
                // Direct response (inquiry or error)
                if let Some(resp_type) = command.response_type() {
                    grafton_visca::command::response::parse_visca_response(&resp, &resp_type)
                } else {
                    Ok(ViscaResponse::Completion)
                }
            }
        } else {
            Err(ViscaError::Timeout)
        }
    }
}

#[test]
fn test_socket_assignment_and_reuse() {
    let mut device = TimedMockDevice::new();
    
    // Set up responses: socket 0 for first command, socket 1 for second
    device.add_ack_completion(0, Duration::from_millis(10), Duration::from_millis(100));
    device.add_ack_completion(1, Duration::from_millis(15), Duration::from_millis(80));
    // After socket 1 completes, socket 0 is still busy, so third command reuses socket 1
    device.add_ack_completion(1, Duration::from_millis(90), Duration::from_millis(150));
    
    // Send three commands in sequence
    let start = Instant::now();
    
    // First command - should get socket 0
    thread::sleep(Duration::from_millis(5));
    assert!(device.execute_command(&PanTiltCommand::Home).is_ok());
    
    // Second command - should get socket 1 (socket 0 still busy)
    thread::sleep(Duration::from_millis(5));
    assert!(device.execute_command(&ZoomCommand::Stop).is_ok());
    
    // Third command - should reuse socket 1 after it completes
    thread::sleep(Duration::from_millis(70)); // Wait for socket 1 to be free
    assert!(device.execute_command(&PowerCommand { power: Power::On }).is_ok());
    
    assert_eq!(device.command_count(), 3);
}

#[test]
fn test_command_buffer_full_handling() {
    let mut device = TimedMockDevice::new();
    
    // First command gets ACK and starts executing
    device.add_response_at(Duration::from_millis(10), vec![0x90, 0x40, 0xFF]); // ACK socket 0
    
    // Second command gets ACK  
    device.add_response_at(Duration::from_millis(20), vec![0x90, 0x41, 0xFF]); // ACK socket 1
    
    // Third command should get buffer full error (both sockets busy)
    device.add_response_at(Duration::from_millis(30), vec![0x90, 0x60, 0x01, 0xFF]); // Buffer full
    
    // Eventually first command completes
    device.add_response_at(Duration::from_millis(100), vec![0x90, 0x50, 0xFF]); // Completion socket 0
    device.add_response_at(Duration::from_millis(110), vec![0x90, 0x51, 0xFF]); // Completion socket 1
    
    // Send commands rapidly
    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&PanTiltCommand::Home);
    assert!(r1.is_ok());
    
    thread::sleep(Duration::from_millis(10));
    let r2 = device.execute_command(&ZoomCommand::Stop);
    assert!(r2.is_ok());
    
    thread::sleep(Duration::from_millis(10));
    let r3 = device.execute_command(&PowerCommand { power: Power::On });
    assert!(r3.is_err());
    assert!(matches!(r3.unwrap_err(), ViscaError::CommandBufferFull));
}

#[test]
fn test_inquiry_no_socket_usage() {
    let mut device = TimedMockDevice::new();
    
    // Inquiry responses don't use sockets - direct response
    device.add_response_at(Duration::from_millis(10), vec![0x90, 0x50, 0x02, 0xFF]); // Power ON
    device.add_response_at(Duration::from_millis(20), vec![
        0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF
    ]); // Position
    
    // Control command uses socket
    device.add_ack_completion(0, Duration::from_millis(30), Duration::from_millis(100));
    
    // Send inquiry commands - they shouldn't consume sockets
    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&InquiryCommand::Power);
    assert!(r1.is_ok());
    
    thread::sleep(Duration::from_millis(10));
    let r2 = device.execute_command(&InquiryCommand::PanTiltPosition);
    assert!(r2.is_ok());
    
    // Control command should still get socket 0
    thread::sleep(Duration::from_millis(10));
    let r3 = device.execute_command(&PanTiltCommand::Home);
    assert!(r3.is_ok());
}

#[test]
fn test_timeout_recovery() {
    let mut device = TimedMockDevice::new();
    
    // First command gets ACK but no completion (timeout)
    device.add_response_at(Duration::from_millis(10), vec![0x90, 0x40, 0xFF]); // ACK socket 0
    // No completion - will timeout
    
    // Second command should work normally
    device.add_ack_completion(1, Duration::from_millis(200), Duration::from_millis(300));
    
    // First command times out
    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&PanTiltCommand::Home);
    assert!(r1.is_err());
    assert!(matches!(r1.unwrap_err(), ViscaError::Timeout));
    
    // System should recover - second command works
    thread::sleep(Duration::from_millis(190));
    let r2 = device.execute_command(&ZoomCommand::Stop);
    assert!(r2.is_ok());
}

#[test]
fn test_error_response_frees_socket() {
    let mut device = TimedMockDevice::new();
    
    // First command gets ACK then error
    device.add_response_at(Duration::from_millis(10), vec![0x90, 0x40, 0xFF]); // ACK socket 0
    device.add_response_at(Duration::from_millis(50), vec![0x90, 0x60, 0x41, 0xFF]); // Not executable
    
    // Second command should be able to use socket 0 after error
    device.add_ack_completion(0, Duration::from_millis(60), Duration::from_millis(100));
    
    // First command fails
    thread::sleep(Duration::from_millis(5));
    let r1 = device.execute_command(&PowerCommand { power: Power::On });
    assert!(r1.is_err());
    
    // Socket should be freed, second command succeeds
    thread::sleep(Duration::from_millis(50));
    let r2 = device.execute_command(&PanTiltCommand::Home);
    assert!(r2.is_ok());
}

#[test]
fn test_rapid_command_timing() {
    let mut device = TimedMockDevice::new();
    
    // Set up overlapping command execution
    device.add_ack_completion(0, Duration::from_millis(10), Duration::from_millis(100));
    device.add_ack_completion(1, Duration::from_millis(20), Duration::from_millis(80));
    device.add_ack_completion(0, Duration::from_millis(110), Duration::from_millis(150));
    
    // Send commands rapidly
    let start = Instant::now();
    
    thread::sleep(Duration::from_millis(5));
    device.execute_command(&PanTiltCommand::Home).unwrap();
    
    thread::sleep(Duration::from_millis(10));
    device.execute_command(&ZoomCommand::Stop).unwrap();
    
    thread::sleep(Duration::from_millis(85)); // Wait for socket to be free
    device.execute_command(&PowerCommand { power: Power::On }).unwrap();
    
    let timings = device.command_timing();
    assert_eq!(timings.len(), 3);
    
    // Verify commands were sent at expected times
    assert!(timings[0] < Duration::from_millis(10));
    assert!(timings[1] < Duration::from_millis(25));
    assert!(timings[2] > Duration::from_millis(90));
}