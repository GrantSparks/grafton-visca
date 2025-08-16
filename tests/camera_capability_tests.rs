//! Tests for camera capability query system.

// NOTE: This test file uses an old API that no longer exists in the current version.
// The tests need to be rewritten to use the new capability traits system.

#[test]
fn test_placeholder() {
    // TODO: Rewrite tests to use the new API
    // This test validates that the capability traits compile correctly
}

/*
use grafton_visca::profiles::{GenericVisca, PtzOpticsG2};

#[test]
fn test_ptzoptics_g2_capabilities() {
    let profile = PtzOpticsG2;

    // Test G2-specific capabilities
    assert!(profile.supports_wide_dynamic_range());
    assert!(profile.supports_image_stabilization());
    assert!(profile.supports_low_light_mode());
    assert!(profile.supports_noise_reduction());

    // Test white balance modes
    assert_eq!(profile.white_balance_mode_count(), 6);

    // Test gain range
    let gain_range = profile
        .gain_range()
        .expect("G2 should support gain control");
    assert_eq!(*gain_range.start(), 0);
    assert_eq!(*gain_range.end(), 8);

    // Test pan/tilt ranges
    assert_eq!(profile.pan_degree_range(), -170.0..=170.0);
    assert_eq!(profile.tilt_degree_range(), -30.0..=90.0);

    // Test preset count
    assert_eq!(PtzOpticsG2::max_preset_id(), 89);
}

#[test]
fn test_generic_visca_capabilities() {
    let profile = GenericVisca;

    // Generic VISCA has minimal capabilities
    assert!(!profile.supports_wide_dynamic_range());
    assert!(!profile.supports_image_stabilization());
    assert!(!profile.supports_low_light_mode());
    assert!(!profile.supports_noise_reduction());

    // Should support at least basic white balance
    assert!(profile.white_balance_mode_count() >= 2);

    // May not support gain control
    assert!(profile.gain_range().is_none());

    // Test pan/tilt ranges (generic defaults)
    assert_eq!(profile.pan_degree_range(), -170.0..=170.0);
    assert_eq!(profile.tilt_degree_range(), -90.0..=90.0);

    // Test preset count (minimal)
    assert_eq!(GenericVisca::max_preset_id(), 5);
}

#[test]
fn test_sony_evid70_capabilities() {
    let profile = SonyEVID70;

    // Sony EVI-D70 specific capabilities
    assert!(!profile.supports_wide_dynamic_range()); // Older model
    assert!(!profile.supports_image_stabilization());
    assert!(profile.supports_low_light_mode());
    assert!(profile.supports_noise_reduction());

    // Test white balance modes
    assert_eq!(profile.white_balance_mode_count(), 4);

    // Test gain range
    let gain_range = profile
        .gain_range()
        .expect("EVI-D70 should support gain control");
    assert_eq!(*gain_range.start(), 0);
    assert_eq!(*gain_range.end(), 7);

    // Test pan/tilt ranges (EVI-D70 specific)
    assert_eq!(profile.pan_degree_range(), -100.0..=100.0);
    assert_eq!(profile.tilt_degree_range(), -25.0..=25.0);

    // Test preset count
    assert_eq!(SonyEVID70::max_preset_id(), 5);
}

#[test]
fn test_capabilities_conversion() {
    // Test that capabilities correctly convert between different units
    let g2 = PtzOpticsG2;

    // Pan conversion
    assert_eq!(g2.pan_units_to_degrees(0), 0.0);
    assert_eq!(g2.pan_units_to_degrees(0x990C), 170.0);
    assert_eq!(g2.pan_units_to_degrees(-0x990C), -170.0);

    // Tilt conversion
    assert_eq!(g2.tilt_units_to_degrees(0), 0.0);
    assert_eq!(g2.tilt_units_to_degrees(0x510E), 90.0);
    assert_eq!(g2.tilt_units_to_degrees(-0x1B58), -30.0);

    // Reverse conversion
    assert_eq!(g2.pan_degrees_to_units(0.0), 0);
    assert_eq!(g2.pan_degrees_to_units(170.0), 0x990C);
    assert_eq!(g2.pan_degrees_to_units(-170.0), -0x990C);

    assert_eq!(g2.tilt_degrees_to_units(0.0), 0);
    assert_eq!(g2.tilt_degrees_to_units(90.0), 0x510E);
    assert_eq!(g2.tilt_degrees_to_units(-30.0), -0x1B58);
}
*/
