//! Example program

//! Demonstrates white balance fine-tuning commands.

mod common;
use common::blocking::UdpTransport;

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::BlockingAdapter,
};

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create camera with PTZOptics G2 profile
    let udp_transport = UdpTransport::new("192.168.1.100:52381")?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(udp_transport));

    // Demonstrate fine-tuning commands
    println!("Demonstrating white balance fine-tuning...");

    // Apply red tuning
    println!("Applying slight red correction (+2)...");
    block_on(camera.set_red_tuning(2))?;

    // Apply blue tuning
    println!("Applying slight blue correction (-3)...");
    block_on(camera.set_blue_tuning(-3))?;

    // Reset both to neutral
    println!("Resetting both tuning values to neutral...");
    block_on(camera.set_red_tuning(0))?;
    block_on(camera.set_blue_tuning(0))?;

    println!("White balance tuning demonstration complete!");

    Ok(())
}
