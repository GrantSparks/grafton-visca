//! Demonstrates the semantic type API for intuitive camera control.
//!
//! This example shows how to use semantic types instead of raw VISCA values
//! for more intuitive and type-safe camera control.

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::{Camera, PTZOpticsG2},
        transport::blocking::UdpTransport,
        units::{Degrees, Fraction, Kelvin, Magnification, Percentage, Radians},
        FStop,
    };
    use std::f32::consts::PI;

    // Connect to camera
    let transport = UdpTransport::connect("192.168.1.100:52381")?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Semantic API Demo ===\n");

    // Zoom Control - Multiple Ways
    println!("1. Zoom Control");
    println!("   Setting zoom to 50% using percentage...");
    camera.set_zoom_percentage(Percentage(50.0))?;

    println!("   Setting zoom to 10x magnification...");
    camera.set_zoom_magnification(Magnification(10.0))?;

    println!("   Setting zoom using raw VISCA value...");
    camera.set_zoom_raw(0x3000u16)?;

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to 75% (near)...");
    camera.set_focus_percentage(Percentage(75.0))?;

    println!("   Setting focus using raw value...");
    camera.set_focus_raw(0x8000u16)?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to 45° right, 15° up...");
    camera.set_position(Degrees(45.0), Degrees(-15.0))?;

    println!("   Moving using radians...");
    camera.set_position_radians(Radians(PI / 4.0), Radians(-PI / 12.0))?;

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
    camera.set_iris_fstop(FStop::F2_8)?;

    println!("   Setting iris to 50% open...");
    camera.set_iris_percentage(Percentage(50.0))?;

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
    camera.stop()?;

    // Convenience Methods
    println!("\n8. Convenience Methods");
    println!("   Setting zoom to 25%...");
    camera.set_zoom_percentage(Percentage(25.0))?;

    println!("   Setting zoom to 5x magnification...");
    camera.set_zoom_magnification(Magnification(5.0))?;

    println!("   Setting focus to infinity (0%)...");
    camera.set_focus_percentage(Percentage(0.0))?;

    println!("   Setting iris to 75% open...");
    camera.set_iris_percentage(Percentage(75.0))?;

    // Image Quality Controls
    println!("\n9. Image Quality Controls");
    println!("   Setting gain to 50% (~10.5dB)...");
    camera.set_gain_percentage(Percentage(50.0))?;

    println!("   Setting sharpness to 70%...");
    camera.set_sharpness_percentage(Percentage(70.0))?;

    println!("   Setting brightness to 50%...");
    camera.set_brightness_percentage(Percentage(50.0))?;

    println!("   Setting contrast to 60%...");
    camera.set_contrast_percentage(Percentage(60.0))?;

    println!("   Setting saturation to 75%...");
    camera.set_saturation_percentage(Percentage(75.0))?;

    println!("   Setting hue to 50%...");
    camera.set_hue_percentage(Percentage(50.0))?;

    // Using Raw Values
    println!("\n10. Using Raw Values for Fine Control");
    println!("   Setting gain to raw value 0x04 (12dB)...");
    camera.set_gain_raw(0x04u8)?;

    println!("   Setting sharpness to raw value 5...");
    camera.set_sharpness_raw(5u8)?;

    println!("   Setting brightness to raw value 0x0C...");
    camera.set_brightness_raw(0x0Cu16)?;

    // Direct Type Usage
    println!("\n11. Direct Type Usage");
    use grafton_visca::types::{BrightnessLevel, GainValue, SharpnessLevel};

    println!("   Setting gain with typed value...");
    camera.set_gain(GainValue::new(0x05)?)?; // 15dB

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
        camera::{Camera, PTZOpticsG2},
        transport::tokio::UdpTransport,
        units::{Degrees, Fraction, Kelvin, Magnification, Percentage, Radians},
        FStop,
    };
    use std::f32::consts::PI;

    // Connect to camera
    let transport = UdpTransport::new("0.0.0.0:0", "192.168.1.100:52381").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Semantic API Demo (Async) ===\n");

    // Zoom Control - Multiple Ways
    println!("1. Zoom Control");
    println!("   Setting zoom to 50% using percentage...");
    camera.set_zoom_percentage(Percentage(50.0)).await?;

    println!("   Setting zoom to 10x magnification...");
    camera.set_zoom_magnification(Magnification(10.0)).await?;

    println!("   Setting zoom using raw VISCA value...");
    camera.set_zoom_raw(0x3000u16).await?;

    // Focus Control
    println!("\n2. Focus Control");
    println!("   Setting focus to 75% (near)...");
    camera.set_focus_percentage(Percentage(75.0)).await?;

    println!("   Setting focus using raw value...");
    camera.set_focus_raw(0x8000u16).await?;

    // Pan/Tilt Control
    println!("\n3. Pan/Tilt Control");
    println!("   Moving to 45° right, 15° up...");
    camera.set_position(Degrees(45.0), Degrees(-15.0)).await?;

    println!("   Moving using radians...");
    camera
        .set_position_radians(Radians(PI / 4.0), Radians(-PI / 12.0))
        .await?;

    // White Balance
    println!("\n4. White Balance");
    println!("   Setting to daylight (5600K)...");
    camera.set_white_balance_kelvin(Kelvin(5600)).await?;

    println!("   Setting to tungsten (3200K)...");
    camera.set_white_balance_kelvin(Kelvin(3200)).await?;

    // Shutter Speed
    println!("\n5. Shutter Speed");
    println!("   Setting to 1/60s...");
    camera.set_shutter_fraction(Fraction::new(1, 60)).await?;

    println!("   Setting to 1/1000s...");
    camera.set_shutter_fraction(Fraction::new(1, 1000)).await?;

    // Iris Control
    println!("\n6. Iris Control");
    println!("   Setting iris to F2.8...");
    camera.set_iris_fstop(FStop::F2_8).await?;

    println!("   Setting iris to 50% open...");
    camera.set_iris_percentage(Percentage(50.0)).await?;

    // Movement with Percentage Speeds
    println!("\n7. Movement Control");
    println!("   Moving right at 50% speed...");
    camera
        .move_percentage(
            grafton_visca::command::pan_tilt::PanTiltDirection::Right,
            Percentage(50.0),
            Percentage(0.0),
        )
        .await?;

    println!("   Moving diagonally at 75% speed...");
    camera
        .move_percentage(
            grafton_visca::command::pan_tilt::PanTiltDirection::UpRight,
            Percentage(75.0),
            Percentage(75.0),
        )
        .await?;

    println!("\n   Stopping movement...");
    camera.stop().await?;

    // Convenience Methods
    println!("\n8. Convenience Methods");
    println!("   Setting zoom to 25%...");
    camera.set_zoom_percentage(Percentage(25.0)).await?;

    println!("   Setting zoom to 5x magnification...");
    camera.set_zoom_magnification(Magnification(5.0)).await?;

    println!("   Setting focus to infinity (0%)...");
    camera.set_focus_percentage(Percentage(0.0)).await?;

    println!("   Setting iris to 75% open...");
    camera.set_iris_percentage(Percentage(75.0)).await?;

    // Image Quality Controls
    println!("\n9. Image Quality Controls");
    println!("   Setting gain to 50% (~10.5dB)...");
    camera.set_gain_percentage(Percentage(50.0)).await?;

    println!("   Setting sharpness to 70%...");
    camera.set_sharpness_percentage(Percentage(70.0)).await?;

    println!("   Setting brightness to 50%...");
    camera.set_brightness_percentage(Percentage(50.0)).await?;

    println!("   Setting contrast to 60%...");
    camera.set_contrast_percentage(Percentage(60.0)).await?;

    println!("   Setting saturation to 75%...");
    camera.set_saturation_percentage(Percentage(75.0)).await?;

    println!("   Setting hue to 50%...");
    camera.set_hue_percentage(Percentage(50.0)).await?;

    // Using Raw Values
    println!("\n10. Using Raw Values for Fine Control");
    println!("   Setting gain to raw value 0x04 (12dB)...");
    camera.set_gain_raw(0x04u8).await?;

    println!("   Setting sharpness to raw value 5...");
    camera.set_sharpness_raw(5u8).await?;

    println!("   Setting brightness to raw value 0x0C...");
    camera.set_brightness_raw(0x0Cu16).await?;

    // Direct Type Usage
    println!("\n11. Direct Type Usage");
    use grafton_visca::types::{BrightnessLevel, GainValue, SharpnessLevel};

    println!("   Setting gain with typed value...");
    camera.set_gain(GainValue::new(0x05)?).await?; // 15dB

    println!("   Setting sharpness with typed level...");
    camera.set_sharpness(SharpnessLevel::new(4)?).await?;

    println!("   Setting brightness with typed level...");
    camera.set_brightness(BrightnessLevel::new(0x08)?).await?;

    println!("\n✅ All semantic API operations completed successfully!");

    Ok(())
}
