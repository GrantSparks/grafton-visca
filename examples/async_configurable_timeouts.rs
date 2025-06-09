// TODO: Update this example for v0.5.0 - async support is not yet available
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("Async support is not yet available in the current version");
}

/*
//! Example demonstrating configurable timeouts for different command types with async transports.
//!
//! Run with: cargo run --example async_configurable_timeouts --features async-client
//!
//! This example shows how to use the TimeoutConfig to set different timeout
//! durations for various categories of VISCA commands when using async transports,
//! allowing fine-tuned control over command execution timeouts.

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async' feature. Run with:");
    eprintln!("cargo run --example async_configurable_timeouts --features async-client");
}

#[cfg(feature = "async-client")]
use grafton_visca::{
    async_transport::AsyncViscaTransport,
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        response::parse_visca_response,
        InquiryCommand,
    },
    AsyncUdpTransport, TimeoutConfigBuilder, ViscaCommand, Error, ViscaResponse,
};
#[cfg(feature = "async-client")]
use std::error::Error;
#[cfg(feature = "async-client")]
use std::net::ToSocketAddrs;
#[cfg(feature = "async-client")]
use std::time::Duration;

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    println!("=== VISCA Async Configurable Timeouts Example ===\n");

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

    // Parse camera address
    let camera_address = "192.168.1.100:5678";
    let socket_addr = camera_address
        .to_socket_addrs()?
        .next()
        .ok_or("Invalid camera address")?;

    // Create async transport with custom timeout configuration
    let mut transport = AsyncUdpTransport::with_timeout_config(socket_addr, timeout_config).await?;

    println!("Connected to camera at {}\n", camera_address);

    // Demonstrate different command categories and their timeouts
    demonstrate_quick_command(&mut transport).await?;
    demonstrate_movement_command(&mut transport).await?;
    demonstrate_preset_command(&mut transport).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_quick_command(
    transport: &mut dyn AsyncViscaTransport,
) -> Result<(), Box<dyn Error>> {
    println!("1. Quick Command (Power Inquiry):");

    let command = InquiryCommand::Power;
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Quick (1 second)");

    let start = std::time::Instant::now();
    match send_and_wait_async(transport, &command).await {
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

#[cfg(feature = "async-client")]
async fn demonstrate_movement_command(
    transport: &mut dyn AsyncViscaTransport,
) -> Result<(), Box<dyn Error>> {
    println!("2. Movement Command (Pan/Tilt):");

    let command = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(10)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Movement (5 seconds)");

    let start = std::time::Instant::now();
    match send_and_wait_async(transport, &command).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Command completed in {:?}", elapsed);

            // Stop movement
            let stop = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            let _ = send_and_wait_async(transport, &stop).await;
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("   ✗ Error after {:?}: {}", elapsed, e);
        }
    }
    println!();

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_preset_command(
    transport: &mut dyn AsyncViscaTransport,
) -> Result<(), Box<dyn Error>> {
    println!("3. Preset Command (Recall Preset):");

    let command = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(1).unwrap(),
    };
    println!("   Command category: {:?}", command.command_category());
    println!("   Expected timeout: Preset (30 seconds)");

    let start = std::time::Instant::now();
    match send_and_wait_async(transport, &command).await {
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

/// Helper function to send a command and wait for the appropriate response
#[cfg(feature = "async-client")]
async fn send_and_wait_async(
    transport: &mut dyn AsyncViscaTransport,
    command: &dyn ViscaCommand,
) -> Result<ViscaResponse, Error> {
    let response_type = command.response_type();

    // Send the command
    transport.send_command(command).await?;

    // Handle different response types
    if let Some(expected_type) = response_type {
        // This is an inquiry command, wait for the specific response
        loop {
            let responses = transport.receive_response().await?;
            for response_data in &responses {
                let parsed = parse_visca_response(response_data, &expected_type)?;
                if matches!(parsed, Response::InquiryResponse(_)) {
                    return Ok(parsed);
                }
            }
        }
    } else {
        // This is an action command, wait for ACK then completion
        let mut _got_ack = false;

        loop {
            let responses = transport.receive_response().await?;
            for response_data in &responses {
                // For action commands, we don't have a specific response type
                // so we need to parse based on the data
                if response_data.len() >= 2 && response_data[0] == 0x90 {
                    match response_data[1] {
                        0x41 | 0x42 => {
                            // ACK
                            _got_ack = true;
                        }
                        0x51 | 0x52 => {
                            // Completion
                            return Ok(Response::Completion);
                        }
                        0x60..=0x62 => {
                            // Error
                            let error_code = if response_data.len() > 2 {
                                response_data[2]
                            } else {
                                0
                            };
                            return Ok(Response::Error(Error::from_code(error_code)));
                        }
                        _ => continue,
                    }
                }
            }

            // Continue looping until we get a response
        }
    }
}

#[cfg(all(test, feature = "async-client"))]
mod tests {
    use super::*;
    use grafton_visca::{CommandCategory, TimeoutConfigBuilder};
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
*/
