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
                    display_camera_response(&response, command);

                    // Handle the Pan/Tilt position inquiry response explicitly
                    if command.response_type() == Some(ViscaResponseType::PanTiltPosition) {
                        if response.len() == 11
                            && response[0] == 0x90
                            && response[1] == 0x50
                            && response[10] == 0xFF
                        {
                            return Ok(());
                        }
                    } else if response.len() == 4
                        || (response.len() == 3 && (0x50..=0x5F).contains(&response[1]))
                    {
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
            _ => {
                error!("Unexpected response format: {:02X?}", response);
            }
        }
        return;
    }

    match command.response_type() {
        Some(response_type) => match response_type {
            ViscaResponseType::PanTiltPosition => {
                if response.len() == 11
                    && response[0] == 0x90
                    && response[1] == 0x50
                    && response[10] == 0xFF
                {
                    info!(
                            "Pan Tilt Position: Pan={:02X?}{:02X?}{:02X?}{:02X?}, Tilt={:02X?}{:02X?}{:02X?}{:02X?}",
                            response[2], response[3], response[4], response[5], response[6], response[7], response[8], response[9]
                        );
                } else {
                    error!(
                        "Invalid Pan/Tilt position response length: {}",
                        response.len()
                    );
                }
            }
            ViscaResponseType::Luminance => {
                if let [_, _, p, q, ..] = response {
                    info!("Luminance Position: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::Contrast => {
                if let [_, _, p, q, ..] = response {
                    info!("Contrast Position: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::SharpnessMode => {
                if let [_, _, p, ..] = response {
                    info!("Sharpness Mode: {:02X}", p);
                }
            }
            ViscaResponseType::SharpnessPosition => {
                if let [_, _, p, q, ..] = response {
                    info!("Sharpness Position: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::HorizontalFlip => {
                if let [_, _, p, ..] = response {
                    info!("Horizontal Flip: {:02X}", p);
                }
            }
            ViscaResponseType::VerticalFlip => {
                if let [_, _, p, ..] = response {
                    info!("Vertical Flip: {:02X}", p);
                }
            }
            ViscaResponseType::ImageFlip => {
                if let [_, _, p, ..] = response {
                    info!("Image Flip: {:02X}", p);
                }
            }
            ViscaResponseType::BlackWhiteMode => {
                if let [_, _, p, ..] = response {
                    info!("Black & White Mode: {:02X}", p);
                }
            }
            ViscaResponseType::ExposureCompensationMode => {
                if let [_, _, p, ..] = response {
                    info!("Exposure Compensation Mode: {:02X}", p);
                }
            }
            ViscaResponseType::ExposureCompensationPosition => {
                if let [_, _, p, q, ..] = response {
                    info!("Exposure Compensation Position: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::Backlight => {
                if let [_, _, p, ..] = response {
                    info!("Backlight: {:02X}", p);
                }
            }
            ViscaResponseType::Iris => {
                if let [_, _, p, ..] = response {
                    info!("Iris Position: {:02X}", p);
                }
            }
            ViscaResponseType::Shutter => {
                if let [_, _, p, q, ..] = response {
                    info!("Shutter Position: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::GainLimit => {
                if let [_, _, p, ..] = response {
                    info!("Gain Limit Position: {:02X}", p);
                }
            }
            ViscaResponseType::AntiFlicker => {
                if let [_, _, p, ..] = response {
                    info!("Anti-Flicker: {:02X}", p);
                }
            }
            ViscaResponseType::WhiteBalanceMode => {
                if let [_, _, p, ..] = response {
                    info!("White Balance Mode: {:02X}", p);
                }
            }
            ViscaResponseType::RedTuning => {
                if let [_, _, p, q, ..] = response {
                    info!("Red Tuning: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::BlueTuning => {
                if let [_, _, p, q, ..] = response {
                    info!("Blue Tuning: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::Saturation => {
                if let [_, _, p, ..] = response {
                    info!("Saturation: {:02X}", p);
                }
            }
            ViscaResponseType::Hue => {
                if let [_, _, p, ..] = response {
                    info!("Hue: {:02X}", p);
                }
            }
            ViscaResponseType::RedGain => {
                if let [_, _, p, ..] = response {
                    info!("Red Gain: {:02X}", p);
                }
            }
            ViscaResponseType::BlueGain => {
                if let [_, _, p, ..] = response {
                    info!("Blue Gain: {:02X}", p);
                }
            }
            ViscaResponseType::ColorTemperature => {
                if let [_, _, p, q, ..] = response {
                    info!("Color Temperature: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::AutoWhiteBalanceSensitivity => {
                if let [_, _, p, ..] = response {
                    info!("Auto White Balance Sensitivity: {:02X}", p);
                }
            }
            ViscaResponseType::ThreeDNoiseReduction => {
                if let [_, _, p, ..] = response {
                    info!("3D Noise Reduction: {:02X}", p);
                }
            }
            ViscaResponseType::TwoDNoiseReduction => {
                if let [_, _, p, ..] = response {
                    info!("2D Noise Reduction: {:02X}", p);
                }
            }
            ViscaResponseType::ZoomPosition => {
                if let [_, _, p, q, r, s, ..] = response {
                    info!("Zoom Position: {:02X} {:02X} {:02X} {:02X}", p, q, r, s);
                }
            }
            ViscaResponseType::MotionSyncSpeed => {
                if let [_, _, p, ..] = response {
                    info!("Motion Sync Speed: {:02X}", p);
                }
            }
            ViscaResponseType::FocusPosition => {
                if let [_, _, p, q, r, s, ..] = response {
                    info!("Focus Position: {:02X} {:02X} {:02X} {:02X}", p, q, r, s);
                }
            }
            ViscaResponseType::FocusZone => {
                if let [_, _, p, ..] = response {
                    info!("Focus Zone: {:02X}", p);
                }
            }
            ViscaResponseType::AutoFocusSensitivity => {
                if let [_, _, p, ..] = response {
                    info!("Auto Focus Sensitivity: {:02X}", p);
                }
            }
            ViscaResponseType::FocusRange => {
                if let [_, _, p, q, ..] = response {
                    info!("Focus Range: {:02X} {:02X}", p, q);
                }
            }
            ViscaResponseType::MenuOpenClose => {
                if let [_, _, p, ..] = response {
                    info!("Menu Open/Close: {:02X}", p);
                }
            }
            ViscaResponseType::UsbAudio => {
                if let [_, _, p, ..] = response {
                    info!("USB Audio: {:02X}", p);
                }
            }
            ViscaResponseType::Rtmp => {
                if let [_, _, p, ..] = response {
                    info!("RTMP: {:02X}", p);
                }
            }
            ViscaResponseType::BlockLens => {
                info!("Block Lens Inquiry: {:02X?}", response);
            }
            ViscaResponseType::BlockColorExposure => {
                info!("Block Color & Exposure Inquiry: {:02X?}", response);
            }
            ViscaResponseType::BlockPowerImageEffect => {
                info!("Block Power & Image Effect Inquiry: {:02X?}", response);
            }
            ViscaResponseType::BlockImage => {
                info!("Block Image Inquiry: {:02X?}", response);
            }
        },
        None => error!("Unknown response command for the given command"),
    }
}
