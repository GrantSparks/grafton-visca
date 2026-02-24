#![cfg(not(feature = "mode-async"))]
//! Hardware integration test: validates VISCA inquiry commands against a real PTZOptics G2 camera.
//!
//! This test requires a real PTZOptics G2 camera on the network.
//! It is **ignored by default** and only runs when the `VISCA_CAMERA_IP` environment
//! variable is set (e.g., `VISCA_CAMERA_IP=192.168.0.110`).
//!
//! This test validates that PTZOptics G2 cameras respond to the standard VISCA inquiry
//! command set. Five inquiries have known parser mismatches between Sony VISCA format and
//! the PTZOptics response format — these are tracked as library parser bugs, not camera
//! limitations.
//!
//! Run with:
//! ```sh
//! VISCA_CAMERA_IP=192.168.0.110 cargo test --test hardware_inquiry_test -- --ignored --nocapture --test-threads=1
//! ```

use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::camera::Connect;
use grafton_visca::transport::BlockingTransportHandle;
use grafton_visca::BlockingClient;

fn connect() -> Option<BlockingClient<PtzOpticsG2, BlockingTransportHandle>> {
    let ip = std::env::var("VISCA_CAMERA_IP").ok()?;
    let addr = format!("{ip}:5678");
    eprintln!("Connecting to camera at {addr}...");
    Some(Connect::open_tcp_blocking::<PtzOpticsG2>(addr).expect("Failed to connect to camera"))
}

macro_rules! inquiry_test {
    ($name:ident, $method:ident) => {
        #[test]
        #[ignore]
        fn $name() {
            let Some(camera) = connect() else {
                eprintln!("Skipped: VISCA_CAMERA_IP not set");
                return;
            };
            let result = camera.$method();
            assert!(
                result.is_ok(),
                "Inquiry {} failed: {:?}",
                stringify!($method),
                result.err()
            );
            eprintln!("  {} = {:?}", stringify!($method), result.unwrap());
        }
    };
}

// === Position inquiries ===
inquiry_test!(test_pan_tilt_position, pan_tilt_position);
inquiry_test!(test_zoom_position, zoom_position);
inquiry_test!(test_focus_position, focus_position);

// === Focus inquiries ===
inquiry_test!(test_focus_mode, focus_mode);

// === Exposure inquiries ===
inquiry_test!(test_exposure_mode, exposure_mode);
inquiry_test!(test_iris, iris);
inquiry_test!(test_shutter, shutter);
inquiry_test!(test_gain, gain);
inquiry_test!(test_gain_limit, gain_limit);
inquiry_test!(test_exposure_compensation, exposure_compensation);
inquiry_test!(
    test_exposure_compensation_enabled,
    exposure_compensation_enabled
);
inquiry_test!(
    test_exposure_compensation_position,
    exposure_compensation_position
);
inquiry_test!(test_backlight_enabled, backlight_enabled);

// === White balance inquiries ===
inquiry_test!(test_white_balance_mode, white_balance_mode);

// === Image processing inquiries ===
inquiry_test!(test_brightness, brightness);
inquiry_test!(test_sharpness_mode, sharpness_mode);
inquiry_test!(test_saturation, saturation);
inquiry_test!(test_hue, hue);
inquiry_test!(test_noise_reduction_2d, noise_reduction_2d);
inquiry_test!(test_noise_reduction_3d, noise_reduction_3d);
inquiry_test!(test_image_flip, image_flip);
inquiry_test!(test_gamma, gamma);
inquiry_test!(test_contrast, contrast);
inquiry_test!(test_luminance, luminance);

// === System inquiries ===
inquiry_test!(test_power_state, power_state);
inquiry_test!(test_menu_status, menu_status);

// === Comprehensive test that runs all inquiries in sequence ===
//
// Categorizes results into three buckets:
// - OK: command sent, response parsed successfully
// - KNOWN_MISMATCH: command sent, camera responded, but our parser can't handle
//   the PTZOptics-specific response format (library bug, not camera limitation)
// - FAIL: unexpected failure
#[test]
#[ignore]
fn test_all_inquiries_succeed() {
    let Some(camera) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };

    let mut passed = 0u32;
    let mut known_mismatches = 0u32;
    let mut unexpected_failures = 0u32;

    macro_rules! check {
        ($label:expr, $expr:expr) => {
            match $expr {
                Ok(val) => {
                    eprintln!("  OK             {:<40} = {:?}", $label, val);
                    passed += 1;
                }
                Err(e) => {
                    eprintln!("  FAIL           {:<40} = {}", $label, e);
                    unexpected_failures += 1;
                }
            }
        };
    }

    macro_rules! check_known_mismatch {
        ($label:expr, $expr:expr, $reason:expr) => {
            match $expr {
                Ok(val) => {
                    eprintln!(
                        "  FIXED!         {:<40} = {:?}  (was: {})",
                        $label, val, $reason
                    );
                    passed += 1;
                }
                Err(e) => {
                    eprintln!("  KNOWN_MISMATCH {:<40} = {}  ({})", $label, e, $reason);
                    known_mismatches += 1;
                }
            }
        };
    }

    eprintln!("\n=== PTZOptics G2 Full Inquiry Validation ===\n");

    // Position
    check!("pan_tilt_position", camera.pan_tilt_position());
    check!("zoom_position", camera.zoom_position());
    check!("focus_position", camera.focus_position());

    // Focus
    check!("focus_mode", camera.focus_mode());

    // Exposure
    check!("exposure_mode", camera.exposure_mode());
    check!("iris", camera.iris());
    check!("shutter", camera.shutter());
    check!("gain", camera.gain());
    check!("gain_limit", camera.gain_limit());
    check!("exposure_compensation", camera.exposure_compensation());
    check!(
        "exposure_compensation_enabled",
        camera.exposure_compensation_enabled()
    );
    check!(
        "exposure_compensation_position",
        camera.exposure_compensation_position()
    );
    check!("backlight_enabled", camera.backlight_enabled());

    // White balance
    check!("white_balance_mode", camera.white_balance_mode());
    // PTZOptics returns 4-nibble format for RGain/BGain but our parser expects 1-byte offset format
    check_known_mismatch!(
        "red_gain",
        camera.red_gain(),
        "PTZOptics returns 4-nibble RGain, parser expects 1-byte offset"
    );
    check_known_mismatch!(
        "blue_gain",
        camera.blue_gain(),
        "PTZOptics returns 4-nibble BGain, parser expects 1-byte offset"
    );
    // PTZOptics returns 1-byte color temp value but our parser expects 4-nibble format
    check_known_mismatch!(
        "color_temperature",
        camera.color_temperature(),
        "PTZOptics returns 1-byte color temp, parser expects 4-nibble"
    );

    // Image processing
    check!("brightness", camera.brightness());
    check!("sharpness_mode", camera.sharpness_mode());
    check!("saturation", camera.saturation());
    check!("hue", camera.hue());
    check!("noise_reduction_2d", camera.noise_reduction_2d());
    check!("noise_reduction_3d", camera.noise_reduction_3d());
    // PTZOptics image_flip uses CAM_PictureFlipInq (0x66) — works but has nuances:
    // PTZOptics also has CAM_FlipInq (0xA4) for combined H/V/HV flip and
    // CAM_LR_ReverseInq (0x61) for horizontal-only flip.
    check!("image_flip", camera.image_flip());
    check!("gamma", camera.gamma());
    check!("contrast", camera.contrast());
    check!("luminance", camera.luminance());
    // flip_mode now uses CAM_FlipInq (0xA4) — should work correctly on PTZOptics
    check!("flip_mode", camera.flip_mode());

    // System
    check!("power_state", camera.power_state());
    // PTZOptics returns a shorter version response (2 bytes) than Sony format (7 bytes)
    check_known_mismatch!(
        "version",
        camera.version(),
        "PTZOptics returns 2-byte version, parser expects 7-byte Sony format"
    );
    check!("menu_status", camera.menu_status());

    eprintln!(
        "\n=== Results: {passed} passed, {known_mismatches} known parser mismatches, {unexpected_failures} unexpected failures ===\n"
    );

    assert_eq!(
        unexpected_failures, 0,
        "{unexpected_failures} inquiry commands had unexpected failures"
    );
}
