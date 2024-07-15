use grafton_visca::visca_command::{PanSpeed, PanTiltDirection, TiltSpeed, ViscaCommand};
use grafton_visca::{send_command_and_wait, TcpTransport, UdpTransport, ViscaTransport};
use log::{debug, info};
use std::env;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting application");

    // Default values
    let default_protocol = "udp";
    let default_ip_address = "192.168.0.110";

    // Collect command-line arguments
    let args: Vec<String> = env::args().collect();
    let protocol = if args.len() > 1 {
        &args[1]
    } else {
        default_protocol
    };
    let ip_address = if args.len() > 2 {
        &args[2]
    } else {
        default_ip_address
    };

    let udp_port = "1259";
    let tcp_port = "5678";

    let use_udp = protocol.eq_ignore_ascii_case("udp");

    // Determine the full address based on the protocol
    let address = if use_udp {
        format!("{}:{}", ip_address, udp_port)
    } else {
        format!("{}:{}", ip_address, tcp_port)
    };

    // Initialize the appropriate transport
    let mut transport: Box<dyn ViscaTransport> = if use_udp {
        Box::new(UdpTransport::new(&address)?)
    } else {
        Box::new(TcpTransport::new(&address)?)
    };

    // Send Pan/Tilt home command
    debug!("Sending Pan/Tilt home command");
    let pan_tilt_home_command = ViscaCommand::PanTiltHome;
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    // Sleep for 1 second
    std::thread::sleep(Duration::from_secs(1));

    // Inquire Pan/Tilt position
    debug!("Inquiring Pan/Tilt position");
    let inquire_pan_tilt_position_command = ViscaCommand::InquiryPanTiltPosition;
    send_command_and_wait(&mut *transport, &inquire_pan_tilt_position_command)?;    

    // Send Pan/Tilt up command
    let pan_tilt_up_command = ViscaCommand::PanTiltDrive(
        PanTiltDirection::Up,
        PanSpeed::new(0x8).expect("Invalid Pan Speed"),
        TiltSpeed::new(0x8).expect("Invalid Tilt Speed"),
    );

    debug!("Sending Pan/Tilt up command");
    send_command_and_wait(&mut *transport, &pan_tilt_up_command)?;

    // Sleep for 3 seconds
    std::thread::sleep(Duration::from_secs(3));

    // Send Pan/Tilt stop command
    let stop_command = ViscaCommand::PanTiltDrive(
        PanTiltDirection::Stop,
        PanSpeed::new(0x00).expect("Invalid Pan Speed"),
        TiltSpeed::new(0x00).expect("Invalid Tilt Speed"),
    );
    debug!("Sending Pan/Tilt stop command");
    send_command_and_wait(&mut *transport, &stop_command)?;

    // Sleep for 1 second
    std::thread::sleep(Duration::from_secs(1));

    // Inquire Pan/Tilt position
    debug!("Inquiring Pan/Tilt position");
    let inquire_pan_tilt_position_command = ViscaCommand::InquiryPanTiltPosition;
    send_command_and_wait(&mut *transport, &inquire_pan_tilt_position_command)?;    


    // Send Pan/Tilt home command
    debug!("Sending Pan/Tilt home command");
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    Ok(())
}
