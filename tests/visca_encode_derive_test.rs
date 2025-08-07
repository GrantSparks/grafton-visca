//! Test for the ViscaEncode derive macro

use grafton_visca::{ViscaEncode, camera_id::CameraId, command::encode_visca::EncodeVisca};

#[derive(ViscaEncode, Debug, Copy, Clone)]
#[visca_encode(max_size = 6, timeout = "Quick")]
enum TestPowerCommand {
    #[visca_bytes(0x01, 0x04, 0x00, 0x02)]
    On,
    #[visca_bytes(0x01, 0x04, 0x00, 0x03)]
    Standby,
}

#[test]
fn test_visca_encode_derive() {
    let cmd = TestPowerCommand::On;
    let camera_id = CameraId::default();
    
    let mut buffer = [0u8; 16];
    let result = cmd.encode_into(camera_id, &mut buffer);
    
    assert!(result.is_ok());
    let size = result.unwrap();
    
    // Should encode to: [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    assert_eq!(size, 6);
    assert_eq!(buffer[0], 0x81); // Camera ID
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x04);
    assert_eq!(buffer[3], 0x00);
    assert_eq!(buffer[4], 0x02);
    assert_eq!(buffer[5], 0xFF); // Terminator
}

#[test]
fn test_visca_encode_standby() {
    let cmd = TestPowerCommand::Standby;
    let camera_id = CameraId::default();
    
    let mut buffer = [0u8; 16];
    let result = cmd.encode_into(camera_id, &mut buffer);
    
    assert!(result.is_ok());
    let size = result.unwrap();
    
    // Should encode to: [0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]
    assert_eq!(size, 6);
    assert_eq!(buffer[0], 0x81); // Camera ID
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x04);
    assert_eq!(buffer[3], 0x00);
    assert_eq!(buffer[4], 0x03);
    assert_eq!(buffer[5], 0xFF); // Terminator
}