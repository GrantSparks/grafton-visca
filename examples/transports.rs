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

#[cfg(not(feature = "async"))]
use std::{
    env, thread,
    time::{Duration, Instant},
};

#[cfg(not(feature = "async"))]
use grafton_visca::{
    mode::{Blocking, BlockingFutureExt},
    profiles::PtzOpticsG2,
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Camera,
    Error,
    // Import control traits
    PanTiltControl,
    ZoomControl,
};

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

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
    let mut tcp_camera = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp(format!("{camera_addr}:5678"))?;

    println!("✓ TCP connection established");

    // Test TCP connection with a simple command
    println!("Testing TCP transport with zoom command...");
    tcp_camera.zoom_absolute(Normalized(0.3)).block()?;
    tcp_camera.await_zoom_idle(Duration::from_secs(5))?;
    println!("✓ Command sent successfully via TCP");
    println!();

    // === TCP WITH CUSTOM PORT ===
    println!("═══ TCP with Custom Port ═══");
    println!("Connecting via TCP on custom port 1259...");

    match Camera::<Blocking, PtzOpticsG2, Box<dyn grafton_visca::transport::SyncTransport>, ()>::open_tcp(format!("{camera_addr}:1259")) {
        Ok(camera) => {
            println!("✓ TCP connection established on port 1259");

            // Test connection
            println!("Testing custom port connection...");
            camera.pan_tilt_home().block()?;
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

    println!("Connecting via UDP port 1259 (raw VISCA for PTZOptics)...");
    // PTZOptics uses UDP port 1259 for raw VISCA
    // Sony cameras typically use UDP port 52381 with encapsulation

    match Camera::<Blocking, PtzOpticsG2, Box<dyn grafton_visca::transport::SyncTransport>, ()>::open_udp(format!("{camera_addr}:1259")) {
        Ok(mut camera) => {
            println!("✓ UDP transport initialized");

            // Test UDP connection
            println!("Testing UDP transport with pan/tilt command...");
            camera.pan_tilt_absolute(Degrees(45.0), Degrees(0.0), SpeedLevel::Medium).block()?;
            camera.await_pan_tilt_idle(Duration::from_secs(5))?;
            println!("✓ Command sent successfully via UDP");

            // Return to home
            camera.pan_tilt_home().block()?;
            camera.await_pan_tilt_idle(Duration::from_secs(5))?;
        }
        Err(e) => {
            println!("✗ Failed to initialize UDP transport: {e}");
            println!("  (Camera may not support UDP or may use different port)");
        }
    }
    println!();

    // === CONNECTION ERROR HANDLING ===
    println!("═══ Connection Error Handling ═══");
    println!("Testing connection error handling...");

    // Try to connect to a non-existent address
    println!("Attempting to connect to non-existent camera (192.168.255.255)...");
    let start = Instant::now();

    let timeout_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp("192.168.255.255:5678");

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

    let tcp_start = Instant::now();
    for i in 0..10 {
        tcp_camera
            .zoom_absolute(Normalized((i as f32) * 0.1))
            .block()?;
        thread::sleep(Duration::from_millis(100));
    }
    let tcp_elapsed = tcp_start.elapsed();
    println!("✓ TCP: 10 commands in {:.2}s", tcp_elapsed.as_secs_f32());

    // Reset zoom
    tcp_camera.zoom_absolute(Normalized(0.0)).block()?;

    // If UDP is available, compare performance
    if let Ok(udp_camera) = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_udp(format!("{camera_addr}:52381"))
    {
        println!("Sending 10 commands via UDP...");

        let udp_start = Instant::now();
        for i in 0..10 {
            udp_camera
                .zoom_absolute(Normalized((i as f32) * 0.1))
                .block()?;
            thread::sleep(Duration::from_millis(100));
        }
        let udp_elapsed = udp_start.elapsed();
        println!("✓ UDP: 10 commands in {:.2}s", udp_elapsed.as_secs_f32());

        // Reset zoom
        udp_camera.zoom_absolute(Normalized(0.0)).block()?;

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

#[cfg(feature = "async")]
fn main() {
    println!("This example requires blocking mode. Run without the async feature:");
    println!("  cargo run --example transports --no-default-features");
}
