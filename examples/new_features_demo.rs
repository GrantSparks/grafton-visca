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
        Camera, CameraExtension, CommandBuilderExt, DiagnosticsExt,
    },
    Error,
};
#[cfg(feature = "async-client")]
use std::time::Duration;

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Demonstrate all new features
    println!("=== VISCA Camera New Features Demo ===\n");

    println!("1. Command Builder for fluent sequences:");
    command_builder_demo().await?;

    println!("\n2. Extension traits for custom functionality:");
    extension_traits_demo().await?;

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
        .zoom_stop()
        .focus_auto()
        .exposure_mode(grafton_visca::command::exposure::ExposureMode::Auto)
        .white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Auto)
        .backlight(false)
        .execute_sequential_async()
        .await?;

    println!("  ✓ Executed {} commands successfully", results.len());

    // Build a movement sequence
    println!("\nBuilding movement sequence:");
    let movement_results = camera
        .commands()
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

/// Demonstrates extension traits for custom functionality
#[cfg(feature = "async-client")]
async fn extension_traits_demo() -> Result<(), Error> {
    println!("Using extension traits for custom functionality...");

    let transport = udp_transport("192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Use DiagnosticsExt trait
    println!("Getting camera diagnostics:");
    let diagnostics = camera.get_diagnostics()?;
    println!("  Model: {}", diagnostics.model);
    println!("  Error count: {}", diagnostics.error_count);

    println!("\nRunning self-test:");
    let test_result = camera.run_self_test()?;
    println!(
        "  Pan/Tilt: {}",
        if test_result.pan_tilt_ok {
            "✓"
        } else {
            "✗"
        }
    );
    println!("  Zoom: {}", if test_result.zoom_ok { "✓" } else { "✗" });
    println!("  Focus: {}", if test_result.focus_ok { "✓" } else { "✗" });
    println!(
        "  Exposure: {}",
        if test_result.exposure_ok {
            "✓"
        } else {
            "✗"
        }
    );

    // Use ScriptingExt trait
    use grafton_visca::camera::extensions::{MovementAction, MovementStep, ScriptingExt};

    println!("\nExecuting movement script:");
    let script = vec![
        MovementStep {
            action: MovementAction::PanTilt {
                pan: 0.0,
                tilt: 0.0,
            },
            label: Some("Home".to_string()),
        },
        MovementStep {
            action: MovementAction::Wait {
                duration: Duration::from_secs(1),
            },
            label: Some("Pause".to_string()),
        },
        MovementStep {
            action: MovementAction::PanTilt {
                pan: 90.0,
                tilt: 15.0,
            },
            label: Some("Look right".to_string()),
        },
        MovementStep {
            action: MovementAction::Zoom { level: 5000 },
            label: Some("Zoom in".to_string()),
        },
    ];

    camera.execute_movement_script(&script)?;
    println!("  ✓ Script execution completed");

    Ok(())
}

// Example: Define a custom extension trait manually
#[cfg(feature = "async-client")]
#[allow(dead_code)]
trait BroadcastExt<P: grafton_visca::camera::CameraProfile>: CameraExtension<P> {
    /// Set up camera for broadcast (custom preset).
    fn setup_for_broadcast(&self) -> Result<(), Error> {
        println!("  Setting up camera for broadcast...");
        // In real implementation, this would configure multiple settings
        // using self.send_raw() or self.send_raw_async()
        Ok(())
    }

    /// Enable tally light (if supported).
    fn set_tally(&self, on: bool) -> Result<(), Error> {
        println!("  Tally light: {}", if on { "ON" } else { "OFF" });
        // Would send custom command for tally light
        Ok(())
    }
}

// Implement the extension for all Camera<P> types
#[cfg(feature = "async-client")]
impl<P: grafton_visca::camera::CameraProfile> BroadcastExt<P> for Camera<P> {}

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example new_features_demo --features async-client");
}
