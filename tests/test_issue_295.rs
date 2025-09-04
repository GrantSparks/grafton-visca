//! Test for issue #295: TallyGreenInquiry bug fix
//! This verifies that commands like TallyGreenInquiry that have the byte-level
//! inquiry indicator (0x09 at byte 1) are correctly classified as inquiries
//! even when their type metadata (response_type()) returns None.

use grafton_visca::command::{bytes::is_inquiry_bytes, inquiry::TallyGreenInquiry};

#[test]
fn test_tally_green_inquiry_is_detected_as_inquiry() {
    // The TallyGreenInquiry command has 0x09 at byte 1, making it an inquiry
    let inquiry = TallyGreenInquiry;

    // Encode the command
    let mut buffer = [0u8; 32];
    let camera_id = grafton_visca::camera_id::CameraId::default();
    let len = grafton_visca::command::encode_visca::ViscaEncode::encode_into(
        &inquiry,
        camera_id,
        &mut buffer,
    )
    .expect("Should encode");

    // Verify that the bytes indicate it's an inquiry (has 0x09 at position 1)
    assert!(
        is_inquiry_bytes(&buffer[..len]),
        "TallyGreenInquiry should be detected as inquiry from bytes"
    );
    assert_eq!(buffer[1], 0x09, "Second byte should be 0x09 for inquiry");

    // Show that the type metadata says it's NOT an inquiry (this was the bug)
    let response_type = grafton_visca::command::encode_visca::ViscaEncode::response_type(&inquiry);
    assert!(
        response_type.is_none(),
        "TallyGreenInquiry incorrectly returns None for response_type"
    );
}

// Add a test that verifies the helper function works correctly
#[test]
fn test_is_inquiry_bytes_helper() {
    // Test inquiry bytes (second byte is 0x09)
    let inquiry_bytes = &[0x81, 0x09, 0x04, 0x00, 0xFF];
    assert!(is_inquiry_bytes(inquiry_bytes), "Should detect inquiry");

    // Test command bytes (second byte is NOT 0x09)
    let command_bytes = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    assert!(!is_inquiry_bytes(command_bytes), "Should detect command");

    // Test edge cases
    assert!(!is_inquiry_bytes(&[]), "Empty slice should return false");
    assert!(
        !is_inquiry_bytes(&[0x81]),
        "Single byte should return false"
    );
    assert!(
        !is_inquiry_bytes(&[0x81, 0x01]),
        "Non-inquiry two bytes should return false"
    );
    assert!(
        is_inquiry_bytes(&[0x81, 0x09]),
        "Inquiry two bytes should return true"
    );
}
