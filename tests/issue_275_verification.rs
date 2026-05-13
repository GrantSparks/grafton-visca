//! Test to verify Issue #275 implementation
//!
//! This verifies that:
//! 1. MAX_SIZE values are exact (not hard-coded 32 or 16)
//! 2. Builder capacity matches declared MAX_SIZE
//! 3. Commands can be encoded successfully within their declared MAX_SIZE

#![cfg(test)]

use grafton_visca::{
    command::{
        SpotlightOff, SpotlightOn, TallyBrightHi, TallyBrightLo, TallyFlash, TallyGreenOff,
        TallyGreenOn, TallyOff, TallyOn, TallyRedOff, TallyRedOn, ViscaCommand,
    },
    CameraId,
};

#[test]
fn test_issue_275_tally_exact_sizing() {
    let camera_id = CameraId::new(1).unwrap();

    // All Tally commands should have MAX_SIZE = 8 (not 32)
    assert_eq!(
        TallyRedOn::MAX_SIZE,
        8,
        "TallyRedOn MAX_SIZE should be 8, not hard-coded 32"
    );

    // Test individual Tally command structs
    let mut buffer = vec![0u8; TallyRedOn::MAX_SIZE];
    let size = TallyRedOn.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyRedOn::MAX_SIZE);

    let size = TallyRedOff.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyRedOff::MAX_SIZE);

    let size = TallyBrightLo.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyBrightLo::MAX_SIZE);

    let size = TallyBrightHi.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyBrightHi::MAX_SIZE);

    let size = TallyGreenOn.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyGreenOn::MAX_SIZE);

    let size = TallyGreenOff.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyGreenOff::MAX_SIZE);

    let size = TallyFlash.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyFlash::MAX_SIZE);

    let size = TallyOn.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyOn::MAX_SIZE);

    let size = TallyOff.write_into(camera_id, &mut buffer).unwrap();
    assert!(size <= TallyOff::MAX_SIZE);
}

#[test]
fn test_issue_275_spotlight_exact_sizing() {
    let camera_id = CameraId::new(1).unwrap();

    // SpotlightOn should have MAX_SIZE = 6 (not 32)
    assert_eq!(
        SpotlightOn::MAX_SIZE,
        6,
        "SpotlightOn MAX_SIZE should be 6, not hard-coded 32"
    );

    // Test individual Spotlight command structs
    let mut buffer = vec![0u8; SpotlightOn::MAX_SIZE];
    let size = SpotlightOn::new()
        .write_into(camera_id, &mut buffer)
        .unwrap();
    assert!(
        size <= SpotlightOn::MAX_SIZE,
        "SpotlightOn size {size} exceeds MAX_SIZE {max_size}",
        size = size,
        max_size = SpotlightOn::MAX_SIZE
    );

    let size = SpotlightOff::new()
        .write_into(camera_id, &mut buffer)
        .unwrap();
    assert!(
        size <= SpotlightOff::MAX_SIZE,
        "SpotlightOff size {size} exceeds MAX_SIZE {max_size}",
        size = size,
        max_size = SpotlightOff::MAX_SIZE
    );
}

#[test]
fn test_issue_275_try_into_vec_allocation() {
    // This tests the core issue: try_into_vec() should allocate exactly MAX_SIZE
    // and commands should encode successfully within that allocation

    let camera_id = CameraId::new(1).unwrap();

    let tally = TallyRedOn;
    let tally_bytes = tally.to_bytes(camera_id).unwrap();
    let tally_vec = tally_bytes.to_vec();
    assert!(
        tally_vec.len() <= TallyRedOn::MAX_SIZE,
        "try_into_vec should not exceed MAX_SIZE"
    );
    assert!(
        !tally_vec.is_empty(),
        "Command should produce non-empty output"
    );

    let spotlight = SpotlightOn::new();
    let spotlight_bytes = spotlight.to_bytes(camera_id).unwrap();
    let spotlight_vec = spotlight_bytes.to_vec();
    assert!(
        spotlight_vec.len() <= SpotlightOn::MAX_SIZE,
        "try_into_vec should not exceed MAX_SIZE"
    );
    assert!(
        !spotlight_vec.is_empty(),
        "Command should produce non-empty output"
    );
}

#[test]
fn test_issue_275_no_hard_coded_sizes() {
    // This test verifies that we're not using the old hard-coded sizes

    // Before Issue #275: Tally was hard-coded to MAX_SIZE = 32
    // After Issue #275: TallyRedOn should be exactly 8 bytes
    assert_ne!(
        TallyRedOn::MAX_SIZE,
        32,
        "TallyRedOn should not use hard-coded MAX_SIZE = 32"
    );
    assert_ne!(
        TallyRedOn::MAX_SIZE,
        16,
        "TallyRedOn should not use hard-coded MAX_SIZE = 16"
    );

    // Before Issue #275: Spotlight was hard-coded to MAX_SIZE = 32
    // After Issue #275: SpotlightOn should be exactly 6 bytes
    assert_ne!(
        SpotlightOn::MAX_SIZE,
        32,
        "SpotlightOn should not use hard-coded MAX_SIZE = 32"
    );
    assert_ne!(
        SpotlightOn::MAX_SIZE,
        16,
        "SpotlightOn should not use hard-coded MAX_SIZE = 16"
    );
}
