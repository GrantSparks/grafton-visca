//! Example demonstrating inquiry methods on Camera
//!
//! This shows how to query camera state using the high-level Camera API.

fn main() {
    // Note: This example demonstrates the API without a real camera connection
    println!("Camera inquiry API example:");
    println!();
    println!("// Create a camera with a specific profile:");
    println!("let transport = Tcp::connect(\"192.168.0.110:5678\")?;");
    println!("let camera = PTZOpticsG2Cam::new(transport);");
    println!();
    println!("// Query camera state using high-level methods:");
    println!("let power_on = camera.is_powered_on()?;");
    println!("let (pan, tilt) = camera.get_pan_tilt_position()?;");
    println!("let zoom = camera.get_zoom_position()?;");
    println!("let focus = camera.get_focus_position()?;");
    println!("let exposure_mode = camera.get_exposure_mode()?;");
    println!("let white_balance = camera.get_white_balance_mode()?;");
    println!();
    println!("// These methods handle all the low-level VISCA protocol details internally.");
    println!("// The InquiryCommand derive macro generates the protocol implementation,");
    println!("// but users don't need to interact with it directly.");
    println!();
    println!("// The Camera API provides a clean, type-safe interface without exposing");
    println!("// internal command structures or protocol details.");
}
