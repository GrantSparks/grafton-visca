use grafton_visca::visca_command::{PanSpeed, PanTiltDirection, TiltSpeed, ViscaCommand};
use grafton_visca::{send_command_and_wait, TcpTransport, UdpTransport, ViscaTransport};
use log::{debug, info};
use std::env;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting application");

    let default_protocol = "udp";
    let default_ip_address = "192.168.0.110";

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
    let address = if use_udp {
        format!("{}:{}", ip_address, udp_port)
    } else {
        format!("{}:{}", ip_address, tcp_port)
    };

    let mut transport: Box<dyn ViscaTransport> = if use_udp {
        Box::new(UdpTransport::new(&address)?)
    } else {
        Box::new(TcpTransport::new(&address)?)
    };

    debug!("Sending Pan/Tilt home command");
    let pan_tilt_home_command = ViscaCommand::PanTiltHome;
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    std::thread::sleep(Duration::from_secs(1));
    debug!("Inquiring Pan/Tilt position");
    send_command_and_wait(&mut *transport, &ViscaCommand::InquiryPanTiltPosition)?;

    let complex_movements = [
        (PanTiltDirection::Up, 5, 3),
        (PanTiltDirection::Right, 4, 3),
        (PanTiltDirection::Down, 4, 2),
        (PanTiltDirection::Left, 4, 3),
        (PanTiltDirection::UpLeft, 3, 3),
        (PanTiltDirection::DownRight, 3, 3),
    ];

    for (direction, pan_speed, tilt_speed) in complex_movements.iter() {
        debug!("Sending Pan/Tilt {:?} command", direction);
        let pan_tilt_command = ViscaCommand::PanTiltDrive(
            *direction,
            PanSpeed::new(*pan_speed).expect("Invalid Pan Speed"),
            TiltSpeed::new(*tilt_speed).expect("Invalid Tilt Speed"),
        );
        send_command_and_wait(&mut *transport, &pan_tilt_command)?;

        std::thread::sleep(Duration::from_secs(3));
        debug!("Sending Pan/Tilt stop command");
        let pan_tilt_stop_command =
            ViscaCommand::PanTiltDrive(PanTiltDirection::Stop, PanSpeed::STOP, TiltSpeed::STOP);
        send_command_and_wait(&mut *transport, &pan_tilt_stop_command)?;
        std::thread::sleep(Duration::from_secs(1));
    }

    debug!("Inquiring Pan/Tilt position");
    send_command_and_wait(&mut *transport, &ViscaCommand::InquiryPanTiltPosition)?;

    debug!("Inquiring Zoom position");
    send_command_and_wait(&mut *transport, &ViscaCommand::InquiryZoomPosition)?;

    let zoom_movements = [
        ViscaCommand::ZoomTeleStandard,
        ViscaCommand::ZoomWideStandard,
        ViscaCommand::ZoomTeleAdjustableSpeed(5),
        ViscaCommand::ZoomWideAdjustableSpeed(5),
    ];

    for command in zoom_movements.iter() {
        debug!("Sending {:?} command", command);
        send_command_and_wait(&mut *transport, command)?;
        std::thread::sleep(Duration::from_secs(3));
    }

    debug!("Sending Zoom stop command");
    send_command_and_wait(&mut *transport, &ViscaCommand::ZoomStop)?;

    debug!("Inquiring Zoom position");
    send_command_and_wait(&mut *transport, &ViscaCommand::InquiryZoomPosition)?;

    debug!("Sending Pan/Tilt home command");
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    debug!("Sending Zoom home command");
    send_command_and_wait(&mut *transport, &ViscaCommand::ZoomWideStandard)?;

    Ok(())
}
