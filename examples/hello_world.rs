//! Example program demonstrating the new Camera API

use grafton_visca::{
    camera::profiles::PTZOpticsG2,
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, TcpTransport, UdpTransport},
    Camera, Error,
};
use log::{debug, info};
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

fn create_camera(
    protocol: &str,
    ip_address: &str,
) -> Result<Camera<PTZOpticsG2>, Box<dyn std::error::Error>> {
    let udp_port = "1259";
    let tcp_port = "5678";

    let use_udp = protocol.eq_ignore_ascii_case("udp");
    let address = if use_udp {
        format!("{ip_address}:{udp_port}")
    } else {
        format!("{ip_address}:{tcp_port}")
    };

    if use_udp {
        let udp_transport = UdpTransport::new(&address)?;
        Ok(Camera::new(BlockingAdapter(udp_transport)))
    } else {
        let tcp_transport = TcpTransport::new(&address)?;
        Ok(Camera::new(BlockingAdapter(tcp_transport)))
    }
}

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn perform_pan_tilt_movements(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
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
        block_on(camera.move_continuous(*direction, *pan_speed, *tilt_speed))?;

        std::thread::sleep(Duration::from_secs(3));

        debug!("Sending Pan/Tilt stop command");
        block_on(camera.stop())?;

        std::thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}

fn perform_zoom_movements(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    debug!("Zooming in (standard speed)");
    block_on(camera.zoom_in())?;
    std::thread::sleep(Duration::from_secs(3));

    debug!("Stopping zoom");
    block_on(camera.zoom_stop())?;
    std::thread::sleep(Duration::from_secs(1));

    debug!("Zooming out (standard speed)");
    block_on(camera.zoom_out())?;
    std::thread::sleep(Duration::from_secs(3));

    debug!("Stopping zoom");
    block_on(camera.zoom_stop())?;

    // Set zoom to specific position (50%)
    debug!("Setting zoom to 50%");
    let zoom_50_percent = 0x3800; // Half of max zoom for G2
    block_on(camera.set_zoom(zoom_50_percent))?;
    std::thread::sleep(Duration::from_secs(2));

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting grafton-visca hello_world example");

    let (protocol, ip_address) = parse_args();
    let mut camera = create_camera(&protocol, &ip_address)?;

    // Display camera capabilities
    let caps = camera.capabilities();
    info!("Camera Model: {}", caps.model_name);
    info!("Pan Range: {:?} degrees", caps.pan_range_degrees);
    info!("Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    info!("Max Pan Speed: {}", caps.max_pan_speed);
    info!("Max Tilt Speed: {}", caps.max_tilt_speed);

    debug!("Sending Pan/Tilt home command");
    block_on(camera.home())?;
    std::thread::sleep(Duration::from_secs(2));

    perform_pan_tilt_movements(&mut camera)?;

    perform_zoom_movements(&mut camera)?;

    debug!("Returning to home position");
    block_on(camera.home())?;
    std::thread::sleep(Duration::from_secs(2));

    debug!("Resetting zoom");
    block_on(camera.set_zoom(0x0000))?;

    info!("Demo complete!");
    Ok(())
}
