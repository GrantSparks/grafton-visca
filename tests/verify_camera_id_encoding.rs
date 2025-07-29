//! Integration test to verify camera ID functionality

use grafton_visca::CameraId;

#[test]
fn test_camera_id_produces_correct_address_bytes() {
    // Test that CameraId constants produce correct VISCA address bytes
    assert_eq!(CameraId::CAMERA_1.to_address_byte(), 0x81);
    assert_eq!(CameraId::CAMERA_2.to_address_byte(), 0x82);
    assert_eq!(CameraId::CAMERA_3.to_address_byte(), 0x83);
    assert_eq!(CameraId::CAMERA_4.to_address_byte(), 0x84);
    assert_eq!(CameraId::CAMERA_5.to_address_byte(), 0x85);
    assert_eq!(CameraId::CAMERA_6.to_address_byte(), 0x86);
    assert_eq!(CameraId::CAMERA_7.to_address_byte(), 0x87);
    assert_eq!(CameraId::BROADCAST.to_address_byte(), 0x88);
}

#[test]
fn test_camera_id_validation() {
    // Test valid ID range (1-7 for individual cameras, 8 for broadcast)
    for id in 1..=7 {
        assert!(CameraId::try_from(id).is_ok(), "ID {} should be valid", id);
    }
    assert!(
        CameraId::try_from(8).is_ok(),
        "Broadcast ID 8 should be valid"
    );

    // Test invalid IDs
    assert!(CameraId::try_from(0).is_err(), "ID 0 should be invalid");
    assert!(CameraId::try_from(9).is_err(), "ID 9 should be invalid");
    assert!(CameraId::try_from(255).is_err(), "ID 255 should be invalid");
}

#[test]
fn test_camera_id_properties() {
    // Test broadcast detection
    assert!(!CameraId::CAMERA_1.is_broadcast());
    assert!(!CameraId::CAMERA_7.is_broadcast());
    assert!(CameraId::BROADCAST.is_broadcast());

    // Test ID extraction
    assert_eq!(CameraId::CAMERA_1.id(), 1);
    assert_eq!(CameraId::CAMERA_7.id(), 7);
    assert_eq!(CameraId::BROADCAST.id(), 8);
}

#[test]
fn test_ptzo_default_camera_is_1() {
    // PTZOptics cameras use 0x81 by default, which is Camera 1
    let default_camera = CameraId::default();
    assert_eq!(default_camera, CameraId::CAMERA_1);
    assert_eq!(default_camera.to_address_byte(), 0x81);
}
