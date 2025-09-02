//! Test to verify Issue #275 implementation
//!
//! This verifies that:
//! 1. MAX_SIZE values are exact (not hard-coded 32 or 16)
//! 2. Builder capacity matches declared MAX_SIZE
//! 3. Commands can be encoded successfully within their declared MAX_SIZE

#![cfg(test)]

use grafton_visca::{
    camera_id::CameraId,
    command::{encode_visca::ViscaEncode, exposure::Spotlight, tally::Tally},
};

#[test]
fn test_issue_275_tally_exact_sizing() {
    let camera_id = CameraId::new(1).unwrap();

    // Tally should have MAX_SIZE = 8 (not 32)
    assert_eq!(
        Tally::MAX_SIZE,
        8,
        "Tally MAX_SIZE should be 8, not hard-coded 32"
    );

    // Test that all variants can encode within MAX_SIZE
    let variants = [
        Tally::RedOn,
        Tally::RedOff,
        Tally::BrightLo,
        Tally::BrightHi,
        Tally::GreenOn,
        Tally::GreenOff,
        Tally::Flash,
        Tally::On,
        Tally::Off,
    ];

    for variant in variants {
        let mut buffer = vec![0u8; Tally::MAX_SIZE];
        let size = variant.encode_into(camera_id, &mut buffer).unwrap();
        assert!(
            size <= Tally::MAX_SIZE,
            "Tally::{:?} size {} exceeds MAX_SIZE {}",
            variant,
            size,
            Tally::MAX_SIZE
        );
    }
}

#[test]
fn test_issue_275_spotlight_exact_sizing() {
    let camera_id = CameraId::new(1).unwrap();

    // Spotlight should have MAX_SIZE = 6 (not 32)
    assert_eq!(
        Spotlight::MAX_SIZE,
        6,
        "Spotlight MAX_SIZE should be 6, not hard-coded 32"
    );

    // Test that all variants can encode within MAX_SIZE
    let variants = [Spotlight::On, Spotlight::Off];

    for variant in variants {
        let mut buffer = vec![0u8; Spotlight::MAX_SIZE];
        let size = variant.encode_into(camera_id, &mut buffer).unwrap();
        assert!(
            size <= Spotlight::MAX_SIZE,
            "Spotlight::{:?} size {} exceeds MAX_SIZE {}",
            variant,
            size,
            Spotlight::MAX_SIZE
        );
    }
}

#[test]
fn test_issue_275_try_into_vec_allocation() {
    // This tests the core issue: try_into_vec() should allocate exactly MAX_SIZE
    // and commands should encode successfully within that allocation

    let camera_id = CameraId::new(1).unwrap();

    let tally = Tally::RedOn;
    let tally_vec = tally.try_into_vec(camera_id).unwrap();
    assert!(
        tally_vec.len() <= Tally::MAX_SIZE,
        "try_into_vec should not exceed MAX_SIZE"
    );
    assert!(
        !tally_vec.is_empty(),
        "Command should produce non-empty output"
    );

    let spotlight = Spotlight::On;
    let spotlight_vec = spotlight.try_into_vec(camera_id).unwrap();
    assert!(
        spotlight_vec.len() <= Spotlight::MAX_SIZE,
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
    // After Issue #275: Tally should be exactly 8 bytes
    assert_ne!(
        Tally::MAX_SIZE,
        32,
        "Tally should not use hard-coded MAX_SIZE = 32"
    );
    assert_ne!(
        Tally::MAX_SIZE,
        16,
        "Tally should not use hard-coded MAX_SIZE = 16"
    );

    // Before Issue #275: Spotlight was hard-coded to MAX_SIZE = 32
    // After Issue #275: Spotlight should be exactly 6 bytes
    assert_ne!(
        Spotlight::MAX_SIZE,
        32,
        "Spotlight should not use hard-coded MAX_SIZE = 32"
    );
    assert_ne!(
        Spotlight::MAX_SIZE,
        16,
        "Spotlight should not use hard-coded MAX_SIZE = 16"
    );
}
