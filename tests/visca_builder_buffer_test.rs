//! Tests for visca_builder! macro buffer size handling
//!
//! This test module verifies that commands using the visca_builder! macro
//! properly handle buffers that are exactly the right size for the actual
//! encoded command, even when smaller than MAX_SIZE.

use grafton_visca::{
    camera_id::CameraId,
    command::{
        encode_visca::ViscaEncode,
        image::{Contrast, Luminance},
    },
    types::{ContrastLevel, LuminanceLevel},
    Error,
};

#[test]
fn test_luminance_right_sized_buffer() {
    // Luminance command has MAX_SIZE of 9
    // Actual encoded size for level 7: [0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF] = 9 bytes
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Test with exact-sized buffer (9 bytes)
    let mut buffer = [0u8; 9];
    let result = cmd.encode_into(CameraId::CAMERA_1, &mut buffer);
    assert!(result.is_ok(), "Should accept exact-sized buffer");
    let len = result.unwrap();
    assert_eq!(len, 9);
    assert_eq!(buffer[len - 1], 0xFF, "Should have terminator");
    assert_eq!(
        &buffer[..len],
        &[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF]
    );
}

#[test]
fn test_contrast_right_sized_buffer() {
    // Contrast command has MAX_SIZE of 9
    // Actual encoded size for level 5: [0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, 0x05, 0xFF] = 9 bytes
    let cmd = Contrast::new(ContrastLevel::new(5).unwrap());

    // Test with exact-sized buffer (9 bytes)
    let mut buffer = [0u8; 9];
    let result = cmd.encode_into(CameraId::CAMERA_1, &mut buffer);
    assert!(result.is_ok(), "Should accept exact-sized buffer");
    let len = result.unwrap();
    assert_eq!(len, 9);
    assert_eq!(buffer[len - 1], 0xFF, "Should have terminator");
    assert_eq!(
        &buffer[..len],
        &[0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, 0x05, 0xFF]
    );
}

#[test]
fn test_luminance_undersized_buffer() {
    // Test with buffer that's too small for actual encoded length
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Buffer too small (8 bytes when we need 9)
    let mut buffer = [0u8; 8];
    let result = cmd.encode_into(CameraId::CAMERA_1, &mut buffer);

    assert!(result.is_err(), "Should reject undersized buffer");
    match result {
        Err(Error::BufferTooSmall { required, actual }) => {
            assert_eq!(required, 9, "Should report actual required size");
            assert_eq!(actual, 8, "Should report provided buffer size");
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

#[test]
fn test_contrast_undersized_buffer() {
    // Test with buffer that's too small for actual encoded length
    let cmd = Contrast::new(ContrastLevel::new(5).unwrap());

    // Buffer too small (7 bytes when we need 9)
    let mut buffer = [0u8; 7];
    let result = cmd.encode_into(CameraId::CAMERA_1, &mut buffer);

    assert!(result.is_err(), "Should reject undersized buffer");
    match result {
        Err(Error::BufferTooSmall { required, actual }) => {
            assert_eq!(required, 9, "Should report actual required size");
            assert_eq!(actual, 7, "Should report provided buffer size");
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

#[test]
fn test_luminance_encode_array_exact_size() {
    // Test encode_array with exact size
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Use encode_array with exact size (9 bytes)
    let result = cmd.encode_array::<9>(CameraId::CAMERA_1);
    assert!(result.is_ok(), "encode_array should work with exact size");

    let buffer = result.unwrap();
    // The actual encoded size is 9 bytes
    assert_eq!(
        &buffer[..9],
        &[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF]
    );
}

#[test]
fn test_contrast_encode_array_exact_size() {
    // Test encode_array with exact size
    let cmd = Contrast::new(ContrastLevel::new(5).unwrap());

    // Use encode_array with exact size (9 bytes)
    let result = cmd.encode_array::<9>(CameraId::CAMERA_1);
    assert!(result.is_ok(), "encode_array should work with exact size");

    let buffer = result.unwrap();
    // The actual encoded size is 9 bytes
    assert_eq!(
        &buffer[..9],
        &[0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, 0x05, 0xFF]
    );
}

#[test]
fn test_encode_array_undersized() {
    // Test encode_array with size smaller than actual encoded length
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Try to use encode_array with insufficient size (8 bytes when we need 9)
    let result = cmd.encode_array::<8>(CameraId::CAMERA_1);

    assert!(
        result.is_err(),
        "encode_array should fail with undersized array"
    );
    match result {
        Err(Error::BufferTooSmall { required, actual }) => {
            assert_eq!(required, 9, "Should report actual required size");
            assert_eq!(actual, 8, "Should report array size");
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

#[test]
fn test_encode_array_larger_than_needed() {
    // Test encode_array with size larger than needed (should succeed)
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Use encode_array with more space than needed (16 bytes)
    let result = cmd.encode_array::<16>(CameraId::CAMERA_1);
    assert!(result.is_ok(), "encode_array should work with extra space");

    let buffer = result.unwrap();
    // Should only use necessary bytes (9 bytes)
    assert_eq!(
        &buffer[..9],
        &[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF]
    );
}

#[test]
fn test_different_camera_ids() {
    // Test that different camera IDs work properly
    let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());

    // Camera 2 (0x82)
    let mut buffer = [0u8; 9];
    let result = cmd.encode_into(CameraId::CAMERA_2, &mut buffer);
    assert!(result.is_ok());
    let _len = result.unwrap();
    assert_eq!(buffer[0], 0x82, "Should have correct camera ID");

    // Camera 7 (0x87)
    let result = cmd.encode_into(CameraId::CAMERA_7, &mut buffer);
    assert!(result.is_ok());
    let _len = result.unwrap();
    assert_eq!(buffer[0], 0x87, "Should have correct camera ID");
}

#[test]
fn test_max_size_constant_unchanged() {
    // Verify that MAX_SIZE constant is still properly defined
    // This ensures we didn't break the constant which is used by try_into_vec
    assert_eq!(Luminance::MAX_SIZE, 9);
    assert_eq!(Contrast::MAX_SIZE, 9);
}
