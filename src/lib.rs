//! # grafton-visca
//!
//! A Rust library for controlling PTZ cameras using the VISCA over IP protocol.
//!
//! ## Features
//!
//! - Full support for PTZOptics G2 VISCA commands
//! - Pan/Tilt/Zoom control with absolute and relative positioning
//! - Exposure control (iris, shutter, gain, brightness)
//! - Color adjustments (white balance, saturation, hue)
//! - Focus control with auto/manual modes
//! - Preset positions
//! - Command inquiry support
//! - TCP and UDP transport support
//!
//! ## Example Usage
//!
//! ```no_run
//! use grafton_visca::{UdpTransport, ViscaCommand, ViscaTransport};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Connect to camera
//! let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//!
//! // Send Pan/Tilt Home command
//! transport.send_command(&PanTiltCommand::Home).unwrap();
//!
//! // Zoom in
//! transport.send_command(&ZoomCommand::TeleStandard).unwrap();
//! ```
//!
//! ## Advanced Camera Control
//!
//! ```no_run
//! use grafton_visca::command::*;
//! use grafton_visca::{UdpTransport, ViscaTransport};
//!
//! let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//!
//! // Adjust exposure compensation
//! transport.send_command(&ExposureCompensationCommand::Direct(3)).unwrap();
//!
//! // Set iris to F4.0
//! transport.send_command(&IrisCommand::Direct(0x06)).unwrap();
//!
//! // Adjust color saturation to 150%
//! transport.send_command(&SaturationCommand { level: 0x0A }).unwrap();
//! ```

use log::{debug, error};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, UdpSocket},
    time::Duration,
};

pub mod command;
pub use command::{
    response::{parse_visca_response, ViscaResponse},
    ViscaCommand, ViscaInquiryResponse, ViscaResponseType,
};

mod error;
pub use error::{AppError, ViscaError};

mod session;
pub use session::ViscaSession;

pub trait ViscaTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError>;
    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError>;
}

pub struct UdpTransport {
    socket: UdpSocket,
    address: String,
}

impl UdpTransport {
    pub fn new(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(10)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self {
            socket,
            address: address.to_string(),
        })
    }
}

pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    pub fn new(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        Ok(Self { stream })
    }
}

fn parse_response(buffer: &[u8]) -> Result<Vec<Vec<u8>>, ViscaError> {
    let mut responses = Vec::new();
    let mut response = Vec::new();
    let mut start_index = false;

    for &byte in buffer {
        response.push(byte);
        if byte == 0x90 {
            start_index = true;
        } else if byte == 0xFF && start_index {
            responses.push(response.clone());
            response.clear();
            start_index = false;
        }
    }

    if start_index {
        // Log an error if the response format is invalid
        error!("Invalid response format detected: {:02X?}", response);
        return Err(ViscaError::InvalidResponseFormat);
    }

    // Log the number of responses parsed
    debug!("Parsed {} responses from buffer", responses.len());

    Ok(responses)
}

impl ViscaTransport for UdpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        let command_bytes = command.to_bytes()?;
        self.socket
            .send_to(&command_bytes, &self.address)
            .map_err(ViscaError::Io)?;
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut received_data = Vec::new();

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((bytes_received, src)) => {
                    debug!(
                        "Received {} bytes from {}: {:02X?}",
                        bytes_received,
                        src,
                        &buffer[..bytes_received]
                    );
                    received_data.extend_from_slice(&buffer[..bytes_received]);
                    if buffer[bytes_received - 1] == 0xFF {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to receive response: {}", e);
                    return Err(ViscaError::Io(e));
                }
            }
        }

        parse_response(&received_data)
    }
}

impl ViscaTransport for TcpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        let command_bytes = command.to_bytes()?;
        self.stream
            .write_all(&command_bytes)
            .map_err(ViscaError::Io)?;
        debug!("Sent {} bytes: {:02X?}", command_bytes.len(), command_bytes);
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut received_data = Vec::new();

        loop {
            match self.stream.read(&mut buffer) {
                Ok(bytes_received) => {
                    debug!(
                        "Received {} bytes: {:02X?}",
                        bytes_received,
                        &buffer[..bytes_received]
                    );
                    received_data.extend_from_slice(&buffer[..bytes_received]);
                    if buffer[bytes_received - 1] == 0xFF {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to receive response: {}", e);
                    return Err(ViscaError::Io(e));
                }
            }
        }

        parse_response(&received_data)
    }
}

pub fn send_command_and_wait(
    transport: &mut dyn ViscaTransport,
    command: &dyn ViscaCommand,
) -> Result<ViscaResponse, ViscaError> {
    // Create a session to manage command state
    let mut session = ViscaSession::new();

    // Assign a socket for this command
    let socket_id = session.assign_socket(command.response_type())?;
    debug!("Sending command on socket {}", socket_id);

    // Send the command
    transport.send_command(command)?;

    // Wait for completion
    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    match session.process_response(&response) {
                        Ok(Some((resp_socket_id, parsed_response))) => {
                            // Check if this response is for our command
                            if resp_socket_id == socket_id {
                                match parsed_response {
                                    ViscaResponse::Ack => {
                                        debug!("Command acknowledged on socket {}", socket_id);
                                        // Continue waiting for completion
                                    }
                                    ViscaResponse::Completion => {
                                        debug!("Command completed on socket {}", socket_id);
                                        session.release_socket(socket_id);
                                        return Ok(ViscaResponse::Completion);
                                    }
                                    ViscaResponse::InquiryResponse(inquiry) => {
                                        debug!("Inquiry response received on socket {}", socket_id);
                                        log_inquiry_response(&inquiry);
                                        session.release_socket(socket_id);
                                        return Ok(ViscaResponse::InquiryResponse(inquiry));
                                    }
                                    ViscaResponse::Error(err) => {
                                        error!("Command error on socket {}: {:?}", socket_id, err);
                                        session.release_socket(socket_id);
                                        return Err(err);
                                    }
                                    _ => {
                                        debug!(
                                            "Unexpected response on socket {}: {:?}",
                                            socket_id, parsed_response
                                        );
                                    }
                                }
                            } else {
                                // Response for a different command, log and continue
                                debug!(
                                    "Received response for socket {} (not our socket {})",
                                    resp_socket_id, socket_id
                                );
                            }
                        }
                        Ok(None) => {
                            // Response for unknown socket, ignore
                            debug!("Received response for unknown socket");
                        }
                        Err(e) => {
                            error!("Error processing response: {}", e);
                            session.release_socket(socket_id);
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Transport error: {}", e);
                session.release_socket(socket_id);
                return Err(e);
            }
        }
    }
}

#[allow(unreachable_patterns)]
fn log_inquiry_response(inquiry_response: &ViscaInquiryResponse) {
    match inquiry_response {
        ViscaInquiryResponse::PanTiltPosition { pan, tilt } => {
            debug!("Pan: {}, Tilt: {}", pan, tilt);
        }
        ViscaInquiryResponse::Luminance(luminance) => {
            debug!("Luminance: {}", luminance);
        }
        ViscaInquiryResponse::Contrast(contrast) => {
            debug!("Contrast: {}", contrast);
        }
        ViscaInquiryResponse::ZoomPosition { position } => {
            debug!("Zoom Position: {:02X?}", position);
        }
        ViscaInquiryResponse::FocusPosition { position } => {
            debug!("Focus Position: {:02X?}", position);
        }
        ViscaInquiryResponse::Gain { gain } => {
            debug!("Gain: {}", gain);
        }
        ViscaInquiryResponse::WhiteBalance { mode } => {
            debug!("White Balance Mode: {:?}", mode);
        }
        ViscaInquiryResponse::ExposureMode { mode } => {
            debug!("Exposure Mode: {:?}", mode);
        }
        ViscaInquiryResponse::ExposureCompensation { value } => {
            debug!("Exposure Compensation Value: {}", value);
        }
        ViscaInquiryResponse::Backlight { status } => {
            debug!("Backlight Status: {}", status);
        }
        ViscaInquiryResponse::ColorTemperature { temperature } => {
            debug!("Color Temperature: {}", temperature);
        }
        ViscaInquiryResponse::Hue { hue } => {
            debug!("Hue: {}", hue);
        }
        // Wildcard pattern to handle any future additions to the enum
        _ => {
            debug!("Unhandled inquiry response: {:?}", inquiry_response);
        }
    }
}
