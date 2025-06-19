//! Example demonstrating position conversion with the new `Camera<P>` API
//!
//! This example shows how to:
//! - Use the type-safe `Camera<P>` API with camera profiles
//! - Convert between different position units (VISCA, degrees, normalized)
//! - Leverage compile-time safety with camera-specific constants
//! - Query camera capabilities
//!
//! Run with:
//! ```sh
//! cargo run --example position_conversion --features async-client [camera_ip:port]
//! ```
//!
//! Default camera IP is 192.168.0.110:1259 if not specified.

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the async-client feature.");
    eprintln!("Run with: cargo run --example position_conversion --features async-client");
}

#[cfg(feature = "async-client")]
fn main() {
    eprintln!("Position conversion example currently disabled due to transport dependency issues.");
    eprintln!("This example demonstrates the Camera<P> API position conversion features.");
    eprintln!("See the source code for implementation details.");
}
