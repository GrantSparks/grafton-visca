//! Example demonstrating camera feature introspection using CameraFeature enum
//!
//! This example shows how the CameraFeature enum can be used to:
//! - List all possible camera features
//! - Get human-readable names and descriptions

use grafton_visca::CameraFeature;

fn main() {
    println!("VISCA Camera Features Reference\n");
    println!("This library supports the following camera features:");
    println!("{}", "=".repeat(70));

    // List all camera features with their descriptions
    let features = vec![
        CameraFeature::PanTilt,
        CameraFeature::Zoom,
        CameraFeature::MotionSync,
        CameraFeature::Focus,
        CameraFeature::FocusLock,
        CameraFeature::Exposure,
        CameraFeature::Gain,
        CameraFeature::Shutter,
        CameraFeature::Iris,
        CameraFeature::Backlight,
        CameraFeature::WhiteBalance,
        CameraFeature::ImageProcessing,
        CameraFeature::ColorSaturation,
        CameraFeature::Gamma,
        CameraFeature::BlackWhiteMode,
        CameraFeature::NoiseReduction,
        CameraFeature::Sharpness,
        CameraFeature::Power,
        CameraFeature::Presets,
        CameraFeature::Tally,
        CameraFeature::ImageFreeze,
        CameraFeature::ImageFlip,
        CameraFeature::PictureEffect,
        CameraFeature::NDFilter,
        CameraFeature::VariableSpeedMode,
        CameraFeature::MenuControl,
        CameraFeature::Privacy,
        CameraFeature::SystemReset,
        CameraFeature::CommandCancel,
        CameraFeature::NDI,
    ];

    for feature in features {
        println!("{:20} - {}", feature.name(), feature.description());
    }

    println!("\n");
    println!("Note: Not all cameras support all features. Check your camera's");
    println!("documentation or use the ProfileIntrospection trait to determine");
    println!("which features are available for a specific camera model.");
}
