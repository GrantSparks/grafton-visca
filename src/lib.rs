pub mod visca_command;

use log::{debug, error, info};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, UdpSocket},
    time::Duration,
};
use visca_command::{ViscaCommand, ViscaResponseType};

pub trait ViscaTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> io::Result<()>;
    fn receive_response(&mut self) -> io::Result<Vec<Vec<u8>>>;
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

fn parse_response(buffer: &[u8]) -> io::Result<Vec<Vec<u8>>> {
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
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Incomplete response received",
        ));
    }

    Ok(responses)
}

impl ViscaTransport for UdpTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> io::Result<()> {
        let command_bytes = command.to_bytes();
        debug!("Command bytes: {:02X?}", command_bytes);
        self.socket.send_to(&command_bytes, &self.address)?;
        info!("Sent {} bytes to {}", command_bytes.len(), self.address);
        Ok(())
    }

    fn receive_response(&mut self) -> io::Result<Vec<Vec<u8>>> {
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
                    return Err(e);
                }
            }
        }

        parse_response(&received_data)
    }
}

impl ViscaTransport for TcpTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> io::Result<()> {
        let command_bytes = command.to_bytes();
        debug!("Command bytes: {:02X?}", command_bytes);
        self.stream.write_all(&command_bytes)?;
        info!("Sent {} bytes", command_bytes.len());
        Ok(())
    }

    fn receive_response(&mut self) -> io::Result<Vec<Vec<u8>>> {
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
                    return Err(e);
                }
            }
        }

        parse_response(&received_data)
    }
}

pub fn send_command_and_wait(
    transport: &mut dyn ViscaTransport,
    command: &ViscaCommand,
) -> io::Result<()> {
    transport.send_command(command)?;

    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    debug!("Received response: {:02X?}", response);
                    display_camera_response(&response, command);

                    if response.ends_with(&[0xFF]) {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                error!("Failed to receive response: {}", e);
                return Err(e);
            }
        }
    }
}

fn display_camera_response(response: &[u8], command: &ViscaCommand) {
    debug!("Received response: {:02X?}", response);

    if response.len() == 3 && response[0] == 0x90 {
        handle_ack_completion_response(response);
        return;
    }

    if let Some(response_type) = command.response_type() {
        let response_type_str = format!("{:?}", response_type);
        match response_type {
            ViscaResponseType::Luminance
            | ViscaResponseType::Contrast
            | ViscaResponseType::SharpnessPosition
            | ViscaResponseType::RedTuning
            | ViscaResponseType::BlueTuning
            | ViscaResponseType::ExposureCompensationPosition
            | ViscaResponseType::RedGain
            | ViscaResponseType::BlueGain => {
                handle_response(response, 11, &response_type_str, &[8, 9])
            }

            ViscaResponseType::SharpnessMode
            | ViscaResponseType::HorizontalFlip
            | ViscaResponseType::VerticalFlip
            | ViscaResponseType::ImageFlip
            | ViscaResponseType::BlackWhiteMode
            | ViscaResponseType::ExposureCompensationMode
            | ViscaResponseType::Backlight
            | ViscaResponseType::GainLimit
            | ViscaResponseType::AntiFlicker
            | ViscaResponseType::WhiteBalanceMode
            | ViscaResponseType::FocusZone
            | ViscaResponseType::AutoWhiteBalanceSensitivity
            | ViscaResponseType::ThreeDNoiseReduction
            | ViscaResponseType::TwoDNoiseReduction
            | ViscaResponseType::MenuOpenClose
            | ViscaResponseType::UsbAudio
            | ViscaResponseType::Rtmp
            | ViscaResponseType::AutoFocusSensitivity => {
                handle_response(response, 5, &response_type_str, &[3])
            }

            ViscaResponseType::Iris
            | ViscaResponseType::Shutter
            | ViscaResponseType::Saturation
            | ViscaResponseType::Hue
            | ViscaResponseType::ColorTemperature
            | ViscaResponseType::FocusPosition
            | ViscaResponseType::FocusRange
            | ViscaResponseType::MotionSyncSpeed
            | ViscaResponseType::ZoomPosition
            | ViscaResponseType::ZoomWideStandard
            | ViscaResponseType::ZoomTeleStandard => {
                handle_response(response, 7, &response_type_str, &[4, 5])
            }

            ViscaResponseType::PanTiltPosition => handle_pan_tilt_position_response(response),

            ViscaResponseType::BlockLens
            | ViscaResponseType::BlockColorExposure
            | ViscaResponseType::BlockPowerImageEffect
            | ViscaResponseType::BlockImage => handle_block_response(response, &response_type_str),
        }
    } else {
        error!("Unknown command response type: {:02X?}", command);
    }
}

fn handle_ack_completion_response(response: &[u8]) {
    match response[1] {
        0x41..=0x5F => {
            debug!("Handling ACK/Completion response: {:02X?}", response);
            if response[1] & 0x10 == 0x10 {
                info!(
                    "Completion: Command executed with status code: {:02X}",
                    response[1]
                );
            } else {
                info!(
                    "ACK: Command accepted with status code: {:02X}",
                    response[1]
                );
            }
        }
        _ => error!("Unexpected response format: {:02X?}", response),
    }
}

fn handle_response(response: &[u8], expected_len: usize, response_type: &str, indices: &[usize]) {
    if response.len() == expected_len
        && response[0] == 0x90
        && response[1] == 0x50
        && response[expected_len - 1] == 0xFF
    {
        let values: Vec<u8> = indices.iter().map(|&i| response[i]).collect();
        info!("{}: {:02X?}", response_type, values);
    } else {
        error!(
            "Invalid {} response format: {:02X?}",
            response_type, response
        );
    }
}

fn handle_pan_tilt_position_response(response: &[u8]) {
    if response.len() == 11 && response[0] == 0x90 && response[1] == 0x50 && response[10] == 0xFF {
        let w = &response[4..8];
        let z = &response[8..10];
        info!(
            "Pan Tilt Position: Pan={:02X}{:02X}{:02X}{:02X}, Tilt={:02X}{:02X}",
            w[0], w[1], w[2], w[3], z[0], z[1]
        );
    } else {
        error!(
            "Invalid Pan Tilt Position response format: {:02X?}",
            response
        );
    }
}

fn handle_block_response(response: &[u8], response_type: &str) {
    if response.len() == 11 && response[0] == 0x90 && response[1] == 0x50 && response[10] == 0xFF {
        info!("{} Inquiry: {:02X?}", response_type, response);
    } else {
        error!(
            "Invalid {} response format: {:02X?}",
            response_type, response
        );
    }
}
