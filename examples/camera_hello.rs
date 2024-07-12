use log::{debug, error, info};
use std::io::{self, Read as _, Write as _};
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

use grafton_visca::visca_command::{PanTiltDirection, ViscaCommand};

enum Protocol {
    UDP,
    TCP,
}

fn send_command(command: &ViscaCommand, address: &str, protocol: Protocol) -> io::Result<()> {
    match protocol {
        Protocol::UDP => {
            debug!("Binding UDP socket...");
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            debug!("UDP socket bound successfully.");

            // Set timeouts
            socket.set_read_timeout(Some(Duration::from_secs(10)))?;
            socket.set_write_timeout(Some(Duration::from_secs(10)))?;

            let command_bytes = command.to_bytes();
            debug!("Command bytes: {:02X?}", command_bytes);

            debug!("Sending command to address: {}", address);
            match socket.send_to(&command_bytes, address) {
                Ok(bytes_sent) => {
                    info!("Sent {} bytes to {}", bytes_sent, address);

                    let mut buffer = [0u8; 1024];
                    match socket.recv_from(&mut buffer) {
                        Ok((bytes_received, src)) => {
                            info!(
                                "Received {} bytes from {}: {:02X?}",
                                bytes_received,
                                src,
                                &buffer[..bytes_received]
                            );
                        }
                        Err(e) => {
                            error!("Failed to receive response: {}", e);
                        }
                    }

                    Ok(())
                }
                Err(e) => {
                    error!("Failed to send command: {}", e);
                    Err(e)
                }
            }
        }
        Protocol::TCP => {
            debug!("Connecting to TCP socket...");
            let mut stream = TcpStream::connect(address)?;
            debug!("TCP socket connected successfully.");

            // Set timeouts
            stream.set_read_timeout(Some(Duration::from_secs(10)))?;
            stream.set_write_timeout(Some(Duration::from_secs(10)))?;

            let command_bytes = command.to_bytes();
            debug!("Command bytes: {:02X?}", command_bytes);

            debug!("Sending command to address: {}", address);
            stream.write_all(&command_bytes)?;
            info!("Sent {} bytes to {}", command_bytes.len(), address);

            let mut buffer = [0u8; 1024];
            match stream.read(&mut buffer) {
                Ok(bytes_received) => {
                    info!(
                        "Received {} bytes: {:02X?}",
                        bytes_received,
                        &buffer[..bytes_received]
                    );
                }
                Err(e) => {
                    error!("Failed to receive response: {}", e);
                }
            }

            Ok(())
        }
    }
}

fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    info!("Starting application");

    let command = ViscaCommand::PanTiltDrive(PanTiltDirection::Up, 0x18, 0x14); // Example command
    debug!("Created command: {:?}", command);

    match send_command(&command, "192.168.0.110:1259", Protocol::UDP) {
        //    match send_command(&command, "192.168.0.110:52381", Protocol::UDP) { // Sony Visca
        Ok(_) => info!("Command sent successfully."),
        Err(e) => error!("Failed to send command: {}", e),
    }

    //    match send_command(&command, "192.168.0.110:5678", Protocol::TCP) {
        match send_command(&command, "192.168.0.110:80", Protocol::TCP) {
//    match send_command(&command, "192.168.0.110:52381", Protocol::TCP) { // Sony Visca
        Ok(_) => info!("Command sent successfully."),
        Err(e) => error!("Failed to send command: {}", e),
    }

    Ok(())
}
