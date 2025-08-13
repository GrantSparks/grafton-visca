//! Integration tests for the complete inquiry lifecycle.
//!
//! This module tests the end-to-end flow of inquiry commands from the camera
//! through the socket manager to the simulator and back.

#![cfg(feature = "rt-tokio")]

use grafton_visca::{
    camera::{AsyncMode, Camera},
    command::{
        exposure::ExposureMode, focus::FocusMode, preset::PresetNumber,
        white_balance::WhiteBalanceMode,
    },
    profiles::GenericVisca,
    testing::camera_simulator::ViscaCameraSimulator,
};
use std::time::Duration;
use tokio::time::timeout;

/// Test basic power inquiry through the full stack
#[tokio::test]
async fn test_power_inquiry_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Query power status
    let power_on = camera
        .power_inquiry()
        .await
        .expect("power inquiry should succeed");
    assert!(power_on, "Camera should be powered on by default");
}

/// Test position inquiries (pan/tilt, zoom, focus)
#[tokio::test]
async fn test_position_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test pan/tilt position inquiry
    let (pan, tilt) = camera
        .pan_tilt_position_inquiry()
        .await
        .expect("pan/tilt inquiry should succeed");
    assert_eq!(
        pan,
        grafton_visca::types::PanPosition::CENTER,
        "Pan should be at center"
    );
    assert_eq!(
        tilt,
        grafton_visca::types::TiltPosition::CENTER,
        "Tilt should be at center"
    );

    // Test zoom position inquiry
    let zoom_pos = camera
        .zoom_position_inquiry()
        .await
        .expect("zoom inquiry should succeed");
    assert_eq!(
        zoom_pos,
        grafton_visca::types::ZoomPosition::MIN,
        "Zoom should be at minimum"
    );

    // Test focus position inquiry
    let focus_pos = camera
        .focus_position_inquiry()
        .await
        .expect("focus inquiry should succeed");
    assert_eq!(
        focus_pos,
        grafton_visca::types::FocusPosition::new(0x1000).unwrap(),
        "Focus should be at default position"
    );

    // Test focus near limit inquiry
    let near_limit = camera
        .focus_near_limit_inquiry()
        .await
        .expect("focus near limit inquiry should succeed");
    assert_eq!(near_limit, 0x1000, "Focus near limit should be at default");
}

/// Test exposure-related inquiries
#[tokio::test]
async fn test_exposure_inquiries_integration() {
    let _ = env_logger::builder().is_test(true).try_init();
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test exposure mode inquiry
    let mode = camera
        .exposure_mode_inquiry()
        .await
        .expect("exposure mode inquiry should succeed");
    assert_eq!(mode, ExposureMode::Auto, "Exposure mode should be Auto");

    // Test exposure compensation inquiry
    let comp = camera
        .exposure_compensation_inquiry()
        .await
        .expect("exposure compensation inquiry should succeed");
    assert_eq!(comp, 0, "Exposure compensation should be 0");

    // Test exposure compensation mode inquiry
    let comp_enabled = camera
        .exposure_compensation_mode_inquiry()
        .await
        .expect("exposure compensation mode inquiry should succeed");
    assert!(!comp_enabled, "Exposure compensation should be disabled");

    // Test iris inquiry
    let iris = camera
        .iris_inquiry()
        .await
        .expect("iris inquiry should succeed");
    assert_eq!(iris, 0x0000, "Iris should be at minimum");

    // Test shutter inquiry
    let shutter = camera
        .shutter_inquiry()
        .await
        .expect("shutter inquiry should succeed");
    assert_eq!(shutter, 0x00, "Shutter should be at default");

    // Test brightness inquiry
    let brightness = camera
        .brightness_inquiry()
        .await
        .expect("brightness inquiry should succeed");
    assert_eq!(brightness, 0x07, "Brightness should be at default");

    // Test gain inquiry
    let gain = camera
        .gain_inquiry()
        .await
        .expect("gain inquiry should succeed");
    assert_eq!(gain, 0x00, "Gain should be at minimum");

    // Test gain limit inquiry
    let gain_limit = camera
        .gain_limit_inquiry()
        .await
        .expect("gain limit inquiry should succeed");
    assert_eq!(gain_limit, 0x07, "Gain limit should be at default");

    // Test backlight inquiry
    let backlight = camera
        .backlight_inquiry()
        .await
        .expect("backlight inquiry should succeed");
    assert!(!backlight, "Backlight should be disabled");
}

/// Test white balance and color inquiries
#[tokio::test]
async fn test_white_balance_color_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test white balance mode inquiry
    let wb_mode = camera
        .white_balance_mode_inquiry()
        .await
        .expect("white balance mode inquiry should succeed");
    assert_eq!(
        wb_mode,
        WhiteBalanceMode::Auto,
        "White balance should be Auto"
    );

    // Test color temperature inquiry
    let color_temp = camera
        .color_temperature_inquiry()
        .await
        .expect("color temperature inquiry should succeed");
    assert_eq!(color_temp, 2800, "Color temperature should be 2800K");
}

/// Test image adjustment inquiries
#[tokio::test]
async fn test_image_adjustment_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // NOTE: Sharpness and contrast inquiries are not documented in VISCA specs
    // and have been disabled until proper documentation is found.

    // // Test sharpness inquiry
    // let sharpness = camera
    //     .sharpness_inquiry()
    //     .await
    //     .expect("sharpness inquiry should succeed");
    // assert_eq!(sharpness, 0x07, "Sharpness should be at default");

    // // Test contrast inquiry
    // let contrast = camera
    //     .contrast_inquiry()
    //     .await
    //     .expect("contrast inquiry should succeed");
    // assert_eq!(contrast, 0x07, "Contrast should be at default");

    // Test saturation inquiry
    let saturation = camera
        .saturation_inquiry()
        .await
        .expect("saturation inquiry should succeed");
    assert_eq!(saturation, 0x07, "Saturation should be at default");

    // Test hue inquiry
    let hue = camera
        .hue_inquiry()
        .await
        .expect("hue inquiry should succeed");
    assert_eq!(hue, 0x07, "Hue should be at default");

    // Test image flip inquiry
    let (flip_v, flip_h) = camera
        .image_flip_inquiry()
        .await
        .expect("image flip inquiry should succeed");
    assert!(!flip_v, "Vertical flip should be off");
    assert!(!flip_h, "Horizontal flip should be off");
}

/// Test noise reduction inquiries
#[tokio::test]
async fn test_noise_reduction_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test noise reduction 2D inquiry
    let nr_2d = camera
        .noise_reduction_2d_inquiry()
        .await
        .expect("noise reduction 2D inquiry should succeed");
    assert_eq!(nr_2d, 0x01, "Noise reduction 2D should be at level 1");

    // Test noise reduction 3D inquiry
    let nr_3d = camera
        .noise_reduction_3d_inquiry()
        .await
        .expect("noise reduction 3D inquiry should succeed");
    assert_eq!(nr_3d, 0x01, "Noise reduction 3D should be at level 1");
}

/// Test focus mode inquiries
#[tokio::test]
async fn test_focus_mode_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test focus mode inquiry
    let focus_mode = camera
        .focus_mode_inquiry()
        .await
        .expect("focus mode inquiry should succeed");
    assert_eq!(focus_mode, FocusMode::Auto, "Focus mode should be Auto");

    // NOTE: AutoFocus inquiry is not documented in VISCA specs
    // and has been disabled until proper documentation is found.

    // // Test auto focus inquiry
    // let auto_focus = camera
    //     .auto_focus_inquiry()
    //     .await
    //     .expect("auto focus inquiry should succeed");
    // assert!(auto_focus, "Auto focus should be enabled");
}

/// Test resolution inquiry
#[tokio::test]
async fn test_resolution_inquiry_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test resolution inquiry
    let resolution = camera
        .resolution_inquiry()
        .await
        .expect("resolution inquiry should succeed");
    assert_eq!(resolution, 0x00, "Resolution should be 1080p (0x00)");
}

/// Test concurrent inquiries to verify socket manager handles them properly
#[tokio::test]
async fn test_concurrent_inquiries_integration() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Launch multiple inquiries concurrently
    let futures = vec![
        camera.power_inquiry(),
        camera.power_inquiry(), // Duplicate to test queuing
        camera.power_inquiry(),
    ];

    // All should complete successfully
    let results = futures::future::join_all(futures).await;
    for (i, result) in results.into_iter().enumerate() {
        assert!(
            result.is_ok(),
            "Concurrent inquiry #{} should succeed: {:?}",
            i + 1,
            result
        );
        assert!(result.unwrap(), "Power should be on");
    }
}

/// Test sequential inquiry commands
#[tokio::test]
async fn test_sequential_inquiries() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Test the failing sequence: exposure mode then exposure compensation
    let mode = camera
        .exposure_mode_inquiry()
        .await
        .expect("exposure mode inquiry should succeed");
    assert_eq!(mode, ExposureMode::Auto, "Exposure mode should be Auto");

    let comp = camera
        .exposure_compensation_inquiry()
        .await
        .expect("exposure compensation inquiry should succeed");
    assert_eq!(comp, 0, "Exposure compensation should be 0");
}

/// Test inquiry timeout behavior
#[tokio::test]
async fn test_inquiry_timeout_behavior() {
    use grafton_visca::testing::camera_simulator::SimulatorBuilder;

    // Create a simulator that delays inquiry responses
    let simulator = SimulatorBuilder::default()
        .with_command_execution_time(
            grafton_visca::testing::camera_simulator::CommandType::Inquiry,
            Duration::from_millis(100),
        )
        .build();

    let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
    let camera =
        Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator).with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Query should complete within reasonable time
    let result = timeout(Duration::from_millis(200), camera.power_inquiry()).await;
    assert!(result.is_ok(), "Inquiry should complete within timeout");
    assert!(
        result.unwrap().is_ok(),
        "Inquiry should succeed despite delay"
    );
}

/// Test mixed command and inquiry execution
#[tokio::test]
async fn test_mixed_commands_and_inquiries() {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = std::sync::Arc::new(TokioRuntime);
    let simulator = ViscaCameraSimulator::new();
    let camera = Camera::<AsyncMode, GenericVisca, _>::from_transport(simulator.clone())
        .with_runtime(runtime);
    // Socket manager is now automatically initialized on first use

    // Execute a preset recall command
    camera
        .preset_recall(PresetNumber::new(1).unwrap())
        .await
        .expect("preset recall should succeed");

    // Immediately query power status (should not be blocked by preset execution)
    let power_on = camera
        .power_inquiry()
        .await
        .expect("power inquiry should succeed during preset execution");
    assert!(power_on, "Power should remain on");

    // Query zoom position while preset is potentially still executing
    let zoom_pos = camera
        .zoom_position_inquiry()
        .await
        .expect("zoom inquiry should succeed");
    assert_eq!(
        zoom_pos,
        grafton_visca::types::ZoomPosition::MIN,
        "Zoom position should be readable"
    );
}
