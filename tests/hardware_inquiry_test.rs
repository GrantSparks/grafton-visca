#![cfg(feature = "blocking")]
//! Hardware integration test: validates VISCA inquiry commands against a real PTZOptics G2 camera.
//!
//! This test requires a real PTZOptics G2 camera on the network.
//! It is **ignored by default** and only runs when the `VISCA_CAMERA_IP` environment
//! variable is set (e.g., `VISCA_CAMERA_IP=192.168.0.110`).
//!
//! This test validates that PTZOptics G2 cameras respond to the standard VISCA inquiry
//! command set and that every reply decodes. The Sony-format version inquiry is not
//! issued: the G2 profile does not implement `HasVersionInquiry`.
//!
//! Run with:
//! ```sh
//! VISCA_CAMERA_IP=192.168.0.110 cargo test --test hardware_inquiry_test -- --ignored --nocapture --test-threads=1
//! ```

use grafton_visca::blocking::{CameraSession, Connect};
use grafton_visca::profiles::PtzOpticsG2;

fn connect() -> Option<CameraSession<PtzOpticsG2>> {
    let ip = std::env::var("VISCA_CAMERA_IP").ok()?;
    let addr = format!("{ip}:5678");
    eprintln!("Connecting to camera at {addr}...");
    Some(Connect::open_tcp::<PtzOpticsG2>(addr).expect("Failed to connect to camera"))
}

macro_rules! inquiry_test {
    ($name:ident, $noun:ident, $method:ident) => {
        #[test]
        #[ignore]
        fn $name() {
            let Some(session) = connect() else {
                eprintln!("Skipped: VISCA_CAMERA_IP not set");
                return;
            };
            let camera = session.camera();
            let result = camera.$noun().$method();
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
inquiry_test!(test_pan_tilt_position, pan_tilt, position);
inquiry_test!(test_zoom_position, zoom, position);
inquiry_test!(test_focus_position, focus, position);

// === Focus inquiries ===
inquiry_test!(test_focus_mode, focus, mode);
inquiry_test!(test_focus_zone, focus, zone);

// === Exposure inquiries ===
inquiry_test!(test_exposure_mode, exposure, mode);
inquiry_test!(test_shutter, exposure, shutter);
inquiry_test!(test_gain, exposure, gain);
inquiry_test!(test_gain_limit, exposure, gain_limit);
inquiry_test!(test_exposure_compensation, exposure, compensation);
inquiry_test!(
    test_exposure_compensation_enabled,
    exposure,
    compensation_enabled
);
inquiry_test!(
    test_exposure_compensation_position,
    exposure,
    compensation_position
);
inquiry_test!(test_backlight_enabled, image, backlight);

// === White balance inquiries ===
inquiry_test!(test_white_balance_mode, white_balance, mode);

// === Image processing inquiries ===
inquiry_test!(test_brightness, exposure, brightness);
inquiry_test!(test_sharpness_mode, image, sharpness_mode);
inquiry_test!(test_saturation, image, saturation);
inquiry_test!(test_hue, image, hue);
inquiry_test!(test_noise_reduction_2d, image, noise_reduction_2d);
inquiry_test!(test_noise_reduction_3d, image, noise_reduction_3d);
inquiry_test!(test_image_flip, image, flip);
inquiry_test!(test_gamma, image, gamma);
inquiry_test!(test_contrast, image, contrast);
inquiry_test!(test_luminance, image, luminance);

// === System inquiries ===
inquiry_test!(test_power_state, power, state);
inquiry_test!(test_menu_status, menu, status);

// === Comprehensive test that runs all inquiries in sequence ===
//
// Categorizes results into two buckets:
// - OK: command sent, response parsed successfully
// - FAIL: unexpected failure
#[test]
#[ignore]
fn test_all_inquiries_succeed() {
    let Some(session) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };
    let camera = session.camera();

    let mut passed = 0u32;
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

    eprintln!("\n=== PTZOptics G2 Full Inquiry Validation ===\n");

    // Position
    check!("pan_tilt_position", camera.pan_tilt().position());
    check!("zoom_position", camera.zoom().position());
    check!("focus_position", camera.focus().position());

    // Focus
    check!("focus_mode", camera.focus().mode());
    // The PTZOptics G2 bench reports `FocusZone::Zone03` (#795) as well as the
    // documented top/center/bottom zones.
    check!("focus_zone", camera.focus().zone());

    // Exposure
    check!("exposure_mode", camera.exposure().mode());
    check!("shutter", camera.exposure().shutter());
    check!("gain", camera.exposure().gain());
    check!("gain_limit", camera.exposure().gain_limit());
    check!("exposure_compensation", camera.exposure().compensation());
    check!(
        "exposure_compensation_enabled",
        camera.exposure().compensation_enabled()
    );
    check!(
        "exposure_compensation_position",
        camera.exposure().compensation_position()
    );
    check!("backlight_enabled", camera.image().backlight());

    // White balance
    check!("white_balance_mode", camera.white_balance().mode());
    // The PTZOptics G2 replies to the R/B gain inquiries with the 4-nibble
    // `00 00 0p 0q` gain, and to the color temperature inquiry with one data
    // byte (`90 50 pq FF`); the decoders read those layouts.
    check!("red_gain", camera.white_balance().red_gain());
    check!("blue_gain", camera.white_balance().blue_gain());
    check!(
        "color_temperature",
        camera.white_balance().color_temperature()
    );

    // Image processing
    check!("brightness", camera.exposure().brightness());
    check!("sharpness_mode", camera.image().sharpness_mode());
    check!("saturation", camera.image().saturation());
    check!("hue", camera.image().hue());
    check!("noise_reduction_2d", camera.image().noise_reduction_2d());
    check!("noise_reduction_3d", camera.image().noise_reduction_3d());
    // PTZOptics image_flip uses CAM_PictureFlipInq (0x66) — works but has nuances:
    // PTZOptics also has CAM_FlipInq (0xA4) for combined H/V/HV flip and
    // CAM_LR_ReverseInq (0x61) for horizontal-only flip.
    check!("image_flip", camera.image().flip());
    check!("gamma", camera.image().gamma());
    check!("contrast", camera.image().contrast());
    check!("luminance", camera.image().luminance());
    // flip_mode now uses CAM_FlipInq (0xA4) — should work correctly on PTZOptics
    check!("flip_mode", camera.image().flip_mode());

    // System
    check!("power_state", camera.power().state());
    // `system().version()` is not issued: PTZOptics G2 replies to
    // `81 09 00 02 FF` with an unsourced 2-byte payload (`00 52`), so the
    // profile does not implement `HasVersionInquiry` and the typed Sony-format
    // version inquiry does not compile for it.
    check!("menu_status", camera.menu().status());

    eprintln!("\n=== Results: {passed} passed, {unexpected_failures} unexpected failures ===\n");

    assert_eq!(
        unexpected_failures, 0,
        "{unexpected_failures} inquiry commands had unexpected failures"
    );
}
