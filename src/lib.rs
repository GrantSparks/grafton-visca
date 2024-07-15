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
                    display_camera_response(&response);

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

pub fn display_camera_response(response: &[u8]) {
    if response.len() < 3 {
        error!("Unexpected response format: {:02X?}", response);
        return;
    }

    match response {
        // Handle ACK/Completion and Error Messages
        [0x90, code, 0xFF] if (0x40..=0x4F).contains(code) => {
            info!("ACK: Command accepted with status code: {:02X?}", code);
        }
        [0x90, code, 0xFF] if (0x50..=0x5F).contains(code) => {
            info!(
                "Completion: Command executed with status code: {:02X?}",
                code
            );
        }
        [0x90, 0x60, 0x02, 0xFF] => {
            error!("Error: Syntax Error.");
        }
        [0x90, 0x60, 0x03, 0xFF] => {
            error!("Error: Command Buffer Full.");
        }
        [0x90, code, 0x04, 0xFF] if (0x60..=0x6F).contains(code) => {
            error!("Error: Command Canceled.");
        }
        [0x90, code, 0x05, 0xFF] if (0x60..=0x6F).contains(code) => {
            error!("Error: No Socket.");
        }
        [0x90, code, 0x41, 0xFF] if (0x60..=0x6F).contains(code) => {
            error!("Error: Command Not Executable.");
        }
        // Handle Inquiry Responses
        [0x90, 0x50, rest @ .., 0xFF] => {
            if let Some((&cmd, data)) = rest.split_first() {
                let response_type = ViscaResponseType::from_bytes(cmd, None);

                match response_type {
                    Some(ViscaResponseType::Luminance) => {
                        if let [p, q] = data {
                            info!("Luminance Position: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::Contrast) => {
                        if let [p, q] = data {
                            info!("Contrast Position: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::SharpnessMode) => {
                        if let [p] = data {
                            info!("Sharpness Mode: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::SharpnessPosition) => {
                        if let [p] = data {
                            info!("Sharpness Position: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::HorizontalFlip) => {
                        if let [p] = data {
                            info!("Horizontal Flip: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::VerticalFlip) => {
                        if let [p] = data {
                            info!("Vertical Flip: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::ImageFlip) => {
                        if let [p] = data {
                            info!("Image Flip: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::BlackWhiteMode) => {
                        if let [p] = data {
                            info!("Black & White Mode: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::ExposureCompensationMode) => {
                        if let [p] = data {
                            info!("Exposure Compensation Mode: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::ExposureCompensationPosition) => {
                        if let [p, q] = data {
                            info!("Exposure Compensation Position: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::Backlight) => {
                        if let [p] = data {
                            info!("Backlight: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::Iris) => {
                        if let [p] = data {
                            info!("Iris Position: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::Shutter) => {
                        if let [p, q] = data {
                            info!("Shutter Position: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::GainLimit) => {
                        if let [p] = data {
                            info!("Gain Limit Position: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::AntiFlicker) => {
                        if let [p] = data {
                            info!("Anti-Flicker: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::WhiteBalanceMode) => {
                        if let [p] = data {
                            info!("White Balance Mode: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::RedTuning) => {
                        if let [p, q] = data {
                            info!("Red Tuning: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::BlueTuning) => {
                        if let [p, q] = data {
                            info!("Blue Tuning: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::Saturation) => {
                        if let [p] = data {
                            info!("Saturation: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::Hue) => {
                        if let [p] = data {
                            info!("Hue: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::RedGain) => {
                        if let [p] = data {
                            info!("Red Gain: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::BlueGain) => {
                        if let [p] = data {
                            info!("Blue Gain: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::ColorTemperature) => {
                        if let [p, q] = data {
                            info!("Color Temperature: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::AutoWhiteBalanceSensitivity) => {
                        if let [p] = data {
                            info!("Auto White Balance Sensitivity: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::ThreeDNoiseReduction) => {
                        if let [p] = data {
                            info!("3D Noise Reduction: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::TwoDNoiseReduction) => {
                        if let [p] = data {
                            info!("2D Noise Reduction: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::PanTiltPosition) => {
                        if data.len() == 8 {
                            info!(
                                "Pan Tilt Position: Pan={:02X?}{:02X?}{:02X?}{:02X?}, Tilt={:02X?}{:02X?}{:02X?}{:02X?}",
                                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
                            );
                        }
                    }
                    Some(ViscaResponseType::ZoomPosition) => {
                        if let [p] = data {
                            info!("Zoom Position: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::MotionSyncSpeed) => {
                        if let [p] = data {
                            info!("Motion Sync Speed: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::FocusPosition) => {
                        if let [p1, p2] = data {
                            info!("Focus Position: {:02X?} {:02X?}", p1, p2);
                        }
                    }
                    Some(ViscaResponseType::FocusZone) => {
                        if let [p] = data {
                            info!("Focus Zone: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::AutoFocusSensitivity) => {
                        if let [p] = data {
                            info!("Auto Focus Sensitivity: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::FocusRange) => {
                        if let [p, q] = data {
                            info!("Focus Range: {:02X?} {:02X?}", p, q);
                        }
                    }
                    Some(ViscaResponseType::MenuOpenClose) => {
                        if let [p] = data {
                            info!("Menu Open/Close: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::UsbAudio) => {
                        if let [p] = data {
                            info!("USB Audio: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::Rtmp) => {
                        if let [p] = data {
                            info!("RTMP: {:02X?}", p);
                        }
                    }
                    Some(ViscaResponseType::BlockLens) => {
                        info!("Block Lens Inquiry: {:02X?}", data);
                    }
                    Some(ViscaResponseType::BlockColorExposure) => {
                        info!("Block Color & Exposure Inquiry: {:02X?}", data);
                    }
                    Some(ViscaResponseType::BlockPowerImageEffect) => {
                        info!("Block Power & Image Effect Inquiry: {:02X?}", data);
                    }
                    Some(ViscaResponseType::BlockImage) => {
                        info!("Block Image Inquiry: {:02X?}", data);
                    }
                    _ => {
                        error!("Unknown inquiry response command: {:02X?} {:?}", cmd, data);
                    }
                }
            } else {
                error!("Unknown inquiry response format: {:02X?}", response);
            }
        }
        _ => {
            error!("Unexpected response format: {:02X?}", response);
        }
    }
}
