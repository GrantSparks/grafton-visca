pub mod visca_command;

use log::{debug, error, info};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, UdpSocket},
    time::Duration,
};

pub use visca_command::{ViscaCommand, ViscaInquiryResponse, ViscaResponse, ViscaResponseType};

mod error;
pub use error::{AppError, ViscaError};

pub trait ViscaTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> Result<(), ViscaError>;
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
        return Err(ViscaError::InvalidResponseFormat);
    }

    Ok(responses)
}

impl ViscaTransport for UdpTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> Result<(), ViscaError> {
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
                    info!(
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
    fn send_command(&mut self, command: &ViscaCommand) -> Result<(), ViscaError> {
        let command_bytes = command.to_bytes()?;
        self.stream
            .write_all(&command_bytes)
            .map_err(ViscaError::Io)?;
        info!("Sent {} bytes", command_bytes.len());
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut received_data = Vec::new();

        loop {
            match self.stream.read(&mut buffer) {
                Ok(bytes_received) => {
                    info!(
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
    command: &ViscaCommand,
) -> Result<ViscaResponse, ViscaError> {
    transport.send_command(command)?;

    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    let parsed_response = parse_and_handle_response(&response, command)?;
                    match parsed_response {
                        ViscaResponse::Completion | ViscaResponse::InquiryResponse(_) => {
                            return Ok(parsed_response);
                        }
                        _ => continue,
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
}

fn parse_and_handle_response(
    response: &[u8],
    command: &ViscaCommand,
) -> Result<ViscaResponse, ViscaError> {
    debug!("Received response: {:02X?}", response);

    if let Some(response_type) = command.response_type() {
        match ViscaResponse::from_bytes(response, &response_type) {
            Ok(visca_response) => {
                if let ViscaResponse::InquiryResponse(inquiry_response) = &visca_response {
                    log_inquiry_response(inquiry_response);
                }
                log_response(&visca_response);
                Ok(visca_response)
            }
            Err(e) => {
                error!("Error processing response: {}", e);
                Err(e)
            }
        }
    } else {
        handle_ack_completion_response(response)
    }
}

fn handle_ack_completion_response(response: &[u8]) -> Result<ViscaResponse, ViscaError> {
    if response.len() == 3 && response[0] == 0x90 && response[2] == 0xFF {
        match response[1] {
            0x40..=0x4F => {
                debug!("Handling ACK response: {:02X?}", response);
                Ok(ViscaResponse::Ack)
            }
            0x50..=0x5F => {
                debug!("Handling Completion response: {:02X?}", response);
                Ok(ViscaResponse::Completion)
            }
            0x60..=0x6F => Err(ViscaError::from_code(response[1])),
            _ => Err(ViscaError::Unknown(response[1])),
        }
    } else {
        Err(ViscaError::Unknown(response[1]))
    }
}

fn log_inquiry_response(inquiry_response: &ViscaInquiryResponse) {
    match inquiry_response {
        ViscaInquiryResponse::PanTiltPosition { pan, tilt } => {
            info!("Pan: {}, Tilt: {}", pan, tilt);
        }
        ViscaInquiryResponse::Luminance(luminance) => {
            info!("Luminance: {}", luminance);
        }
        ViscaInquiryResponse::Contrast(contrast) => {
            info!("Contrast: {}", contrast);
        }
        ViscaInquiryResponse::ZoomPosition { position } => {
            info!("Zoom Position: {:02X?}", position);
        }
        ViscaInquiryResponse::FocusPosition { position } => {
            info!("Focus Position: {:02X?}", position);
        }
        ViscaInquiryResponse::Gain { gain } => {
            info!("Gain: {}", gain);
        }
        ViscaInquiryResponse::WhiteBalance { mode } => {
            info!("White Balance Mode: {}", mode);
        }
        ViscaInquiryResponse::ExposureCompensation { value } => {
            info!("Exposure Compensation Value: {}", value);
        }
        ViscaInquiryResponse::Backlight { status } => {
            info!("Backlight Status: {}", status);
        }
        ViscaInquiryResponse::ColorTemperature { temperature } => {
            info!("Color Temperature: {}", temperature);
        }
        ViscaInquiryResponse::Hue { hue } => {
            info!("Hue: {}", hue);
        } // Add additional inquiry responses as needed
    }
}

fn log_response(response: &ViscaResponse) {
    match response {
        ViscaResponse::Ack => debug!("ACK received"),
        ViscaResponse::Completion => info!("Completion received"),
        ViscaResponse::Error(err) => error!("Error received: {:?}", err),
        ViscaResponse::InquiryResponse(inquiry_response) => {
            debug!("Inquiry response: {:?}", inquiry_response);
        }
        _ => (),
    }
}
