use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
        zoom::{ZoomCommand, ZoomSpeed},
        InquiryCommand, PanTiltCommand,
    },
    AppError, Client, InquiryResponse, Response,
};
use log::{debug, error, info};
use std::{env, time::Duration};

fn parse_args() -> (String, String) {
    let default_protocol = "udp";
    let default_ip_address = "192.168.0.110";

    let args: Vec<String> = env::args().collect();
    let protocol = if args.len() > 1 {
        args[1].clone()
    } else {
        default_protocol.to_string()
    };
    let ip_address = if args.len() > 2 {
        args[2].clone()
    } else {
        default_ip_address.to_string()
    };

    (protocol, ip_address)
}

fn create_client(protocol: &str, ip_address: &str) -> Result<Client, AppError> {
    let udp_port = "1259";
    let tcp_port = "5678";

    let use_udp = protocol.eq_ignore_ascii_case("udp");
    let address = if use_udp {
        format!("{ip_address}:{udp_port}")
    } else {
        format!("{ip_address}:{tcp_port}")
    };

    if use_udp {
        Client::connect_udp(&address)
    } else {
        Client::connect_tcp(&address)
    }
    .map_err(AppError::from)
}

fn perform_pan_tilt_movements(client: &Client) -> Result<(), AppError> {
    let complex_movements = [
        (PanTiltDirection::Up, 5, 3),
        (PanTiltDirection::Right, 4, 3),
        (PanTiltDirection::Down, 4, 2),
        (PanTiltDirection::Left, 4, 3),
        (PanTiltDirection::UpLeft, 3, 3),
        (PanTiltDirection::DownRight, 3, 3),
    ];

    for (direction, pan_speed, tilt_speed) in &complex_movements {
        debug!("Sending Pan/Tilt {direction:?} command");
        let pan_tilt_command = PanTiltCommand::Move {
            direction: *direction,
            pan_speed: PanSpeed::new(*pan_speed)?,
            tilt_speed: TiltSpeed::new(*tilt_speed)?,
        };
        client.send(&pan_tilt_command)?;

        std::thread::sleep(Duration::from_secs(3));

        debug!("Sending Pan/Tilt stop command");
        let pan_tilt_stop_command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0x00)?,
            tilt_speed: TiltSpeed::new(0x00)?,
        };
        client.send(&pan_tilt_stop_command)?;

        std::thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}

fn perform_zoom_movements(client: &Client) -> Result<(), AppError> {
    let zoom_movements = [
        ZoomCommand::ZoomInStandard,
        ZoomCommand::ZoomOutStandard,
        ZoomCommand::ZoomInVariable(ZoomSpeed::new(5).unwrap()),
        ZoomCommand::ZoomOutVariable(ZoomSpeed::new(5).unwrap()),
    ];

    for command in &zoom_movements {
        debug!("Sending {command:?} command");
        if let Err(e) = client.send(command) {
            error!("Error while sending zoom command: {e:?}");
            return Err(AppError::Visca(e));
        }

        std::thread::sleep(Duration::from_secs(3));

        debug!("Inquiring Zoom position after {command:?}");
        if let Ok(Response::InquiryResponse(InquiryResponse::ZoomPosition { position })) =
            client.send(&InquiryCommand::ZoomPosition)
        {
            info!("Zoom position after {command:?}: {position}");
        } else {
            error!("Failed to get Zoom position after {command:?}");
        }
    }

    debug!("Sending Zoom stop command");
    client.send(&ZoomCommand::Stop)?;

    Ok(())
}

fn inquire_pan_tilt_position(client: &Client) {
    debug!("Inquiring Pan/Tilt position");
    if let Ok(Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt })) =
        client.send(&InquiryCommand::PanTiltPosition)
    {
        info!("Pan position: {pan}, Tilt position: {tilt}");
    } else {
        error!("Failed to get Pan/Tilt position");
    }
}

fn inquire_zoom_position(client: &Client, label: &str) {
    debug!("Inquiring {label} Zoom position");
    if let Ok(Response::InquiryResponse(InquiryResponse::ZoomPosition { position })) =
        client.send(&InquiryCommand::ZoomPosition)
    {
        info!("{label} Zoom position: {position}");
    } else {
        error!("Failed to get {label} Zoom position");
    }
}

fn main() -> Result<(), AppError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting application");

    let (protocol, ip_address) = parse_args();
    let client = create_client(&protocol, &ip_address)?;

    debug!("Sending Pan/Tilt home command");
    client.send(&PanTiltCommand::Home)?;
    std::thread::sleep(Duration::from_secs(1));

    inquire_pan_tilt_position(&client);

    perform_pan_tilt_movements(&client)?;

    inquire_pan_tilt_position(&client);

    inquire_zoom_position(&client, "initial");

    perform_zoom_movements(&client)?;

    inquire_zoom_position(&client, "final");

    debug!("Sending Pan/Tilt home command");
    client.send(&PanTiltCommand::Home)?;

    debug!("Sending Zoom home command");
    client.send(&ZoomCommand::ZoomOutStandard)?;

    Ok(())
}
