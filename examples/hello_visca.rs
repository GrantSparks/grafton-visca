use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
        InquiryCommand, PanTiltCommand, ZoomCommand,
    },
    send_command_and_wait, AppError, TcpTransport, UdpTransport, ViscaInquiryResponse,
    ViscaResponse, ViscaTransport,
};
use log::{debug, error, info};
use std::{env, time::Duration};

fn main() -> Result<(), AppError> {
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
    let pan_tilt_home_command = PanTiltCommand {
        direction: PanTiltDirection::Home,
        pan_speed: PanSpeed::STOP,
        tilt_speed: TiltSpeed::STOP,
    };
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    std::thread::sleep(Duration::from_secs(1));
    debug!("Inquiring Pan/Tilt position");
    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt })) =
        send_command_and_wait(&mut *transport, &InquiryCommand::PanTiltPosition)
    {
        info!("Pan position: {}, Tilt position: {}", pan, tilt);
    } else {
        error!("Failed to get Pan/Tilt position");
    }

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
        let pan_tilt_command = PanTiltCommand {
            direction: *direction,
            pan_speed: PanSpeed::new(*pan_speed).expect("Invalid Pan Speed"),
            tilt_speed: TiltSpeed::new(*tilt_speed).expect("Invalid Tilt Speed"),
        };
        send_command_and_wait(&mut *transport, &pan_tilt_command)?;

        std::thread::sleep(Duration::from_secs(3));
        debug!("Sending Pan/Tilt stop command");
        let pan_tilt_stop_command = PanTiltCommand {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::STOP,
            tilt_speed: TiltSpeed::STOP,
        };
        send_command_and_wait(&mut *transport, &pan_tilt_stop_command)?;
        std::thread::sleep(Duration::from_secs(1));
    }

    debug!("Inquiring Pan/Tilt position");
    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt })) =
        send_command_and_wait(&mut *transport, &InquiryCommand::PanTiltPosition)
    {
        info!("Pan position: {}, Tilt position: {}", pan, tilt);
    } else {
        error!("Failed to get Pan/Tilt position");
    }

    debug!("Inquiring initial Zoom position");
    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position })) =
        send_command_and_wait(&mut *transport, &InquiryCommand::ZoomPosition)
    {
        info!("Initial Zoom position: {}", position);
    } else {
        error!("Failed to get initial Zoom position");
    }

    let zoom_movements = [
        ZoomCommand::TeleStandard,
        ZoomCommand::WideStandard,
        ZoomCommand::TeleVariable(5),
        ZoomCommand::WideVariable(5),
    ];

    for command in zoom_movements.iter() {
        debug!("Sending {:?} command", command);
        if let Err(e) = send_command_and_wait(&mut *transport, command) {
            error!("Error while sending zoom command: {:?}", e);
            return Err(AppError::Visca(e));
        }

        std::thread::sleep(Duration::from_secs(3));

        debug!("Inquiring Zoom position after {:?}", command);
        if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position })) =
            send_command_and_wait(&mut *transport, &InquiryCommand::ZoomPosition)
        {
            info!("Zoom position after {:?}: {}", command, position);
        } else {
            error!("Failed to get Zoom position after {:?}", command);
        }
    }

    debug!("Sending Zoom stop command");
    send_command_and_wait(&mut *transport, &ZoomCommand::Stop)?;

    debug!("Inquiring final Zoom position");
    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position })) =
        send_command_and_wait(&mut *transport, &InquiryCommand::ZoomPosition)
    {
        info!("Final Zoom position: {}", position);
    } else {
        error!("Failed to get final Zoom position");
    }

    debug!("Sending Pan/Tilt home command");
    send_command_and_wait(&mut *transport, &pan_tilt_home_command)?;

    debug!("Sending Zoom home command");
    send_command_and_wait(&mut *transport, &ZoomCommand::WideStandard)?;

    Ok(())
}
