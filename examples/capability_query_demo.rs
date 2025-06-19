//! Example demonstrating camera capability querying.

mod common;
use common::blocking::UdpTransport;
use grafton_visca::camera::{Camera, CameraProfile, GenericVisca, PTZOpticsG2, SonyEVID70};

// Include the transport implementation from the example file

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create cameras with different profiles using blocking transport
    let g2_camera = Camera::<PTZOpticsG2>::new(UdpTransport::new("192.168.1.100:1259"?));

    let sony_camera = Camera::<SonyEVID70>::new(UdpTransport::new("192.168.1.101:1259"?));

    let generic_camera = Camera::<GenericVisca>::new(UdpTransport::new("192.168.1.102:1259"?));

    println!("=== Camera Capability Comparison ===\n");

    // Compare basic capabilities
    print_basic_capabilities("PTZOptics G2", &g2_camera);
    print_basic_capabilities("Sony EVI-D70", &sony_camera);
    print_basic_capabilities("Generic VISCA", &generic_camera);

    // Get full capability summaries
    println!("\n=== Detailed Capability Summaries ===\n");

    let g2_summary = g2_camera.capability_summary();
    println!("PTZOptics G2 Full Capabilities:");
    print_capability_summary(&g2_summary);

    let sony_summary = sony_camera.capability_summary();
    println!("\nSony EVI-D70 Full Capabilities:");
    print_capability_summary(&sony_summary);

    // Demonstrate profile-specific capability checks
    println!("\n=== Profile-Specific Capabilities ===\n");

    let g2_profile = PTZOpticsG2;
    let sony_profile = SonyEVID70;

    println!("PTZOptics G2:");
    println!(
        "  - Wide Dynamic Range: {}",
        g2_profile.supports_wide_dynamic_range()
    );
    println!(
        "  - Image Stabilization: {}",
        g2_profile.supports_image_stabilization()
    );
    println!(
        "  - Low Light Mode: {}",
        g2_profile.supports_low_light_mode()
    );
    println!(
        "  - Noise Reduction: {}",
        g2_profile.supports_noise_reduction()
    );
    println!(
        "  - White Balance Modes: {}",
        g2_profile.white_balance_mode_count()
    );
    println!("  - Gain Range: {:?}", g2_profile.gain_range());

    println!("\nSony EVI-D70:");
    println!(
        "  - Wide Dynamic Range: {}",
        sony_profile.supports_wide_dynamic_range()
    );
    println!(
        "  - Image Stabilization: {}",
        sony_profile.supports_image_stabilization()
    );
    println!("  - Image Flip: {}", sony_profile.supports_image_flip());
    println!(
        "  - White Balance Modes: {}",
        sony_profile.white_balance_mode_count()
    );
    println!("  - Exposure Modes: {}", sony_profile.exposure_mode_count());
    println!("  - Max Preset ID: {}", SonyEVID70::max_preset_id());

    Ok(())
}

fn print_basic_capabilities<P: grafton_visca::camera::CameraProfile>(
    name: &str,
    camera: &Camera<P>,
) {
    let caps = camera.capabilities();
    println!("{} Basic Capabilities:", name);
    println!("  Model: {}", caps.model_name);
    println!("  Pan Range: {:?}°", caps.pan_range_degrees);
    println!("  Tilt Range: {:?}°", caps.tilt_range_degrees);
    println!("  Zoom Steps: {}", caps.zoom_steps);
    println!("  Focus Steps: {}", caps.focus_steps);
    println!("  Presets: {}", caps.preset_count);
    println!("  Digital Zoom: {}", caps.supports_digital_zoom);
    println!("  Max Pan Speed: {}", caps.max_pan_speed);
    println!("  Max Tilt Speed: {}", caps.max_tilt_speed);
    println!();
}

fn print_capability_summary(summary: &grafton_visca::camera::CapabilitySummary) {
    println!("  Movement:");
    println!("    - Continuous: {}", summary.movement.continuous);
    println!("    - Absolute: {}", summary.movement.absolute);
    println!("    - Relative: {}", summary.movement.relative);
    println!("    - Pan: {:?}°", summary.movement.pan_range);
    println!("    - Tilt: {:?}°", summary.movement.tilt_range);

    println!("  Zoom:");
    println!("    - Range: {:?}", summary.zoom.optical_range);
    println!("    - Digital: {}", summary.zoom.digital_zoom);
    println!("    - Speed Levels: {}", summary.zoom.speed_levels);

    println!("  Focus:");
    println!("    - Auto: {}", summary.focus.auto_focus);
    println!("    - Manual: {}", summary.focus.manual_focus);
    println!("    - Range: {:?}", summary.focus.range);

    println!("  Exposure:");
    println!("    - Auto: {}", summary.exposure.auto_exposure);
    println!("    - Manual: {}", summary.exposure.manual_exposure);
    println!("    - Shutter: {}", summary.exposure.shutter_control);
    println!("    - Iris: {}", summary.exposure.iris_control);
    println!("    - Gain: {}", summary.exposure.gain_control);
    println!("    - Backlight Comp: {}", summary.exposure.backlight_comp);
    println!("    - WDR: {}", summary.exposure.wide_dynamic_range);

    println!("  Image:");
    println!("    - White Balance: {}", summary.image.white_balance);
    println!("    - Color Control: {}", summary.image.color_control);
    println!("    - Flip: {}", summary.image.image_flip);
    println!("    - Stabilization: {}", summary.image.image_stabilization);
    println!("    - Noise Reduction: {}", summary.image.noise_reduction);
    println!("    - Low Light: {}", summary.image.low_light_mode);

    println!("  Presets:");
    println!("    - Count: {}", summary.presets.count);
    println!("    - Speed Support: {}", summary.presets.speed_support);
}
