//! Integration tests for the complete inquiry lifecycle.
//!
//! This module tests the end-to-end flow of inquiry commands from the camera
//! through the socket manager to the simulator and back.

#![cfg(all(feature = "runtime-tokio", feature = "test-utils"))]

use tokio::join;

use std::time::Duration;

use grafton_visca::{
    camera::{profiles::GenericVisca, CameraBuilder},
    capabilities::{
        exposure::ShutterSpeed, Exposure, Focus, HasBacklightCompensation, HasBrightnessControl,
        HasColorTemperature, HasContrastControl, HasExposureCompensation, HasHueControl,
        HasImageFlip, HasIrisControl, HasLuminanceControl, HasNoiseReduction2D,
        HasNoiseReduction3D, HasSaturationControl, HasSharpnessControl, ImageProcessing,
        InquirySupport, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt, Power,
        Presets, ProfileMetadata, Tally, VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    command::{ExposureMode, FocusMode, WhiteBalanceMode},
    runtime::TokioRuntime,
    testing::camera_simulator::{SimulatorBuilder, ViscaCameraSimulator},
    transport::RawVisca,
    ResolutionMode,
};

const SIM_EXPOSURE_MODES: &[ExposureMode] = &[
    ExposureMode::Auto,
    ExposureMode::Manual,
    ExposureMode::Shutter,
    ExposureMode::Iris,
    ExposureMode::Bright,
];
const SIM_SHUTTER_SPEEDS: &[ShutterSpeed] = &[ShutterSpeed::new("1/60", 0x01)];
const SIM_WB_MODES: &[WhiteBalanceMode] = &[
    WhiteBalanceMode::Auto,
    WhiteBalanceMode::Indoor,
    WhiteBalanceMode::Outdoor,
    WhiteBalanceMode::OnePush,
    WhiteBalanceMode::Manual,
    WhiteBalanceMode::ColorTemperature,
];

#[derive(Debug, Default, Clone, Copy)]
struct SimulatorFullProfile;

impl ProfileMetadata for SimulatorFullProfile {
    const MODEL_NAME: &'static str = "Simulator Full Profile";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(200);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(10_000);
    const INQUIRY_SUPPORT: InquirySupport = InquirySupport::Full;
}

impl PanTilt for SimulatorFullProfile {
    const PAN_RANGE: std::ops::Range<i16> = -2880..2881;
    const TILT_RANGE: std::ops::Range<i16> = -1440..1441;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 24;
    const PAN_DEGREES_TO_UNITS: f32 = 16.0;
    const TILT_DEGREES_TO_UNITS: f32 = 16.0;
}

impl Zoom for SimulatorFullProfile {
    const OPTICAL_ZOOM_MAX: u16 = 0xFFFF;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const SUPPORTS_DIRECT_ZOOM: bool = false;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 1000.0;
}

impl Power for SimulatorFullProfile {
    const POWER_ON_TIME: Duration = Duration::from_secs(30);
    const SUPPORTS_STANDBY: bool = false;
}

impl MenuCapability for SimulatorFullProfile {}

impl Exposure for SimulatorFullProfile {
    const EXPOSURE_MODES: &'static [ExposureMode] = SIM_EXPOSURE_MODES;
    const IRIS_RANGE: Option<std::ops::Range<u16>> = Some(0x00..0x1D);
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = SIM_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..16;
    const BRIGHTNESS_RANGE: Option<std::ops::Range<u16>> = Some(0..18);
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_EXPOSURE_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for SimulatorFullProfile {
    const WB_MODES: &'static [WhiteBalanceMode] = SIM_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const SUPPORTS_COLOR_TEMP: bool = true;
    const COLOR_TEMP_RANGE: Option<std::ops::Range<u16>> = Some(2800..7500);
    const SUPPORTS_RGB_GAIN: bool = true;
    const RED_GAIN_RANGE: Option<std::ops::Range<u8>> = Some(0..255);
    const BLUE_GAIN_RANGE: Option<std::ops::Range<u8>> = Some(0..255);
}

impl Focus for SimulatorFullProfile {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xE000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}

impl ImageProcessing for SimulatorFullProfile {
    const CONTRAST_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SHARPNESS_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_HUE: bool = true;
    const HUE_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
    const SUPPORTS_LUMINANCE: bool = true;
    const LUMINANCE_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
}

impl Presets for SimulatorFullProfile {
    const MAX_PRESETS: u8 = 6;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..24;
    const SUPPORTS_PRESET_TOUR: bool = false;
    const SUPPORTS_PRESET_THUMBNAIL: bool = false;
}

impl Tally for SimulatorFullProfile {}
impl MotionSyncMetadata for SimulatorFullProfile {}
impl NdFilterMetadata for SimulatorFullProfile {}
impl VariableSpeedMetadata for SimulatorFullProfile {}

impl HasBacklightCompensation for SimulatorFullProfile {}
impl HasBrightnessControl for SimulatorFullProfile {}
impl HasColorTemperature for SimulatorFullProfile {}
impl HasContrastControl for SimulatorFullProfile {}
impl HasExposureCompensation for SimulatorFullProfile {}
impl HasHueControl for SimulatorFullProfile {}
impl HasImageFlip for SimulatorFullProfile {}
impl HasIrisControl for SimulatorFullProfile {}
impl HasLuminanceControl for SimulatorFullProfile {}
impl HasNoiseReduction2D for SimulatorFullProfile {}
impl HasNoiseReduction3D for SimulatorFullProfile {}
impl HasSaturationControl for SimulatorFullProfile {}
impl HasSharpnessControl for SimulatorFullProfile {}

/// Test basic power inquiry through the full stack
#[tokio::test(start_paused = true)]
async fn test_power_inquiry_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();

    // Query power status
    let power_on = camera
        .power()
        .state()
        .await
        .expect("power inquiry should succeed");
    assert!(power_on, "Camera should be powered on by default");
}

/// Test position inquiries (pan/tilt, zoom, focus)
#[tokio::test(start_paused = true)]
async fn test_position_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();

    // Test pan/tilt position inquiry
    let position = camera
        .pan_tilt()
        .position()
        .await
        .expect("pan/tilt inquiry should succeed");
    assert_eq!(
        position.pan,
        0, // CENTER position is 0
        "Pan should be at center"
    );
    assert_eq!(
        position.tilt,
        0, // CENTER position is 0
        "Tilt should be at center"
    );

    // Test zoom position inquiry
    let zoom_pos = camera
        .zoom()
        .position()
        .await
        .expect("zoom inquiry should succeed");
    assert_eq!(
        zoom_pos,
        grafton_visca::types::ZoomPosition::MIN,
        "Zoom should be at minimum"
    );

    // Test focus position inquiry
    let focus_pos = camera
        .focus()
        .position()
        .await
        .expect("focus inquiry should succeed");
    assert_eq!(
        focus_pos,
        grafton_visca::types::FocusPosition::new(0x1000), // Default focus position
        "Focus should be at default position"
    );

    // Test focus near limit inquiry
    let near_limit = camera
        .focus()
        .near_limit()
        .await
        .expect("focus near limit inquiry should succeed");
    assert_eq!(
        near_limit,
        grafton_visca::types::FocusPosition::new(0x1000),
        "Focus near limit should be at default"
    );
}

/// Test exposure-related inquiries
#[tokio::test(start_paused = true)]
async fn test_exposure_inquiries_integration() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<SimulatorFullProfile, _>(simulator)
        .await
        .unwrap();

    // Test exposure mode inquiry
    let mode = camera
        .exposure()
        .mode()
        .await
        .expect("exposure mode inquiry should succeed");
    assert_eq!(mode, ExposureMode::Auto, "Exposure mode should be Auto");

    // Test exposure compensation inquiry
    let comp = camera
        .exposure()
        .compensation()
        .await
        .expect("exposure compensation inquiry should succeed");
    assert_eq!(
        comp,
        grafton_visca::types::ExposureCompensationLevel::new(0).unwrap(),
        "Exposure compensation should be 0"
    );

    // Test exposure compensation mode inquiry
    let comp_enabled = camera
        .exposure()
        .compensation_enabled()
        .await
        .expect("exposure compensation mode inquiry should succeed");
    assert!(!comp_enabled, "Exposure compensation should be disabled");

    // Test iris inquiry
    let iris = camera
        .exposure()
        .iris()
        .await
        .expect("iris inquiry should succeed");
    assert_eq!(
        iris,
        grafton_visca::types::IrisLevel::new(0x0000).unwrap(),
        "Iris should be at minimum"
    );

    // Test shutter inquiry
    let shutter = camera
        .exposure()
        .shutter()
        .await
        .expect("shutter inquiry should succeed");
    // ShutterSpeed has strict validation - valid values are 0x01-0x11
    // The simulator now returns 0x01 as default
    assert_eq!(
        shutter,
        grafton_visca::types::ShutterSpeed::new(0x01).unwrap(),
        "Shutter should be at default (0x01)"
    );

    // Test brightness inquiry
    let brightness = camera
        .exposure()
        .brightness()
        .await
        .expect("brightness inquiry should succeed");
    assert_eq!(
        brightness,
        grafton_visca::types::BrightnessLevel::new(0x07).unwrap(),
        "Brightness should be at default"
    );

    // Test gain inquiry
    let gain = camera
        .exposure()
        .gain()
        .await
        .expect("gain inquiry should succeed");
    assert_eq!(
        gain,
        grafton_visca::types::GainLevel::new(0x00).unwrap(),
        "Gain should be at minimum"
    );

    // Test gain limit inquiry
    let gain_limit = camera
        .exposure()
        .gain_limit()
        .await
        .expect("gain limit inquiry should succeed");
    assert_eq!(
        gain_limit,
        grafton_visca::types::GainLimit::new(0x07).unwrap(),
        "Gain limit should be at default"
    );

    // Test backlight inquiry
    let backlight = camera
        .image()
        .backlight_enabled()
        .await
        .expect("backlight inquiry should succeed");
    assert!(!backlight, "Backlight should be disabled");
}

/// Test white balance and color inquiries
#[tokio::test(start_paused = true)]
async fn test_white_balance_color_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<SimulatorFullProfile, _>(simulator)
        .await
        .unwrap();

    // Test white balance mode inquiry
    let wb_mode = camera
        .white_balance()
        .mode()
        .await
        .expect("white balance mode inquiry should succeed");
    assert_eq!(
        wb_mode,
        WhiteBalanceMode::Auto,
        "White balance should be Auto"
    );

    // Test color temperature inquiry
    let color_temp = camera
        .white_balance()
        .color_temperature()
        .await
        .expect("color temperature inquiry should succeed");
    // Color temperature is returned as the VISCA value (0-55 scale)
    // Value 3 corresponds to 2800K (2500 + 3*100)
    assert_eq!(
        color_temp,
        grafton_visca::types::ColorTemp::new(3).unwrap(),
        "Color temperature should be 3 (2800K)"
    );
}

/// Test image adjustment inquiries
#[tokio::test(start_paused = true)]
async fn test_image_adjustment_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<SimulatorFullProfile, _>(simulator)
        .await
        .unwrap();

    // Test contrast inquiry
    let contrast = camera
        .image()
        .contrast()
        .await
        .expect("contrast inquiry should succeed");
    assert_eq!(
        contrast,
        grafton_visca::types::ContrastLevel::new(0x09).unwrap(),
        "Contrast should be at default"
    );

    // Test luminance inquiry
    let luminance = camera
        .image()
        .luminance()
        .await
        .expect("luminance inquiry should succeed");
    assert_eq!(
        luminance,
        grafton_visca::types::LuminanceLevel::new(0x06).unwrap(),
        "Luminance should be at default"
    );

    // Test saturation inquiry
    let saturation = camera
        .image()
        .saturation()
        .await
        .expect("saturation inquiry should succeed");
    assert_eq!(
        saturation,
        grafton_visca::types::SaturationLevel::new(0x07).unwrap(),
        "Saturation should be at default"
    );

    // Test hue inquiry
    let hue = camera
        .image()
        .hue()
        .await
        .expect("hue inquiry should succeed");
    assert_eq!(
        hue,
        grafton_visca::types::HueLevel::new(0x07).unwrap(),
        "Hue should be at default"
    );

    // Test image flip inquiry
    let flip_status = camera
        .image()
        .flip()
        .await
        .expect("image flip inquiry should succeed");
    assert!(!flip_status.vertical, "Vertical flip should be off");
    assert!(!flip_status.horizontal, "Horizontal flip should be off");
}

/// Test noise reduction inquiries
#[tokio::test(start_paused = true)]
async fn test_noise_reduction_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<SimulatorFullProfile, _>(simulator)
        .await
        .unwrap();

    // Test noise reduction 2D inquiry
    let nr_2d = camera
        .image()
        .noise_reduction_2d()
        .await
        .expect("noise reduction 2D inquiry should succeed");
    assert_eq!(
        nr_2d,
        grafton_visca::types::NoiseReduction2DLevel::new(0x01).unwrap(),
        "Noise reduction 2D should be at level 1"
    );

    // Test noise reduction 3D inquiry
    let nr_3d = camera
        .image()
        .noise_reduction_3d()
        .await
        .expect("noise reduction 3D inquiry should succeed");
    assert_eq!(
        nr_3d,
        grafton_visca::types::NoiseReduction3DLevel::new(0x01).unwrap(),
        "Noise reduction 3D should be at level 1"
    );
}

/// Test focus mode inquiries
#[tokio::test(start_paused = true)]
async fn test_focus_mode_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();

    // Test focus mode inquiry
    let focus_mode = camera
        .focus()
        .mode()
        .await
        .expect("focus mode inquiry should succeed");
    assert_eq!(focus_mode, FocusMode::Auto, "Focus mode should be Auto");

    // NOTE: AutoFocus inquiry is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
}

/// Test resolution inquiry
#[tokio::test(start_paused = true)]
async fn test_resolution_inquiry_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();

    // Test resolution inquiry
    let resolution = camera
        .image()
        .resolution()
        .await
        .expect("resolution inquiry should succeed");
    // The simulator returns 0x00 which maps to FullHD60
    assert_eq!(
        resolution,
        ResolutionMode::FullHD60, // 0x00 = 1080p60
        "Resolution inquiry should succeed"
    );
}

/// Test concurrent inquiries to verify socket manager handles them properly
#[tokio::test(start_paused = true)]
async fn test_concurrent_inquiries_integration() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();
    // Socket manager is now automatically initialized on first use

    // Launch multiple inquiries concurrently

    // Create accessor once to avoid temporary issues
    let power = camera.power();

    let (r1, r2, r3) = join!(
        power.state(),
        power.state(), // Duplicate to test queuing
        power.state(),
    );

    // All should complete successfully
    let results = vec![r1, r2, r3];
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
#[tokio::test(start_paused = true)]
async fn test_sequential_inquiries() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<SimulatorFullProfile, _>(simulator)
        .await
        .unwrap();

    // Test the failing sequence: exposure mode then exposure compensation
    let mode = camera
        .exposure()
        .mode()
        .await
        .expect("exposure mode inquiry should succeed");
    assert_eq!(mode, ExposureMode::Auto, "Exposure mode should be Auto");

    let comp = camera
        .exposure()
        .compensation()
        .await
        .expect("exposure compensation inquiry should succeed");
    assert_eq!(
        comp,
        grafton_visca::types::ExposureCompensationLevel::new(0).unwrap(),
        "Exposure compensation should be 0"
    );
}

/// Test inquiry timeout behavior
#[tokio::test(start_paused = true)]
async fn test_inquiry_timeout_behavior() {
    // Create a simulator that delays inquiry responses
    let simulator = SimulatorBuilder::default()
        .with_command_execution_time(
            grafton_visca::testing::camera_simulator::CommandType::Inquiry,
            Duration::from_millis(100),
        )
        .build();

    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator)
        .await
        .unwrap();

    // Query should complete within reasonable time
    let result = tokio::time::timeout(Duration::from_millis(200), camera.power().state()).await;
    assert!(result.is_ok(), "Inquiry should complete within timeout");
    assert!(
        result.unwrap().is_ok(),
        "Inquiry should succeed despite delay"
    );
}

/// Test mixed command and inquiry execution
#[tokio::test]
async fn test_mixed_commands_and_inquiries() {
    let simulator = ViscaCameraSimulator::new();
    let runtime = TokioRuntime::from_current().unwrap();
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(simulator.clone())
        .await
        .unwrap();

    // Execute a preset recall command
    camera
        .presets()
        .recall(1)
        .await
        .expect("preset recall should succeed");

    // Immediately query power status (should not be blocked by preset execution)
    let power_on = camera
        .power()
        .state()
        .await
        .expect("power inquiry should succeed during preset execution");
    assert!(power_on, "Power should remain on");

    // Query zoom position while preset is potentially still executing
    let zoom_pos = camera
        .zoom()
        .position()
        .await
        .expect("zoom inquiry should succeed");
    assert_eq!(
        zoom_pos,
        grafton_visca::types::ZoomPosition::MIN,
        "Zoom position should be readable"
    );
}
