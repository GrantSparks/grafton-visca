//! Hello world example demonstrating the async API.
//!
//! This example shows basic camera control operations.

use grafton_visca::{
    camera::profiles::PTZOpticsG2, command::pan_tilt::PanTiltDirection, Camera, Error,
};
use log::{debug, info};
use std::{env, time::Duration};

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::time::sleep;

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    info!("Starting {} Hello World example...", env!("CARGO_PKG_NAME"));
    info!("Connecting to camera at {}", camera_addr);

    // Create camera with TCP transport
    let camera = create_camera(&camera_addr).await?;

    // Power on
    info!("Powering on the camera");
    camera.power_on().await?;
    sleep(Duration::from_secs(2)).await;

    // Move to home
    info!("Moving to home position");
    camera.home().await?;
    sleep(Duration::from_secs(3)).await;

    // Perform movements
    perform_pan_tilt_movements(&camera).await?;

    // Zoom demo
    info!("Performing zoom operations");
    camera.zoom_in().await?;
    sleep(Duration::from_secs(2)).await;
    camera.zoom_stop().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_out().await?;
    sleep(Duration::from_secs(2)).await;
    camera.zoom_stop().await?;

    // Return home
    info!("Returning to home position");
    camera.home().await?;

    info!("Hello World example completed successfully!");
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration};

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    info!("Starting {} Hello World example...", env!("CARGO_PKG_NAME"));
    info!("Connecting to camera at {}", camera_addr);

    // Create camera
    let mut camera = create_camera(&camera_addr)?;

    // Power on
    info!("Powering on the camera");
    camera.power_on()?;
    thread::sleep(Duration::from_secs(2));

    // Move to home
    info!("Moving to home position");
    camera.home()?;
    thread::sleep(Duration::from_secs(3));

    // Perform movements
    perform_pan_tilt_movements(&mut camera)?;

    // Zoom demo
    info!("Performing zoom operations");
    camera.zoom_in()?;
    thread::sleep(Duration::from_secs(2));
    camera.zoom_stop()?;
    thread::sleep(Duration::from_secs(1));
    camera.zoom_out()?;
    thread::sleep(Duration::from_secs(2));
    camera.zoom_stop()?;

    // Return home
    info!("Returning to home position");
    camera.home()?;

    info!("Hello World example completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
async fn create_camera(
    addr: &str,
) -> Result<
    Camera<PTZOpticsG2, grafton_visca::transport::tokio::TcpTransport>,
    Box<dyn std::error::Error>,
> {
    use grafton_visca::transport::tokio::TcpTransport;

    let transport = TcpTransport::connect(addr).await?;
    Ok(Camera::<PTZOpticsG2, _>::new(transport))
}

#[cfg(not(feature = "async"))]
fn create_camera(
    addr: &str,
) -> Result<
    Camera<PTZOpticsG2, grafton_visca::transport::blocking::TcpTransport>,
    Box<dyn std::error::Error>,
> {
    use grafton_visca::transport::blocking::TcpTransport;

    let transport = TcpTransport::connect(addr)?;
    Ok(Camera::<PTZOpticsG2, _>::new(transport))
}

#[cfg(feature = "async")]
async fn perform_pan_tilt_movements<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    use tokio::time::sleep;

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

        sleep(Duration::from_secs(3)).await;

        debug!("Sending Pan/Tilt stop command");
        camera.stop().await?;

        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn perform_pan_tilt_movements<T: grafton_visca::transport::BlockingTransport>(
    camera: &mut Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    use std::{thread, time::Duration};

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
