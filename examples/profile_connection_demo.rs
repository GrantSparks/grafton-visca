//! Profile-centric connection helper demonstration.
//!
//! This example shows how to use the new connection helpers introduced in issue #183.
//! Each camera profile provides convenient connection methods for different transport types.
//!
//! Run with:
//! - Blocking TCP: cargo run --example profile_connection_demo tcp <camera_ip:port>
//! - Blocking UDP: cargo run --example profile_connection_demo udp <camera_ip:port>
//! - Async TCP: cargo run --example profile_connection_demo --features tokio async-tcp <camera_ip:port>
//! - Async UDP: cargo run --example profile_connection_demo --features tokio async-udp <camera_ip:port>

use grafton_visca::CameraBuilder;

#[cfg(not(feature = "tokio"))]
use grafton_visca::camera::profiles::{GenericVisca, PTZOpticsG2};

#[cfg(feature = "tokio")]
use grafton_visca::camera::profiles::{PTZOpticsG2, SonyFR7};
use grafton_visca::Result;
use std::env;

// Import operation traits for blocking operations
#[cfg(not(feature = "tokio"))]
use grafton_visca::blocking::{InquiryOps, PanTiltOps, PowerOps};

// Import operation traits for async operations
#[cfg(feature = "tokio")]
use grafton_visca::r#async::{InquiryOps, PanTiltOps, PowerOps};

#[cfg(not(feature = "tokio"))]
fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tcp|udp> <camera_ip:port>", args[0]);
        std::process::exit(1);
    }

    let transport_type = &args[1];
    let camera_addr = &args[2];

    match transport_type.as_str() {
        "tcp" => {
            println!("Connecting to PTZOptics G2 camera via TCP at {camera_addr}");
            let camera = CameraBuilder::tcp(camera_addr)
                .profile::<PTZOpticsG2>()
                .build()?;

            println!("Model: {}", camera.model_name());
            println!("Power on...");
            camera.power_on()?;

            println!("Getting power state...");
            match camera.get_power_state() {
                Ok(state) => println!("Power is: {}", if state { "ON" } else { "OFF" }),
                Err(e) => println!("Could not get power state: {e}"),
            }
        }
        "udp" => {
            println!("Connecting to Generic VISCA camera via UDP at {camera_addr}");
            let camera = CameraBuilder::udp(camera_addr)
                .profile::<GenericVisca>()
                .build()?;

            println!("Model: {}", camera.model_name());
            println!("Power on...");
            camera.power_on()?;

            println!("Moving to home position...");
            camera.pan_tilt_home()?;
        }
        _ => {
            eprintln!("Invalid transport type. Use 'tcp' or 'udp'");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <async-tcp|async-udp> <camera_ip:port>", args[0]);
        std::process::exit(1);
    }

    let transport_type = &args[1];
    let camera_addr = &args[2];

    match transport_type.as_str() {
        "async-tcp" => {
            println!("Connecting to Sony FR7 camera via async TCP at {camera_addr}");
            let camera = CameraBuilder::tokio_tcp(camera_addr)
                .profile::<SonyFR7>()
                .build()
                .await?;

            println!("Model: {}", camera.model_name());
            println!("This camera supports ND filters!");

            println!("Power on...");
            camera.power_on().await?;

            println!("Getting zoom position...");
            match camera.get_zoom_position().await {
                Ok(pos) => println!("Zoom position: 0x{pos:04X}"),
                Err(e) => println!("Could not get zoom position: {e}"),
            }
        }
        "async-udp" => {
            println!("Connecting to PTZOptics G2 camera via async UDP at {camera_addr}");
            let camera = CameraBuilder::tokio_udp(camera_addr)
                .profile::<PTZOpticsG2>()
                .build()
                .await?;

            println!("Model: {}", camera.model_name());
            println!("Max pan speed: {}", camera.max_pan_speed());
            println!("Max tilt speed: {}", camera.max_tilt_speed());

            println!("Power on...");
            camera.power_on().await?;

            // Pan left then right
            println!("Pan left...");
            camera
                .pan_tilt_move(
                    grafton_visca::PanTiltDirection::Left,
                    grafton_visca::types::PanSpeed::new(10).unwrap(),
                    grafton_visca::types::TiltSpeed::new(0).unwrap(),
                )
                .await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            println!("Pan right...");
            camera
                .pan_tilt_move(
                    grafton_visca::PanTiltDirection::Right,
                    grafton_visca::types::PanSpeed::new(10).unwrap(),
                    grafton_visca::types::TiltSpeed::new(0).unwrap(),
                )
                .await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            println!("Stop...");
            camera.pan_tilt_stop().await?;
        }
        _ => {
            eprintln!("Invalid transport type. Use 'async-tcp' or 'async-udp'");
            std::process::exit(1);
        }
    }

    Ok(())
}
