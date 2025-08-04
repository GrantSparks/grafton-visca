//! Simple UDP test for VISCA camera connection

#[cfg(feature = "tokio")]
use grafton_visca::{prelude::r#async::*, transport::tokio::Udp};

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature.");
    eprintln!("Run with: cargo run --example test_udp --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let addr = std::env::args()
        .nth(1)
        .map(|ip| format!("{ip}:1259"))
        .unwrap_or_else(|| "192.168.0.110:1259".to_string());

    println!("Connecting to camera at {addr} via UDP...");

    let transport = Udp::connect(&addr).await?;
    println!("✓ Connected!");

    // Create a camera using the transport
    let camera = PTZOpticsG2Cam::new(transport);

    // Try a simple command - power on
    println!("Sending power on command...");
    match camera.power_on().await {
        Ok(_) => {
            println!("✓ Power on command sent successfully!");
        }
        Err(e) => {
            println!("✗ Command failed: {e}");
        }
    }

    // Try to move to home position
    println!("Moving to home position...");
    match camera.pan_tilt_home().await {
        Ok(_) => {
            println!("✓ Home command sent successfully!");
        }
        Err(e) => {
            println!("✗ Home command failed: {e}");
        }
    }

    Ok(())
}
