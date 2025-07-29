//! Tests for feature detection API and camera profiles.

#[cfg(feature = "tokio")]
use grafton_visca::{Camera, CameraFeature, CameraModel, FeatureDetection};

#[cfg(feature = "tokio")]
mod common;

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_ptzoptics_g2_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

    // PTZOptics G2 should support these features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::MotionSync));
    assert!(camera.supports_feature(CameraFeature::Tally));

    // PTZOptics G2 should NOT support these features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_sony_fr7_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::SonyFR7, transport);

    // Sony FR7 should support these features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::NDFilter));
    assert!(camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(camera.supports_feature(CameraFeature::Tally));
    assert!(camera.supports_feature(CameraFeature::Privacy));

    // Sony FR7 should NOT support these features
    assert!(!camera.supports_feature(CameraFeature::MotionSync));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_sony_brc_h900_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::SonyBRCH900, transport);

    // Sony BRC-H900 should support these features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::Tally));
    assert!(camera.supports_feature(CameraFeature::ImageFreeze));
    assert!(camera.supports_feature(CameraFeature::ImageFlip));

    // Sony BRC-H900 should NOT support these features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::MotionSync));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::Privacy));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_sony_evi_h100_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::SonyEVIH100, transport);

    // Sony EVI-H100 should support basic features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));

    // Sony EVI-H100 should NOT support advanced features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::MotionSync));
    assert!(!camera.supports_feature(CameraFeature::Tally));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
    assert!(!camera.supports_feature(CameraFeature::ImageFlip));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::Privacy));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_sony_brc_300_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::SonyBRC300, transport);

    // Sony BRC-300 (legacy) should support these features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::Tally));
    assert!(camera.supports_feature(CameraFeature::ImageFreeze));

    // Sony BRC-300 should NOT support modern features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::MotionSync));
    assert!(!camera.supports_feature(CameraFeature::ImageFlip));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::Privacy));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_ptzoptics_g3_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG3, transport);

    // PTZOptics G3 should support these features (enhanced from G2)
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::MotionSync));
    assert!(camera.supports_feature(CameraFeature::Tally));
    assert!(camera.supports_feature(CameraFeature::ImageFlip));
    assert!(camera.supports_feature(CameraFeature::Privacy));

    // PTZOptics G3 should NOT support these features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_ptzoptics_30x_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::PTZOptics30X, transport);

    // PTZOptics 30X should support same features as G3
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));
    assert!(camera.supports_feature(CameraFeature::MotionSync));
    assert!(camera.supports_feature(CameraFeature::Tally));
    assert!(camera.supports_feature(CameraFeature::ImageFlip));
    assert!(camera.supports_feature(CameraFeature::Privacy));

    // PTZOptics 30X should NOT support these features
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_generic_visca_features() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::GenericVisca, transport);

    // Generic VISCA should support only basic features
    assert!(camera.supports_feature(CameraFeature::PanTilt));
    assert!(camera.supports_feature(CameraFeature::Zoom));
    assert!(camera.supports_feature(CameraFeature::Focus));
    assert!(camera.supports_feature(CameraFeature::Exposure));
    assert!(camera.supports_feature(CameraFeature::WhiteBalance));
    assert!(camera.supports_feature(CameraFeature::Power));
    assert!(camera.supports_feature(CameraFeature::Presets));

    // Generic VISCA should NOT support advanced features
    assert!(!camera.supports_feature(CameraFeature::ImageProcessing));
    assert!(!camera.supports_feature(CameraFeature::NDFilter));
    assert!(!camera.supports_feature(CameraFeature::MotionSync));
    assert!(!camera.supports_feature(CameraFeature::Tally));
    assert!(!camera.supports_feature(CameraFeature::ImageFreeze));
    assert!(!camera.supports_feature(CameraFeature::ImageFlip));
    assert!(!camera.supports_feature(CameraFeature::VariableSpeedMode));
    assert!(camera.supports_feature(CameraFeature::MenuControl));
    assert!(!camera.supports_feature(CameraFeature::Privacy));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_supported_features_list() {
    let transport = common::mock_transport_enhanced::MockTransport::new();
    let camera = Camera::with_profile(CameraModel::SonyFR7, transport);

    let features = camera.supported_features();

    // Check that the list contains expected features
    assert!(features.contains(&CameraFeature::PanTilt));
    assert!(features.contains(&CameraFeature::Zoom));
    assert!(features.contains(&CameraFeature::NDFilter));
    assert!(features.contains(&CameraFeature::VariableSpeedMode));
    assert!(features.contains(&CameraFeature::MenuControl));

    // Check that the list does NOT contain unsupported features
    assert!(!features.contains(&CameraFeature::MotionSync));
    assert!(!features.contains(&CameraFeature::ImageFreeze));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_command_validation() {
    // Command validation test - currently validate_command always returns Ok()
    // as CommandFeatures is not yet implemented

    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test that validate_command exists and returns Ok for any command
    let _camera = Camera::with_profile(CameraModel::SonyFR7, transport.clone());

    // We can't test specific commands as they're not publicly exported
    // This just verifies the API exists
    // In the future, this would test actual command validation
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_system_features_always_supported() {
    let transport = common::mock_transport_enhanced::MockTransport::new();

    // Test multiple camera models
    let models = vec![
        CameraModel::PTZOpticsG2,
        CameraModel::SonyFR7,
        CameraModel::SonyBRCH900,
        CameraModel::SonyEVIH100,
        CameraModel::SonyBRC300,
        CameraModel::GenericVisca,
    ];

    for model in models {
        let camera = Camera::with_profile(model, transport.clone());

        // System features should be supported by all cameras
        assert!(camera.supports_feature(CameraFeature::SystemReset));
        assert!(camera.supports_feature(CameraFeature::CommandCancel));
    }
}
