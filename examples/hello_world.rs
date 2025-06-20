//! Hello world example demonstrating both blocking and async approaches.
//!
//! This example shows how the library supports both blocking-first and async APIs.

use grafton_visca::{
    camera::profiles::PTZOpticsG2, command::pan_tilt::PanTiltDirection, Camera, Error,
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

#[cfg(not(feature = "async"))]
mod blocking_impl {
    use super::*;
    use grafton_visca::transport::blocking::create;
    use std::thread;

    pub fn create_camera(
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
            let transport = create::udp(&address)?;
            Ok(Camera::new(transport))
        } else {
            let transport = create::tcp(&address)?;
            Ok(Camera::new(transport))
        }
    }

    pub fn perform_pan_tilt_movements(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
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
            camera.move_continuous(*direction, *pan_speed, *tilt_speed)?;

            thread::sleep(Duration::from_secs(3));

            debug!("Sending Pan/Tilt stop command");
            camera.stop()?;

            thread::sleep(Duration::from_secs(1));
        }

        Ok(())
    }

    pub fn perform_zoom_movements(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        debug!("Zooming in (standard speed)");
        camera.zoom_in()?;
        thread::sleep(Duration::from_secs(3));

        debug!("Stopping zoom");
        camera.zoom_stop()?;
        thread::sleep(Duration::from_secs(1));

        debug!("Zooming out (standard speed)");
        camera.zoom_out()?;
        thread::sleep(Duration::from_secs(3));

        debug!("Stopping zoom");
        camera.zoom_stop()?;

        // Set zoom to specific position (50%)
        debug!("Setting zoom to 50%");
        camera.set_zoom(0x3800u16)?; // Half of max zoom for G2
        thread::sleep(Duration::from_secs(2));

        Ok(())
    }

    pub fn run_blocking() -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting grafton-visca hello_world example (blocking mode)");

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
        camera.home()?;
        thread::sleep(Duration::from_secs(2));

        perform_pan_tilt_movements(&mut camera)?;

        perform_zoom_movements(&mut camera)?;

        debug!("Returning to home position");
        camera.home()?;
        thread::sleep(Duration::from_secs(2));

        debug!("Resetting zoom");
        camera.set_zoom(0x0000u16)?; // Minimum zoom

        info!("Demo complete!");
        Ok(())
    }
}

#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use grafton_visca::transport::create;

    pub async fn create_camera(
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
            let transport = create::udp(&address).await?;
            Ok(Camera::new(transport))
        } else {
            let transport = create::tcp(&address).await?;
            Ok(Camera::new(transport))
        }
    }

    pub async fn perform_pan_tilt_movements(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
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
            camera
                .move_continuous(*direction, *pan_speed, *tilt_speed)
                .await?;

            tokio::time::sleep(Duration::from_secs(3)).await;

            debug!("Sending Pan/Tilt stop command");
            camera.stop().await?;

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }

    pub async fn perform_zoom_movements(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        debug!("Zooming in (standard speed)");
        camera.zoom_in().await?;
        tokio::time::sleep(Duration::from_secs(3)).await;

        debug!("Stopping zoom");
        camera.zoom_stop().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        debug!("Zooming out (standard speed)");
        camera.zoom_out().await?;
        tokio::time::sleep(Duration::from_secs(3)).await;

        debug!("Stopping zoom");
        camera.zoom_stop().await?;

        // Set zoom to specific position (50%)
        debug!("Setting zoom to 50%");
        camera.set_zoom(0x3800).await?; // Half of max zoom for G2
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(())
    }

    pub async fn run_async() -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting grafton-visca hello_world example (async mode)");

        let (protocol, ip_address) = parse_args();
        let camera = create_camera(&protocol, &ip_address).await?;

        // Display camera capabilities
        let caps = camera.capabilities();
        info!("Camera Model: {}", caps.model_name);
        info!("Pan Range: {:?} degrees", caps.pan_range_degrees);
        info!("Tilt Range: {:?} degrees", caps.tilt_range_degrees);
        info!("Max Pan Speed: {}", caps.max_pan_speed);
        info!("Max Tilt Speed: {}", caps.max_tilt_speed);

        debug!("Sending Pan/Tilt home command");
        camera.home().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        perform_pan_tilt_movements(&camera).await?;

        perform_zoom_movements(&camera).await?;

        debug!("Returning to home position");
        camera.home().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        debug!("Resetting zoom");
        camera.set_zoom(0x0000).await?; // Minimum zoom

        info!("Demo complete!");
        Ok(())
    }
}

// Main function that works for both blocking and async modes
#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    // Run async version when both features are enabled
    async_impl::run_async().await
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    // Run blocking version when only blocking is enabled
    blocking_impl::run_blocking()
}
