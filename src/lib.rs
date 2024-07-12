pub mod visca_command;
use log::{debug, error, info};
use std::{
    io::{self, Read as _, Write as _},
    net::{TcpStream, UdpSocket},
    time::Duration,
};
use visca_command::ViscaCommand;

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
        match self.socket.recv_from(&mut buffer) {
            Ok((bytes_received, src)) => {
                info!(
                    "Received {} bytes from {}: {:02X?}",
                    bytes_received,
                    src,
                    &buffer[..bytes_received]
                );

                let mut responses = Vec::new();
                let mut start_index = None;
                let mut response = Vec::new();

                for &byte in &buffer[..bytes_received] {
                    if byte == 0x90 {
                        if let Some(start) = start_index {
                            responses.push(response.split_off(start));
                        }
                        start_index = Some(response.len());
                    }
                    response.push(byte);
                    if byte == 0xFF {
                        if let Some(start) = start_index {
                            responses.push(response.split_off(start));
                            start_index = None;
                        }
                    }
                }

                if start_index.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Incomplete response received",
                    ));
                }

                Ok(responses)
            }
            Err(e) => {
                error!("Failed to receive response: {}", e);
                Err(e)
            }
        }
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
        match self.stream.read(&mut buffer) {
            Ok(bytes_received) => {
                info!(
                    "Received {} bytes: {:02X?}",
                    bytes_received,
                    &buffer[..bytes_received]
                );

                let mut responses = Vec::new();
                let mut start_index = None;
                let mut response = Vec::new();

                for &byte in &buffer[..bytes_received] {
                    if byte == 0x90 {
                        if let Some(start) = start_index {
                            responses.push(response.split_off(start));
                        }
                        start_index = Some(response.len());
                    }
                    response.push(byte);
                    if byte == 0xFF {
                        if let Some(start) = start_index {
                            responses.push(response.split_off(start));
                            start_index = None;
                        }
                    }
                }

                if start_index.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Incomplete response received",
                    ));
                }

                Ok(responses)
            }
            Err(e) => {
                error!("Failed to receive response: {}", e);
                Err(e)
            }
        }
    }
}

pub fn send_command_and_wait(
    transport: &mut Box<dyn ViscaTransport>,
    command: &ViscaCommand,
) -> io::Result<()> {
    transport.send_command(command)?;

    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    display_camera_response(&response);
                    // Check if the response is 4 bytes or, if it has 3 bytes, check whether the second byte is in the range 0x50 to 0x5F
                    if response.len() == 4
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
    let mut i = 0;
    while i < response.len() {
        if response[i] == 0x90 {
            match response.get(i..i + 3) {
                Some([0x90, code, 0xFF]) if (0x40..=0x4F).contains(code) => {
                    info!("ACK: Command accepted with status code: {:02X?}", code);
                    i += 3;
                }
                Some([0x90, code, 0xFF]) if (0x50..=0x5F).contains(code) => {
                    info!(
                        "Completion: Command executed with status code: {:02X?}",
                        code
                    );
                    i += 3;
                }
                Some([0x90, 0x60, 0x02, 0xFF]) => {
                    error!("Error: Syntax Error.");
                    i += 4;
                }
                Some([0x90, 0x60, 0x03, 0xFF]) => {
                    error!("Error: Command Buffer Full.");
                    i += 4;
                }
                Some([0x90, code, 0x04, 0xFF]) if (0x60..=0x6F).contains(code) => {
                    error!("Error: Command Canceled.");
                    i += 4;
                }
                Some([0x90, code, 0x05, 0xFF]) if (0x60..=0x6F).contains(code) => {
                    error!("Error: No Socket.");
                    i += 4;
                }
                Some([0x90, code, 0x41, 0xFF]) if (0x60..=0x6F).contains(code) => {
                    error!("Error: Command Not Executable.");
                    i += 4;
                }
                _ => {
                    error!("Unknown response: {:02X?}", &response[i..i + 3]);
                    i += 3;
                }
            }
        } else {
            error!("Unexpected response format: {:02X?}", &response[i..]);
            break;
        }
    }
}
