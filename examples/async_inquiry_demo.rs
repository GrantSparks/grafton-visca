//! Example demonstrating Camera API inquiry commands.
//!
//! This example shows:
//! - How to use inquiry commands with the Camera<P> API
//! - Profile-aware position conversions
//! - Getting comprehensive camera state
//! - Querying various camera parameters
//!
//! The Camera API now supports full inquiry functionality through the
//! send_and_receive() method, making it a complete replacement for the Client API.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::Camera,
    profiles::PTZOpticsG2,
    transport::create,
    units::{Degrees, Raw},
    Error,
};
use std::env;
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration};

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'async' feature.");
    eprintln!("Run with: cargo run --example async_inquiry_demo --features async");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    eprintln!("This example uses inquiry methods that are not yet implemented in the current API.");
    eprintln!("");
    eprintln!("The Camera API focuses on control commands. Inquiry functionality would need");
    eprintln!("to be implemented using the command API directly with InquiryCommand types.");
    eprintln!("");
    eprintln!("This example is kept for reference but needs to be rewritten to use the");
    eprintln!("available API methods.");
    
    Ok(())
}