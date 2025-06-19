//! Demonstration of new Camera<P> API features:
//! - CommandBuilder for fluent command sequences
//! - Extension traits for custom functionality

#[cfg(feature = "async-client")]
mod common;
#[cfg(feature = "async-client")]
use common::r#async::udp_transport;
#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        Camera, CommandBuilderExt,
    },
    Error,
};

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Demonstrate all new features
    println!("=== VISCA Camera New Features Demo ===\n");

    println!("1. Command Builder for fluent sequences:");
    command_builder_demo().await?;

    Ok(())
}

/// Demonstrates CommandBuilder for fluent command sequences
#[cfg(feature = "async-client")]
async fn command_builder_demo() -> Result<(), Error> {
    println!("Using CommandBuilder for complex sequences...");

    let transport = udp_transport("192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Build and execute a complex sequence
    println!("Building camera initialization sequence:");
    let results = camera
        .commands()
        .power_on()
        .pan_tilt_home()
        .delay(std::time::Duration::from_secs(2))
        .zoom_in(grafton_visca::command::zoom::ZoomSpeed::new(5).unwrap())
        .delay(std::time::Duration::from_millis(500))
        .zoom_stop()
        .execute_sequential_async()
        .await?;

    println!("  ✓ Initialization sequence completed ({} commands)", results.len());

    // Build a more complex movement sequence
    println!("\nExecuting movement sequence:");
    let movement_results = camera
        .commands()
        .pan_tilt_home()
        .delay(std::time::Duration::from_secs(2))
        .pan_tilt_to_degrees(
            45.0,
            0.0,
            grafton_visca::command::pan_tilt::PanSpeed::new(15).unwrap(),
            grafton_visca::command::pan_tilt::TiltSpeed::new(15).unwrap(),
        )?
        .zoom_in(grafton_visca::command::zoom::ZoomSpeed::new(5).unwrap())
        .preset_set(G2PresetId::new(1).unwrap())
        .pan_tilt_to_degrees(
            -45.0,
            0.0,
            grafton_visca::command::pan_tilt::PanSpeed::new(15).unwrap(),
            grafton_visca::command::pan_tilt::TiltSpeed::new(15).unwrap(),
        )?
        .zoom_out(grafton_visca::command::zoom::ZoomSpeed::new(5).unwrap())
        .preset_set(G2PresetId::new(2).unwrap())
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Movement sequence completed ({} commands)",
        movement_results.len()
    );

    // Demonstrate concurrent execution
    println!("\nExecuting concurrent commands:");
    let concurrent_results = camera
        .commands()
        .custom(
            grafton_visca::command::inquiry::InquiryCommand::Power,
            "Query Power State",
        )
        .custom(
            grafton_visca::command::inquiry::InquiryCommand::ZoomPosition,
            "Query Zoom Position",
        )
        .custom(
            grafton_visca::command::inquiry::InquiryCommand::FocusPosition,
            "Query Focus Mode",
        )
        .execute_concurrent()
        .await?;

    let successful = concurrent_results.iter().filter(|r| r.is_ok()).count();
    println!(
        "  ✓ {} of {} concurrent queries succeeded",
        successful,
        concurrent_results.len()
    );

    Ok(())
}

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature. Run with:");
    eprintln!("  cargo run --example new_features_demo --features async-client");
}