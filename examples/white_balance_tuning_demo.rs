//! Example program
//!
//! Demonstrates white balance control commands.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::methods::WhiteBalanceOps, profiles::PTZOpticsG2, transport::tokio::Udp, Camera,
};

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create camera with PTZOptics G2 profile
    let transport = Udp::connect("192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Demonstrate white balance commands
    println!("Demonstrating white balance control...");

    // Set to auto white balance
    println!("Setting white balance to auto...");
    camera.white_balance_auto().await?;

    // Wait a moment
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Note: white_balance_manual and white_balance_one_push_trigger
    // are not available in the current API.
    // You would need to implement these using the command API directly
    // or extend the white balance trait with these methods.

    println!("White balance demonstration complete!");

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example white_balance_tuning_demo --features tokio");
}
