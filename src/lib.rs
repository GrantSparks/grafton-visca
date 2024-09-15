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
    transport.send_command(command)?;

    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    let parsed_response =
                        parse_and_handle_response(&response, command.response_type())?;
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
    response_type: Option<ViscaResponseType>,
) -> Result<ViscaResponse, ViscaError> {
    debug!("Received response: {:02X?}", response);

    if let Some(response_type) = response_type {
        match parse_visca_response(response, &response_type) {
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
        error!("No response type provided for response: {:02X?}", response);
        Err(ViscaError::UnexpectedResponseType)
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

fn log_response(response: &ViscaResponse) {
    match response {
        ViscaResponse::Ack => debug!("ACK received"),
        ViscaResponse::Completion => debug!("Completion received"),
        ViscaResponse::Error(err) => error!("Error received: {:?}", err),
        ViscaResponse::InquiryResponse(inquiry_response) => {
            debug!("Inquiry response: {:?}", inquiry_response);
        }
        _ => (),
    }
}
