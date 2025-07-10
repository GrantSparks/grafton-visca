//! Example program

//! Demonstrates white balance fine-tuning commands.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::Camera,
    profiles::PTZOpticsG2,
    transport::create,
    types::{BlueTuning, RedTuning},
};

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create camera with PTZOptics G2 profile
    let transport = create::udp("192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Demonstrate fine-tuning commands
    println!("Demonstrating white balance fine-tuning...");

    // Apply red tuning
    println!("Applying slight red correction (+2)...");
    camera.set_red_tuning(RedTuning::new(2)?).await?;

    // Apply blue tuning
    println!("Applying slight blue correction (-3)...");
    camera.set_blue_tuning(BlueTuning::new(-3)?).await?;

    // Reset both to neutral
    println!("Resetting both tuning values to neutral...");
    camera.set_red_tuning(RedTuning::new(0)?).await?;
    camera.set_blue_tuning(BlueTuning::new(0)?).await?;

    println!("White balance tuning demonstration complete!");

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example white_balance_tuning_demo --features tokio");
}
