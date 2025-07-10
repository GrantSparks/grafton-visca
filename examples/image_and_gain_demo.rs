//! Example program

//! Example demonstrating image quality and gain control features.

#[cfg(feature = "tokio")]
use grafton_visca::Error;

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example image_and_gain_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    eprintln!("This example uses image processing methods that are not yet implemented in the current API.");
    eprintln!();
    eprintln!("Methods like set_luminance, set_contrast, set_sharpness, and set_gain would need");
    eprintln!("to be implemented in the camera methods module.");
    eprintln!();
    eprintln!("This example is kept for reference but needs to be updated when these methods");
    eprintln!("are implemented.");

    Ok(())
}
