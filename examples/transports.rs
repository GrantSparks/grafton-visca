//! Transport options demonstration - TCP and UDP connections.
//!
//! This example shows how to use different transport layers (TCP and UDP)
//! for VISCA communication. It demonstrates:
//! - TCP transport for reliable communication
//! - UDP transport for low-latency communication
//! - Custom port configuration
//! - Connection timeouts and retries
//! - Transport-specific error handling
//!
//! Run with:
//! ```sh
//! cargo run --example transports [camera_ip[:port]]
//! ```

use grafton_visca::{
    camera::helpers::MovementHelpers, prelude::blocking::*, types::SpeedLevel, units::*,
    CameraBuilder, Error,
};
use std::{env, time::Duration};

fn main() -> Result<(), Error> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🔌 Transport Options Demo");
    println!("=========================");
    println!("Target camera: {camera_addr}");
    println!();

    // === TCP TRANSPORT (DEFAULT) ===
    println!("═══ TCP Transport ═══");
    println!("TCP provides reliable, ordered delivery of commands.");
    println!("Best for: Critical operations, preset management, configuration");
    println!();

    println!("Connecting via TCP (default port 5678)...");
    let tcp_camera = match CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?
    {
        grafton_visca::camera::BlockingCamera::Tcp(cam) => cam,
        grafton_visca::camera::BlockingCamera::Udp(_) => unreachable!("TCP builder should return TCP camera"),
    };

    println!("✓ TCP connection established");

    // Save initial state
    let initial_state = tcp_camera.save_state()?;

    // Test TCP connection with a simple command
    println!("Testing TCP transport with zoom command...");
    tcp_camera.zoom_absolute(Normalized(0.3))?;
    tcp_camera.await_zoom_idle(Duration::from_secs(5))?;
    println!("✓ Command sent successfully via TCP");

    // Restore original state
    tcp_camera.restore_state(&initial_state)?;
    println!();

    // === TCP WITH CUSTOM PORT ===
    println!("═══ TCP with Custom Port ═══");
    println!("Connecting via TCP on custom port 1259...");

    let tcp_custom = CameraBuilder::tcp(&format!("{camera_addr}:1259"))
        .profile::<PTZOpticsG2>()
        .build();

    match tcp_custom {
        Ok(camera_result) => {
            let camera = match camera_result {
                grafton_visca::camera::BlockingCamera::Tcp(cam) => cam,
                grafton_visca::camera::BlockingCamera::Udp(_) => unreachable!("TCP builder should return TCP camera"),
            };
            println!("✓ TCP connection established on port 1259");

            // Test connection
            println!("Testing custom port connection...");
            camera.pan_tilt_home()?;
            println!("✓ Command sent successfully via custom port");
        }
        Err(e) => {
            println!("✗ Failed to connect on port 1259: {e}");
            println!("  (This is expected if camera doesn't listen on this port)");
        }
    }
    println!();

    // === UDP TRANSPORT ===
    println!("═══ UDP Transport ═══");
    println!("UDP provides low-latency, connectionless communication.");
    println!("Best for: Real-time control, streaming operations");
    println!();

    println!("Connecting via UDP (auto-selects port 1259 for PTZOptics)...");
    // The builder automatically selects the correct port based on the profile:
    // - PTZOptics: UDP port 1259 (raw VISCA)
    // - Sony: UDP port 52381 (encapsulated VISCA)
    let udp_camera = CameraBuilder::udp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build();

    match udp_camera {
        Ok(camera_result) => {
            let camera = match camera_result {
                grafton_visca::camera::BlockingCamera::Tcp(_) => unreachable!("UDP builder should return UDP camera"),
                grafton_visca::camera::BlockingCamera::Udp(cam) => cam,
            };
            println!("✓ UDP transport initialized");

            // Test UDP connection
            println!("Testing UDP transport with pan/tilt command...");
            camera.pan_tilt_absolute(Degrees(45.0), Degrees(0.0), SpeedLevel::Medium)?;
            camera.await_pan_tilt_idle(Duration::from_secs(5))?;
            println!("✓ Command sent successfully via UDP");

            // Return to home
            camera.pan_tilt_home()?;
            camera.await_pan_tilt_idle(Duration::from_secs(5))?;
        }
        Err(e) => {
            println!("✗ Failed to initialize UDP transport: {e}");
            println!("  (Camera may not support UDP or may use different port)");
        }
    }
    println!();

    // === CONNECTION WITH TIMEOUT ===
    println!("═══ Connection with Timeout ═══");
    println!("Testing connection timeout handling...");

    // Try to connect to a non-existent address with timeout
    println!("Attempting to connect to non-existent camera (192.168.255.255)...");
    let start = std::time::Instant::now();

    let timeout_result = CameraBuilder::tcp("192.168.255.255")
        .profile::<PTZOpticsG2>()
        .build();

    let elapsed = start.elapsed();

    match timeout_result {
        Ok(_) => {
            println!("✗ Unexpected success (camera shouldn't exist)");
        }
        Err(e) => {
            println!(
                "✓ Connection failed as expected after {:.1}s",
                elapsed.as_secs_f32()
            );
            println!("  Error: {e}");
        }
    }
    println!();

    // === TRANSPORT COMPARISON ===
    println!("═══ Transport Comparison ═══");
    println!();
    println!("TCP Transport:");
    println!("  ✓ Reliable delivery");
    println!("  ✓ Connection state tracking");
    println!("  ✓ Automatic retransmission");
    println!("  ✗ Higher latency");
    println!("  ✗ Connection overhead");
    println!();
    println!("UDP Transport:");
    println!("  ✓ Low latency");
    println!("  ✓ No connection overhead");
    println!("  ✓ Better for real-time control");
    println!("  ✗ No delivery guarantee");
    println!("  ✗ Possible packet loss");
    println!();

    // === PERFORMANCE TEST ===
    println!("═══ Performance Comparison ═══");
    println!("Sending 10 commands via TCP...");

    let tcp_start = std::time::Instant::now();
    for i in 0..10 {
        tcp_camera.zoom_absolute(Normalized((i as f32) * 0.1))?;
        std::thread::sleep(Duration::from_millis(100));
    }
    let tcp_elapsed = tcp_start.elapsed();
    println!("✓ TCP: 10 commands in {:.2}s", tcp_elapsed.as_secs_f32());

    // Reset zoom
    tcp_camera.zoom_absolute(Normalized(0.0))?;

    // If UDP is available, compare performance
    if let Ok(camera_result) = CameraBuilder::udp(&format!("{camera_addr}:52381"))
        .profile::<PTZOpticsG2>()
        .build()
    {
        let udp_camera = match camera_result {
            grafton_visca::camera::BlockingCamera::Tcp(_) => unreachable!("UDP builder should return UDP camera"),
            grafton_visca::camera::BlockingCamera::Udp(cam) => cam,
        };
        println!("Sending 10 commands via UDP...");

        let udp_start = std::time::Instant::now();
        for i in 0..10 {
            udp_camera.zoom_absolute(Normalized((i as f32) * 0.1))?;
            std::thread::sleep(Duration::from_millis(100));
        }
        let udp_elapsed = udp_start.elapsed();
        println!("✓ UDP: 10 commands in {:.2}s", udp_elapsed.as_secs_f32());

        // Reset zoom
        udp_camera.zoom_absolute(Normalized(0.0))?;

        if udp_elapsed < tcp_elapsed {
            println!();
            println!(
                "UDP was {:.0}% faster than TCP",
                ((tcp_elapsed.as_secs_f32() - udp_elapsed.as_secs_f32())
                    / tcp_elapsed.as_secs_f32())
                    * 100.0
            );
        }
    }

    println!();
    println!("✨ Transport demo complete!");
    println!();
    println!("Summary:");
    println!("  • Use TCP for reliable command delivery");
    println!("  • Use UDP for low-latency real-time control");
    println!("  • Configure custom ports as needed");
    println!("  • Set appropriate timeouts for your network");
    println!();
    println!("See camera_profiles for camera-specific configurations.");

    Ok(())
}
