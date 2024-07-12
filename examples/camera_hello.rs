use grafton_visca::visca_command::{PanSpeed, PanTiltDirection, TiltSpeed, ViscaCommand};
use grafton_visca::{send_command_and_wait, TcpTransport, UdpTransport, ViscaTransport};
use log::{debug, info};
use std::env;
use std::io;
use std::time::Duration;

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

    debug!("Sending Pan/Tilt home command");
    let pan_tilt_home_command = ViscaCommand::PanTiltHome;
    send_command_and_wait(&mut transport, &pan_tilt_home_command)?;

    let pan_tilt_up_command = ViscaCommand::PanTiltDrive(
        PanTiltDirection::Down,
        PanSpeed::new(0x4).unwrap(),
        TiltSpeed::new(0x4).unwrap(),
    );
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(3) {
        debug!("Sending Pan/Tilt up command");
        send_command_and_wait(&mut transport, &pan_tilt_up_command)?;
    }

    let stop_command = ViscaCommand::PanTiltDrive(
        PanTiltDirection::Stop,
        PanSpeed::new(0x00).unwrap(),
        TiltSpeed::new(0x00).unwrap(),
    );
    debug!("Sending Pan/Tilt stop command");
    send_command_and_wait(&mut transport, &stop_command)?;

    let pan_tilt_home_command = ViscaCommand::PanTiltHome;
    debug!("Sending Pan/Tilt home command");
    send_command_and_wait(&mut transport, &pan_tilt_home_command)?;

    Ok(())
}
