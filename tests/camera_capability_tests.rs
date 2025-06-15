//! Tests for camera capability query system.

use grafton_visca::camera::{CameraProfile, GenericVisca, PTZOpticsG2, SonyEVID70};

#[test]
fn test_ptzoptics_g2_capabilities() {
    let profile = PTZOpticsG2::default();
    
    // Test G2-specific capabilities
    assert!(profile.supports_wide_dynamic_range());
    assert!(profile.supports_image_stabilization());
    assert!(profile.supports_low_light_mode());
    assert!(profile.supports_noise_reduction());
    
    // Test white balance modes
    assert_eq!(profile.white_balance_mode_count(), 6);
    
    // Test gain range
    let gain_range = profile.gain_range().expect("G2 should support gain control");
    assert_eq!(*gain_range.start(), 0);
    assert_eq!(*gain_range.end(), 8);
    
    // Test pan/tilt ranges
    assert_eq!(profile.pan_degree_range(), -170.0..=170.0);
    assert_eq!(profile.tilt_degree_range(), -30.0..=90.0);
    
    // Test preset count
    assert_eq!(PTZOpticsG2::max_preset_id(), 89);
}

#[test]
fn test_sony_evid70_capabilities() {
    let profile = SonyEVID70::default();
    
    // Test Sony-specific capabilities
    assert!(!profile.supports_wide_dynamic_range());
    assert!(!profile.supports_image_stabilization());
    assert!(!profile.supports_low_light_mode());
    assert!(!profile.supports_noise_reduction());
    assert!(!profile.supports_image_flip());
    
    // Test white balance and exposure modes
    assert_eq!(profile.white_balance_mode_count(), 4);
    assert_eq!(profile.exposure_mode_count(), 3);
    
    // Test pan/tilt ranges
    assert_eq!(profile.pan_degree_range(), -100.0..=100.0);
    assert_eq!(profile.tilt_degree_range(), -25.0..=25.0);
    
    // Test preset count
    assert_eq!(SonyEVID70::max_preset_id(), 5);
}

#[test]
fn test_generic_visca_capabilities() {
    let profile = GenericVisca::default();
    
    // Test default capabilities
    assert!(profile.supports_auto_focus());
    assert!(profile.supports_manual_focus());
    assert!(profile.supports_white_balance());
    assert!(profile.supports_continuous_movement());
    assert!(profile.supports_absolute_positioning());
    
    // Test default ranges
    assert_eq!(profile.shutter_speed_range(), Some(0..=21));
    assert_eq!(profile.iris_range(), Some(0..=20));
    assert_eq!(profile.gain_range(), Some(0..=15));
    
    // Test pan/tilt ranges (generic assumes ±180° pan, ±90° tilt)
    let pan_range = profile.pan_degree_range();
    assert!((pan_range.start() - -180.0).abs() < 0.01);
    assert!((pan_range.end() - 180.0).abs() < 0.01);
    
    let tilt_range = profile.tilt_degree_range();
    assert!((tilt_range.start() - -90.0).abs() < 0.01);
    assert!((tilt_range.end() - 90.0).abs() < 0.01);
}

#[test]
fn test_capability_summary() {
    let g2_profile = PTZOpticsG2::default();
    let summary = g2_profile.capability_summary();
    
    // Verify summary structure
    assert_eq!(summary.model, "PTZOptics G2");
    assert!(summary.movement.continuous);
    assert!(summary.movement.absolute);
    assert!(summary.movement.relative);
    assert_eq!(summary.movement.max_pan_speed, 24);
    assert_eq!(summary.movement.max_tilt_speed, 20);
    
    assert!(summary.zoom.digital_zoom);
    assert_eq!(summary.zoom.speed_levels, 8);
    
    assert!(summary.focus.auto_focus);
    assert!(summary.focus.manual_focus);
    assert_eq!(summary.focus.speed_levels, 8);
    
    assert!(summary.exposure.auto_exposure);
    assert!(summary.exposure.wide_dynamic_range);
    assert!(summary.exposure.backlight_comp);
    
    assert!(summary.image.image_stabilization);
    assert!(summary.image.noise_reduction);
    assert!(summary.image.low_light_mode);
    
    assert_eq!(summary.presets.count, 90); // 0-89 = 90 presets
    assert!(summary.presets.speed_support);
}

#[test]
fn test_zoom_and_focus_speed_support() {
    let profile = GenericVisca::default();
    
    // Test zoom speeds
    for speed in 0..=7 {
        assert!(profile.supports_zoom_speed(speed));
    }
    assert!(!profile.supports_zoom_speed(8));
    assert!(!profile.supports_zoom_speed(255));
    
    // Test focus speeds
    for speed in 0..=7 {
        assert!(profile.supports_focus_speed(speed));
    }
    assert!(!profile.supports_focus_speed(8));
    assert!(!profile.supports_focus_speed(100));
}

#[test]
fn test_optional_capability_ranges() {
    let profile = GenericVisca::default();
    
    // Test that ranges return None when feature is not supported
    // (In our implementation, generic VISCA supports all these, but this
    // demonstrates the pattern for cameras that don't)
    
    let shutter_range = profile.shutter_speed_range();
    assert!(shutter_range.is_some());
    
    let iris_range = profile.iris_range();
    assert!(iris_range.is_some());
    
    let gain_range = profile.gain_range();
    assert!(gain_range.is_some());
}