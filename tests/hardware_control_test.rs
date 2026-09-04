#![cfg(feature = "blocking")]
//! Hardware integration test: validates VISCA SET commands against a real PTZOptics G2 camera.
//!
//! **WARNING: These tests change camera settings.** Each test follows a safe pattern:
//! 1. Read current value (save original)
//! 2. Set to a different value
//! 3. Read back to confirm the set worked
//! 4. Restore the original value
//! 5. Read back to confirm restoration
//!
//! If a test panics between steps 2 and 4, the camera setting may be left in a modified state.
//! All modified settings are non-destructive image processing parameters.
//!
//! This test requires a real PTZOptics G2 camera on the network.
//! It is **ignored by default** and only runs when the `VISCA_CAMERA_IP` environment
//! variable is set.
//!
//! Run with:
//! ```sh
//! VISCA_CAMERA_IP=192.168.0.110 cargo test --test hardware_control_test -- --ignored --nocapture --test-threads=1
//! ```

use std::thread;
use std::time::Duration;

use grafton_visca::blocking::{CameraSession, Connect};
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::types::{ContrastLevel, GammaLevel, LuminanceLevel};

/// Delay between set and readback to allow camera processing.
const SETTLE_DELAY: Duration = Duration::from_millis(500);

fn connect() -> Option<CameraSession<PtzOpticsG2>> {
    let ip = std::env::var("VISCA_CAMERA_IP").ok()?;
    let addr = format!("{ip}:5678");
    eprintln!("Connecting to camera at {addr}...");
    Some(Connect::open_tcp::<PtzOpticsG2>(addr).expect("Failed to connect to camera"))
}

#[test]
#[ignore]
fn test_gamma_round_trip() {
    let Some(session) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };
    let camera = session.camera();

    // Step 1: Read current gamma
    let original = camera
        .image()
        .gamma()
        .expect("Failed to read initial gamma value");
    eprintln!("  Original gamma: {original:?}");

    // Step 2: Choose a different value
    let test_value = if original.value() == 0 {
        GammaLevel::new(1).unwrap()
    } else {
        GammaLevel::new(0).unwrap()
    };
    eprintln!("  Setting gamma to: {test_value:?}");

    // Step 3: Set it
    camera
        .image()
        .set_gamma(test_value)
        .expect("Failed to set gamma value");
    thread::sleep(SETTLE_DELAY);

    // Step 4: Read back and verify
    let readback = camera
        .image()
        .gamma()
        .expect("Failed to read back gamma after set");
    eprintln!("  Readback gamma: {readback:?}");

    // Step 5: Restore original
    camera
        .image()
        .set_gamma(original)
        .expect("Failed to restore original gamma");
    thread::sleep(SETTLE_DELAY);

    // Step 6: Verify restoration
    let restored = camera
        .image()
        .gamma()
        .expect("Failed to read restored gamma value");
    eprintln!("  Restored gamma: {restored:?}");

    assert_eq!(
        readback, test_value,
        "Gamma readback does not match set value"
    );
    assert_eq!(
        restored, original,
        "Gamma restoration does not match original"
    );
}

#[test]
#[ignore]
fn test_contrast_round_trip() {
    let Some(session) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };
    let camera = session.camera();

    let original = camera
        .image()
        .contrast()
        .expect("Failed to read initial contrast value");
    eprintln!("  Original contrast: {original:?}");

    let test_value = if original.value() == 0 {
        ContrastLevel::new(1).unwrap()
    } else {
        ContrastLevel::new(0).unwrap()
    };
    eprintln!("  Setting contrast to: {test_value:?}");

    camera
        .image()
        .set_contrast(test_value)
        .expect("Failed to set contrast value");
    thread::sleep(SETTLE_DELAY);

    let readback = camera
        .image()
        .contrast()
        .expect("Failed to read back contrast after set");
    eprintln!("  Readback contrast: {readback:?}");

    camera
        .image()
        .set_contrast(original)
        .expect("Failed to restore original contrast");
    thread::sleep(SETTLE_DELAY);

    let restored = camera
        .image()
        .contrast()
        .expect("Failed to read restored contrast value");
    eprintln!("  Restored contrast: {restored:?}");

    assert_eq!(
        readback, test_value,
        "Contrast readback does not match set value"
    );
    assert_eq!(
        restored, original,
        "Contrast restoration does not match original"
    );
}

#[test]
#[ignore]
fn test_luminance_round_trip() {
    let Some(session) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };
    let camera = session.camera();

    let original = camera
        .image()
        .luminance()
        .expect("Failed to read initial luminance value");
    eprintln!("  Original luminance: {original:?}");

    let test_value = if original.value() == 0 {
        LuminanceLevel::new(1).unwrap()
    } else {
        LuminanceLevel::new(0).unwrap()
    };
    eprintln!("  Setting luminance to: {test_value:?}");

    camera
        .image()
        .set_luminance(test_value)
        .expect("Failed to set luminance value");
    thread::sleep(SETTLE_DELAY);

    let readback = camera
        .image()
        .luminance()
        .expect("Failed to read back luminance after set");
    eprintln!("  Readback luminance: {readback:?}");

    camera
        .image()
        .set_luminance(original)
        .expect("Failed to restore original luminance");
    thread::sleep(SETTLE_DELAY);

    let restored = camera
        .image()
        .luminance()
        .expect("Failed to read restored luminance value");
    eprintln!("  Restored luminance: {restored:?}");

    assert_eq!(
        readback, test_value,
        "Luminance readback does not match set value"
    );
    assert_eq!(
        restored, original,
        "Luminance restoration does not match original"
    );
}

/// Comprehensive round-trip test for all image processing controls.
///
/// Runs gamma, contrast, and luminance round-trips in sequence, reporting
/// results in OK/FAIL categories.
#[test]
#[ignore]
fn test_all_image_processing_round_trips() {
    let Some(session) = connect() else {
        eprintln!("Skipped: VISCA_CAMERA_IP not set");
        return;
    };
    let camera = session.camera();

    let mut passed = 0u32;
    let mut failed = 0u32;

    macro_rules! round_trip {
        ($label:expr, $inquiry:ident, $setter:ident, $type:ident, $alt:expr) => {{
            eprint!("  {:<20} ", $label);
            match camera.image().$inquiry() {
                Ok(original) => {
                    let test_val = if original.value() == $alt {
                        <$type>::new(0).unwrap()
                    } else {
                        <$type>::new($alt).unwrap()
                    };
                    match camera.image().$setter(test_val) {
                        Ok(()) => {
                            thread::sleep(SETTLE_DELAY);
                            match camera.image().$inquiry() {
                                Ok(readback) => {
                                    // Restore regardless of readback result
                                    let _ = camera.image().$setter(original);
                                    thread::sleep(SETTLE_DELAY);
                                    if readback == test_val {
                                        eprintln!(
                                            "OK  (original={original:?}, set={test_val:?}, readback={readback:?})"
                                        );
                                        passed += 1;
                                    } else {
                                        eprintln!(
                                            "FAIL readback mismatch (set={test_val:?}, got={readback:?})"
                                        );
                                        failed += 1;
                                    }
                                }
                                Err(e) => {
                                    let _ = camera.image().$setter(original);
                                    eprintln!("FAIL readback inquiry failed: {e}");
                                    failed += 1;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("FAIL set command failed: {e}");
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("FAIL initial inquiry failed: {e}");
                    failed += 1;
                }
            }
        }};
    }

    eprintln!("\n=== PTZOptics G2 Image Processing Control Validation ===\n");

    round_trip!("gamma", gamma, set_gamma, GammaLevel, 1);
    round_trip!("contrast", contrast, set_contrast, ContrastLevel, 1);
    round_trip!("luminance", luminance, set_luminance, LuminanceLevel, 1);

    eprintln!("\n=== Results: {passed} passed, {failed} failed ===\n");

    assert_eq!(
        failed, 0,
        "{failed} image processing control commands had unexpected failures"
    );
}
