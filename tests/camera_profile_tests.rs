//! Tests for camera profile constants and capabilities.

#[cfg(feature = "tokio")]
use grafton_visca::{Camera, CameraFeature, CameraModel, FeatureDetection};

#[cfg(feature = "tokio")]
mod common;

// Tests for camera profile metadata
// Note: Most profile constants are in private traits and can't be tested directly
// We test through the public API instead

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_coordinate_system_conversions() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test modern camera (signed coordinates)
    let modern_camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport.clone());
    let coords = modern_camera.to_camera_coords(0, 0);
    assert_eq!(coords, (0, 0));

    // Modern cameras use signed coordinates internally but the API returns u16
    // Test with positive coordinates
    let coords = modern_camera.to_camera_coords(100, 50);
    assert_eq!(coords, (100, 50));

    // Test legacy camera (unsigned coordinates)
    let legacy_camera = Camera::with_profile(CameraModel::SonyBRC300, transport.clone());
    let coords = legacy_camera.to_camera_coords(0, 0);
    assert_eq!(coords, (0x8000, 0x8000)); // Center is 0x8000

    let coords = legacy_camera.to_camera_coords(100, 100);
    assert_eq!(coords, (0x8064, 0x8064)); // 0x8000 + 100

    // Test conversion back
    let logical = legacy_camera.from_camera_coords(0x8000, 0x8000);
    assert_eq!(logical, (0, 0));

    let logical = legacy_camera.from_camera_coords(0x8064, 0x8064);
    assert_eq!(logical, (100, 100));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_camera_profile_selection() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test that each model creates the correct profile
    let models_and_names = vec![
        (CameraModel::PTZOpticsG2, "PTZOptics G2"),
        (CameraModel::PTZOpticsG3, "PTZOptics G3"),
        (CameraModel::PTZOptics30X, "PTZOptics 30X"),
        (CameraModel::SonyFR7, "Sony FR7"),
        (CameraModel::SonyBRCH900, "Sony BRC-H900"),
        (CameraModel::SonyEVIH100, "Sony EVI-H100"),
        (CameraModel::SonyBRC300, "Sony BRC-300"),
        (CameraModel::NearusBRC300, "Nearus BRC-300"),
        (CameraModel::GenericVisca, "Generic VISCA Camera"),
    ];

    for (model, expected_name) in models_and_names {
        let camera = Camera::with_profile(model, transport.clone());
        assert_eq!(camera.model_name(), expected_name);
    }
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_profile_metadata() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test that each camera has appropriate model name
    let cameras = vec![
        (CameraModel::PTZOpticsG2, "PTZOptics G2"),
        (CameraModel::SonyFR7, "Sony FR7"),
        (CameraModel::SonyBRCH900, "Sony BRC-H900"),
        (CameraModel::SonyEVIH100, "Sony EVI-H100"),
        (CameraModel::SonyBRC300, "Sony BRC-300"),
        (CameraModel::NearusBRC300, "Nearus BRC-300"),
        (CameraModel::PTZOpticsG3, "PTZOptics G3"),
        (CameraModel::PTZOptics30X, "PTZOptics 30X"),
        (CameraModel::GenericVisca, "Generic VISCA Camera"),
    ];

    for (model, expected_name) in cameras {
        let camera = Camera::with_profile(model, transport.clone());
        assert_eq!(camera.model_name(), expected_name);
    }
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_preset_support() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test that all cameras support presets
    let models = vec![
        CameraModel::SonyEVIH100, // Has very limited presets (6)
        CameraModel::SonyBRC300,  // Has limited presets (16)
        CameraModel::PTZOpticsG2, // Has standard presets (89)
        CameraModel::SonyBRCH900, // Has extended presets (100)
        CameraModel::PTZOpticsG3, // Has maximum presets (255)
        CameraModel::PTZOptics30X,
        CameraModel::NearusBRC300,
        CameraModel::GenericVisca,
    ];

    for model in models {
        let camera = Camera::with_profile(model, transport.clone());
        assert!(
            camera.supports_feature(CameraFeature::Presets),
            "Camera {:?} should support presets",
            model
        );
    }
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_feature_combinations() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test specific feature combinations for different camera models

    // Sony FR7: Has ND filter but no motion sync
    let fr7 = Camera::with_profile(CameraModel::SonyFR7, transport.clone());
    assert!(fr7.supports_feature(CameraFeature::NDFilter));
    assert!(!fr7.supports_feature(CameraFeature::MotionSync));

    // PTZOptics G2: Has motion sync but no ND filter
    let g2 = Camera::with_profile(CameraModel::PTZOpticsG2, transport.clone());
    assert!(!g2.supports_feature(CameraFeature::NDFilter));
    assert!(g2.supports_feature(CameraFeature::MotionSync));

    // Sony BRC-300: Legacy camera with basic features
    let brc300 = Camera::with_profile(CameraModel::SonyBRC300, transport.clone());
    assert!(brc300.supports_feature(CameraFeature::PanTilt));
    assert!(brc300.supports_feature(CameraFeature::Zoom));
    assert!(!brc300.supports_feature(CameraFeature::ImageFlip)); // No modern features
}
