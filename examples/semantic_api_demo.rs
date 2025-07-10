//! Demonstrates the semantic type API for intuitive camera control.
//!
//! This example shows how to use semantic types instead of raw VISCA values
//! for more intuitive and type-safe camera control.

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::Camera,
        profiles::PTZOpticsG2,
        transport::blocking::Udp,
        units::{Degrees, Fraction, Kelvin, Magnification, Percentage},
        FStop,
    };

    // Connect to camera
    let transport = Udp::connect("192.168.1.100:52381")?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Semantic API Demo ===\n");

    // Zoom Control - Multiple Ways
    println!("1. Zoom Control");
    println!("   Setting zoom to 50% using percentage...");
    camera.set_zoom(Percentage(50.0))?;

    println!("   Setting zoom to 10x magnification...");
    camera.set_zoom_magnification(Magnification(10.0))?;

    println!("   Setting zoom using raw VISCA value...");
    camera.set_zoom(ZoomPosition::new(0x3000)?)?;

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to 75% (near)...");
    camera.set_focus_percentage(Percentage(75.0))?;

    println!("   Setting focus using raw value...");
    camera.set_focus(FocusPosition::new(0x8000)?)?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to 45° right, 15° up...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(-15.0))?;

    println!("   Moving using radians...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(-15.0))?;

    // White Balance
    println!("\n4. White Balance");
    println!("   Setting to daylight (5600K)...");
    camera.set_white_balance_kelvin(Kelvin(5600))?;

    println!("   Setting to tungsten (3200K)...");
    camera.set_white_balance_kelvin(Kelvin(3200))?;

    // Shutter Speed
    println!("\n5. Shutter Speed");
    println!("   Setting to 1/60s...");
    camera.set_shutter_fraction(Fraction::new(1, 60))?;

    println!("   Setting to 1/1000s...");
    camera.set_shutter_fraction(Fraction::new(1, 1000))?;

    // Iris Control
    println!("\n6. Iris Control");
    println!("   Setting iris to F2.8...");
    camera.set_iris(FStop::F2_8)?;

    println!("   Setting iris to 50% open...");
    camera.set_iris(Percentage(50.0))?;

    // Movement with Percentage Speeds
    println!("\n7. Movement Control");
    println!("   Moving right at 50% speed...");
    camera.move_percentage(
        grafton_visca::command::pan_tilt::PanTiltDirection::Right,
        Percentage(50.0),
        Percentage(0.0),
    )?;

    println!("   Moving diagonally at 75% speed...");
    camera.move_percentage(
        grafton_visca::command::pan_tilt::PanTiltDirection::UpRight,
        Percentage(75.0),
        Percentage(75.0),
    )?;

    println!("\n   Stopping movement...");
    camera.pan_tilt_stop()?;

    // Convenience Methods
    println!("\n8. Convenience Methods");
    println!("   Setting zoom to 25%...");
    camera.set_zoom(Percentage(25.0))?;

    println!("   Setting zoom to 5x magnification...");
    camera.set_zoom_magnification(Magnification(5.0))?;

    println!("   Setting focus to infinity (0%)...");
    camera.set_focus_percentage(Percentage(0.0))?;

    println!("   Setting iris to 75% open...");
    camera.set_iris(Percentage(75.0))?;

    // Image Quality Controls
    println!("\n9. Image Quality Controls");
    println!("   Setting gain to 50% (~10.5dB)...");
    camera.set_gain(Percentage(50.0))?;

    println!("   Setting sharpness to 70%...");
    camera.set_sharpness(Percentage(70.0))?;

    println!("   Setting brightness to 50%...");
    camera.set_brightness(Percentage(50.0))?;

    println!("   Setting contrast to 60%...");
    camera.set_contrast(Percentage(60.0))?;

    println!("   Setting saturation to 75%...");
    camera.set_saturation(Percentage(75.0))?;

    println!("   Setting hue to 50%...");
    camera.set_hue_percentage(Percentage(50.0))?;

    // Using Raw Values
    println!("\n10. Using Raw Values for Fine Control");
    println!("   Setting gain to raw value 0x04 (12dB)...");
    camera.set_gain(Gain::new(0x04)?)?;

    println!("   Setting sharpness to raw value 5...");
    camera.set_sharpness(SharpnessLevel::new(5)?)?;

    println!("   Setting brightness to raw value 0x0C...");
    camera.set_brightness(BrightnessLevel::new(0x0C)?)?;

    // Direct Type Usage
    println!("\n11. Direct Type Usage");

    println!("   Setting gain with typed value...");
    camera.set_gain(Gain::new(0x05)?)?; // 15dB

    println!("   Setting sharpness with typed level...");
    camera.set_sharpness(SharpnessLevel::new(4)?)?;

    println!("   Setting brightness with typed level...");
    camera.set_brightness(BrightnessLevel::new(0x08)?)?;

    println!("\n✅ All semantic API operations completed successfully!");

    Ok(())
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::Camera,
        profiles::PTZOpticsG2,
        transport::tokio::Udp,
        types::{BrightnessLevel, FocusPosition, Gain, SharpnessLevel, ZoomPosition},
        units::{Degrees, Fraction, Kelvin, Magnification, Percentage},
        FStop,
    };

    // Connect to camera
    let transport = Udp::new("0.0.0.0:0", "192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Semantic API Demo (Async) ===\n");

    // Zoom Control - Multiple Ways
    println!("1. Zoom Control");
    println!("   Setting zoom to 50% using percentage...");
    camera.set_zoom(Percentage(50.0)).await?;

    println!("   Setting zoom to 10x magnification...");
    camera.set_zoom(Magnification(10.0)).await?;

    println!("   Setting zoom using raw VISCA value...");
    camera.set_zoom(ZoomPosition::new(0x3000)?).await?;

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to 75% (near)...");
    camera.set_focus(Percentage(75.0)).await?;

    println!("   Setting focus using raw value...");
    camera.set_focus(FocusPosition::new(0x8000)?).await?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to 45° right, 15° up...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(-15.0)).await?;

    println!("   Moving to another position...");
    camera.pan_tilt_absolute(Degrees(90.0), Degrees(0.0)).await?;

    // White Balance
    println!("\n4. White Balance");
    println!("   Setting to daylight (5600K)...");
    camera.set_white_balance(Kelvin(5600)).await?;

    println!("   Setting to tungsten (3200K)...");
    camera.set_white_balance(Kelvin(3200)).await?;

    // Shutter Speed
    println!("\n5. Shutter Speed");
    println!("   Setting to 1/60s...");
    camera.set_shutter(Fraction::new(1, 60)).await?;

    println!("   Setting to 1/1000s...");
    camera.set_shutter(Fraction::new(1, 1000)).await?;

    // Iris Control
    println!("\n6. Iris Control");
    println!("   Setting iris to F2.8...");
    camera.set_iris(FStop::F2_8).await?;

    println!("   Setting iris to 50% open...");
    camera.set_iris(Percentage(50.0)).await?;

    // Movement with Percentage Speeds
    println!("\n7. Movement Control");
    println!("   Moving right at 50% speed...");
    camera
        .move_continuous(
            grafton_visca::command::pan_tilt::PanTiltDirection::Right,
            Percentage(50.0),
            Percentage(0.0),
        )
        .await?;

    println!("   Moving diagonally at 75% speed...");
    camera
        .move_continuous(
            grafton_visca::command::pan_tilt::PanTiltDirection::UpRight,
            Percentage(75.0),
            Percentage(75.0),
        )
        .await?;

    println!("\n   Stopping movement...");
    camera.pan_tilt_stop().await?;

    // Convenience Methods
    println!("\n8. Convenience Methods");
    println!("   Setting zoom to 25%...");
    camera.set_zoom(Percentage(25.0)).await?;

    println!("   Setting zoom to 5x magnification...");
    camera.set_zoom(Magnification(5.0)).await?;

    println!("   Setting focus to infinity (0%)...");
    camera.set_focus(Percentage(0.0)).await?;

    println!("   Setting iris to 75% open...");
    camera.set_iris(Percentage(75.0)).await?;

    // Image Quality Controls
    println!("\n9. Image Quality Controls");
    println!("   Setting gain to 50% (~10.5dB)...");
    camera.set_gain(Percentage(50.0)).await?;

    println!("   Setting sharpness to 70%...");
    camera.set_sharpness(Percentage(70.0)).await?;

    println!("   Setting brightness to 50%...");
    camera.set_brightness(Percentage(50.0)).await?;

    println!("   Setting contrast to 60%...");
    camera.set_contrast(Percentage(60.0)).await?;

    println!("   Setting saturation to 75%...");
    camera.set_saturation(Percentage(75.0)).await?;

    println!("   Setting hue to 50%...");
    camera.set_hue(Percentage(50.0)).await?;

    // Using Raw Values
    println!("\n10. Using Raw Values for Fine Control");
    println!("   Setting gain to raw value 0x04 (12dB)...");
    camera.set_gain(Gain::new(0x04)?).await?;

    println!("   Setting sharpness to raw value 5...");
    camera.set_sharpness(SharpnessLevel::new(5)?).await?;

    println!("   Setting brightness to raw value 0x0C...");
    camera.set_brightness(BrightnessLevel::new(0x0C)?).await?;

    // Direct Type Usage
    println!("\n11. Direct Type Usage");

    println!("   Setting gain with typed value...");
    camera.set_gain(Gain::new(0x05)?).await?; // 15dB

    println!("   Setting sharpness with typed level...");
    camera.set_sharpness(SharpnessLevel::new(4)?).await?;

    println!("   Setting brightness with typed level...");
    camera.set_brightness(BrightnessLevel::new(0x08)?).await?;

    println!("\n✅ All semantic API operations completed successfully!");

    Ok(())
}
