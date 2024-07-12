use grafton_visca::visca_command::{PanTiltDirection, ViscaCommand};
use log::{debug, error, info};
use std::env;
use std::{
    io::{self, Read as _, Write as _},
    net::{TcpStream, UdpSocket},
    time::Duration,
};

trait ViscaTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> io::Result<()>;
}

struct UdpTransport {
    socket: UdpSocket,
    address: String,
}

impl UdpTransport {
    fn new(address: &str) -> io::Result<Self> {
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

        let mut buffer = [0u8; 1024];
        match self.socket.recv_from(&mut buffer) {
            Ok((bytes_received, src)) => {
                info!(
                    "Received {} bytes from {}: {:02X?}",
                    bytes_received,
                    src,
                    &buffer[..bytes_received]
                );
                display_camera_response(&buffer[..bytes_received]);
            }
            Err(e) => {
                error!("Failed to receive response: {}", e);
            }
        }
        Ok(())
    }
}

struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    fn new(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self { stream })
    }
}

impl ViscaTransport for TcpTransport {
    fn send_command(&mut self, command: &ViscaCommand) -> io::Result<()> {
        let command_bytes = command.to_bytes();
        debug!("Command bytes: {:02X?}", command_bytes);
        self.stream.write_all(&command_bytes)?;
        info!("Sent {} bytes", command_bytes.len());

        let mut buffer = [0u8; 1024];
        match self.stream.read(&mut buffer) {
            Ok(bytes_received) => {
                info!(
                    "Received {} bytes: {:02X?}",
                    bytes_received,
                    &buffer[..bytes_received]
                );
                display_camera_response(&buffer[..bytes_received]);
            }
            Err(e) => {
                error!("Failed to receive response: {}", e);
            }
        }
        Ok(())
    }
}

fn display_camera_response(response: &[u8]) {
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

fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting application");

    let args: Vec<String> = env::args().collect();
    let protocol = if args.len() > 1 { &args[1] } else { "udp" };

    let udp_address = "192.168.0.110:1259";
    let tcp_address = "192.168.0.110:5678";

    let use_udp = protocol.eq_ignore_ascii_case("udp");

    let mut transport: Box<dyn ViscaTransport> = if use_udp {
        Box::new(UdpTransport::new(udp_address)?)
    } else {
        Box::new(TcpTransport::new(tcp_address)?)
    };

    let pan_tilt_up_command = ViscaCommand::PanTiltDrive(PanTiltDirection::Up, 0x01, 0x01);
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(3) {
        match transport.send_command(&pan_tilt_up_command) {
            Ok(_) => info!("Pan up command sent successfully."),
            Err(e) => error!("Failed to send pan up command: {}", e),
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let stop_command = ViscaCommand::PanTiltDrive(PanTiltDirection::Stop, 0x00, 0x00);
    match transport.send_command(&stop_command) {
        Ok(_) => info!("Stop command sent successfully."),
        Err(e) => error!("Failed to send stop command: {}", e),
    }

    let pan_tilt_home_command = ViscaCommand::PanTiltHome;
    match transport.send_command(&pan_tilt_home_command) {
        Ok(_) => info!("Pan tilt home command sent successfully."),
        Err(e) => error!("Failed to send pan tilt home command: {}", e),
    }

    Ok(())
}
