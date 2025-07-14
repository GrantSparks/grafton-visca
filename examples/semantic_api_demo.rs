//! Demonstrates the semantic type API for intuitive camera control.
//!
//! This example shows how to use the camera API with type-safe methods
//! for more intuitive camera control.

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::methods::{FocusOps, PanTiltOps, ZoomOps},
        command::pan_tilt::PanTiltDirection,
        transport::blocking::Udp,
        types::{PanSpeed, SpeedLevel, TiltSpeed},
        units::{Degrees, Normalized},
        Camera,
    };

    // Connect to camera
    let transport = Udp::connect("192.168.1.100:52381")?;
    let mut camera = Camera::new(transport);

    println!("=== Semantic API Demo ===\n");

    // Zoom Control
    println!("1. Zoom Control");
    println!("   Setting zoom to 50%...");
    camera.zoom_absolute(Normalized::new(0.5))?;

    println!("   Setting zoom using raw VISCA value...");
    camera.zoom_absolute(Normalized::new(0x3000 as f32 / 0x4000 as f32))?; // Convert to normalized position

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to auto...");
    camera.focus_auto()?;

    println!("   Setting focus to manual...");
    camera.focus_manual()?;

    println!("   Using focus one push...");
    camera.focus_one_push()?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to home position...");
    camera.pan_tilt_home()?;

    println!("   Moving to 45° right, 15° up...");
    camera.pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-15.0), SpeedLevel::Medium)?;

    // Movement Control
    println!("\n4. Movement Control");
    println!("   Moving right at speed 10...");
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;

    println!("   Stopping movement...");
    camera.pan_tilt_stop()?;

    // Zoom Operations
    println!("\n5. Zoom Operations");
    println!("   Zooming in...");
    camera.zoom_in()?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("   Stopping zoom...");
    camera.zoom_stop()?;

    println!("   Zooming out...");
    camera.zoom_out()?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("   Stopping zoom...");
    camera.zoom_stop()?;

    println!("\n✅ All semantic API operations completed successfully!");

    Ok(())
}

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    eprintln!("This example requires the 'tokio' feature.");
    eprintln!("Run with: cargo run --example semantic_api_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::methods::{FocusOps, PanTiltOps, ZoomOps},
        command::pan_tilt::PanTiltDirection,
        transport::tokio::Udp,
        types::{PanSpeed, SpeedLevel, TiltSpeed},
        units::{Degrees, Normalized},
        Camera,
    };

    // Connect to camera
    let transport = Udp::connect("192.168.1.100:52381").await?;
    let camera = Camera::new(transport);

    println!("=== Semantic API Demo (Async) ===\n");

    // Zoom Control
    println!("1. Zoom Control");
    println!("   Setting zoom to 50%...");
    camera.zoom_absolute(Normalized::new(0.5)).await?;

    println!("   Setting zoom using raw VISCA value...");
    camera
        .zoom_absolute(Normalized::new(0x3000 as f32 / 0x4000 as f32))
        .await?; // Convert to normalized position

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to auto...");
    camera.focus_auto().await?;

    println!("   Setting focus to manual...");
    camera.focus_manual().await?;

    println!("   Using focus one push...");
    camera.focus_one_push().await?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to home position...");
    camera.pan_tilt_home().await?;

    println!("   Moving to 45° right, 15° up...");
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-15.0), SpeedLevel::Medium)
        .await?;

    // Movement Control
    println!("\n4. Movement Control");
    println!("   Moving right at speed 10...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;

    println!("   Stopping movement...");
    camera.pan_tilt_stop().await?;

    // Zoom Operations
    println!("\n5. Zoom Operations");
    println!("   Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    println!("   Stopping zoom...");
    camera.zoom_stop().await?;

    println!("   Zooming out...");
    camera.zoom_out().await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    println!("   Stopping zoom...");
    camera.zoom_stop().await?;

    println!("\n✅ All semantic API operations completed successfully!");

    Ok(())
}
