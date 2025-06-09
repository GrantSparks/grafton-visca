//! Example demonstrating configurable timeouts for different command types.
//!
//! This example shows how to use the `TimeoutConfig` to set different timeout
//! durations for various categories of VISCA commands, allowing fine-tuned
//! control over command execution timeouts.

// TODO: Update imports when example is re-enabled
// Currently unused imports commented out to satisfy clippy
/*
use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        InquiryCommand,
    },
    TimeoutConfigBuilder, Client, ViscaCommand,
};
use std::error::Error;
use std::time::Duration;
*/

// TODO: Update this example for v0.5.0 - configurable timeouts are not yet available in Client
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("Configurable timeouts are not yet available in Client");
}

/*
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    println!("=== VISCA Configurable Timeouts Example ===\n");

    // Create a custom timeout configuration
    let timeout_config = TimeoutConfigBuilder::default()
        .quick_timeout(Duration::from_secs(1)) // Fast for inquiries
        .movement_timeout(Duration::from_secs(5)) // Medium for pan/tilt/zoom
        .preset_timeout(Duration::from_secs(30)) // Long for preset operations
        .long_timeout(Duration::from_secs(120)) // Very long for complex operations
        .default_timeout(Duration::from_secs(10)) // Default for everything else
        .build();

    // Display the configured timeouts
    println!("Configured Timeouts:");
    println!("  Quick commands:    {:?}", timeout_config.quick_timeout);
    println!("  Movement commands: {:?}", timeout_config.movement_timeout);
    println!("  Preset commands:   {:?}", timeout_config.preset_timeout);
    println!("  Long operations:   {:?}", timeout_config.long_timeout);
    println!("  Default timeout:   {:?}", timeout_config.default_timeout);
    println!();

    // Create client with custom timeout configuration
    let camera_address = "192.168.1.100:5678";
    let mut client = Client::connect_udp_with_timeout(camera_address, timeout_config)?;

    println!("Connected to camera at {}\n", camera_address);

    // Demonstrate different command categories and their timeouts
    demonstrate_quick_command(&mut client)?;
    demonstrate_movement_command(&mut client)?;
    demonstrate_preset_command(&mut client)?;

    Ok(())
}

fn demonstrate_quick_command(client: &mut Client) -> Result<(), Box<dyn Error>> {
    println!("1. Quick Command (Power Inquiry):");

    let command = InquiryCommand::Power;
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Quick (1 second)");

    let start = std::time::Instant::now();
    match client.send_command_and_wait(&command) {
        Ok(response) => {
            let elapsed = start.elapsed();
            println!("   ✓ Response received in {:?}", elapsed);
            println!("   Response: {:?}", response);
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("   ✗ Error after {:?}: {}", elapsed, e);
        }
    }
    println!();

    Ok(())
}

fn demonstrate_movement_command(client: &mut Client) -> Result<(), Box<dyn Error>> {
    println!("2. Movement Command (Pan/Tilt):");

    let command = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(10)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Movement (5 seconds)");

    let start = std::time::Instant::now();
    match client.send_command_and_wait(&command) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Command completed in {:?}", elapsed);

            // Stop movement
            let stop = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            let _ = client.send_command_and_wait(&stop);
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("   ✗ Error after {:?}: {}", elapsed, e);
        }
    }
    println!();

    Ok(())
}

fn demonstrate_preset_command(client: &mut Client) -> Result<(), Box<dyn Error>> {
    println!("3. Preset Command (Recall Preset):");

    let command = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(1)?,
    };
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Preset (30 seconds)");

    let start = std::time::Instant::now();
    match client.send_command_and_wait(&command) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Preset recalled in {:?}", elapsed);
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("   ✗ Error after {:?}: {}", elapsed, e);
        }
    }
    println!();

    Ok(())
}
*/

#[cfg(test)]
mod tests {
    use grafton_visca::{
        command::{
            pan_tilt::PanTiltCommand,
            preset::{PresetAction, PresetCommand, PresetNumber},
            CommandCategory, InquiryCommand,
        },
        TimeoutConfigBuilder,
    };
    use std::time::Duration;

    #[test]
    fn test_timeout_configuration() {
        // Test that we can create a timeout configuration
        let config = TimeoutConfigBuilder::default()
            .quick_timeout(Duration::from_millis(500))
            .movement_timeout(Duration::from_secs(3))
            .preset_timeout(Duration::from_secs(20))
            .build();

        assert_eq!(config.quick_timeout, Duration::from_millis(500));
        assert_eq!(config.movement_timeout, Duration::from_secs(3));
        assert_eq!(config.preset_timeout, Duration::from_secs(20));
    }

    #[test]
    fn test_command_categories() {
        // Test that commands report the correct categories
        let power_inquiry = InquiryCommand::Power;
        assert_eq!(power_inquiry.command_category(), CommandCategory::Quick);

        let pan_tilt = PanTiltCommand::Home;
        assert_eq!(pan_tilt.command_category(), CommandCategory::Movement);

        let preset = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(1).unwrap(),
        };
        assert_eq!(preset.command_category(), CommandCategory::Preset);
    }
}
