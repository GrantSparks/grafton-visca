//! Demonstrates the automatic port selection feature of the CameraBuilder.
//!
//! The builder automatically adds the correct default port based on the camera profile:
//! - PTZOptics/Generic VISCA: TCP=5678, UDP=1259
//! - Sony cameras: 52381
//!
//! This is a demonstration-only example that shows how the port defaults work.
//! It doesn't actually create any camera connections.

use grafton_visca::constants::ports;

fn main() {
    println!("=== CameraBuilder Automatic Port Selection Demo ===\n");

    println!("When you don't specify a port, the builder adds the correct default:\n");

    // PTZOptics camera without port - will use 5678 for TCP
    println!("1. PTZOptics TCP without port:");
    println!("   CameraBuilder::tcp(\"192.168.0.110\")");
    println!("       .profile::<PTZOpticsG2>()");
    println!(
        "   → Will connect to: 192.168.0.110:{}",
        ports::PTZOPTICS_TCP_PORT
    );

    // PTZOptics camera with UDP - will use 1259
    println!("\n2. PTZOptics UDP without port:");
    println!("   CameraBuilder::udp(\"192.168.0.110\")");
    println!("       .profile::<PTZOpticsG2>()");
    println!(
        "   → Will connect to: 192.168.0.110:{}",
        ports::PTZOPTICS_UDP_PORT
    );

    // Sony camera - will use 52381
    println!("\n3. Sony camera without port:");
    println!("   CameraBuilder::tcp(\"192.168.0.111\")");
    println!("       .profile::<SonyFR7>()");
    println!(
        "   → Will connect to: 192.168.0.111:{}",
        ports::SONY_VISCA_PORT
    );

    println!("\n{}", "=".repeat(50));
    println!("\nWhen you DO specify a port, it overrides the default:\n");

    // Override default port
    println!("4. PTZOptics with custom port:");
    println!("   CameraBuilder::tcp(\"192.168.0.110:8080\")");
    println!("       .profile::<PTZOpticsG2>()");
    println!("   → Will connect to: 192.168.0.110:8080 (your custom port)");

    println!("\n{}", "=".repeat(50));
    println!("\nYou can also use the port constants directly:\n");

    println!("5. Using constants from the library:");
    println!("   use grafton_visca::constants::ports;");
    println!("   ");
    println!("   let addr = format!(\"192.168.0.110:{{}}\", ports::PTZOPTICS_TCP_PORT);");
    println!("   CameraBuilder::tcp(&addr).profile::<PTZOpticsG2>()");

    println!("\nAvailable port constants:");
    println!(
        "   ports::PTZOPTICS_TCP_PORT = {}",
        ports::PTZOPTICS_TCP_PORT
    );
    println!(
        "   ports::PTZOPTICS_UDP_PORT = {}",
        ports::PTZOPTICS_UDP_PORT
    );
    println!("   ports::SONY_VISCA_PORT = {}", ports::SONY_VISCA_PORT);
}
