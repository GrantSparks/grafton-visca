//! Demonstrates white balance fine-tuning commands.

use grafton_visca::{Client, ViscaWhiteBalanceExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create client
    let mut client = Client::connect_udp("192.168.1.100:52381")?;

    // Demonstrate fine-tuning commands
    println!("Demonstrating white balance fine-tuning...");

    // Apply red tuning
    println!("Applying slight red correction (+2)...");
    client.white_balance_red_tuning(2)?;

    // Apply blue tuning
    println!("Applying slight blue correction (-3)...");
    client.white_balance_blue_tuning(-3)?;

    // Reset both to neutral
    println!("Resetting both tuning values to neutral...");
    client.white_balance_red_tuning(0)?;
    client.white_balance_blue_tuning(0)?;

    println!("White balance tuning demonstration complete!");

    Ok(())
}
