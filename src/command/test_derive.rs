//! Internal test module for ViscaEncode derive macro
//!
//! This module demonstrates how the ViscaEncode derive macro can simplify
//! command implementations compared to manual macros.

#![allow(clippy::expect_used)]

use crate::camera_id::CameraId;
use crate::command::encode_visca::EncodeVisca;
use crate::ViscaEncode;

#[derive(ViscaEncode, Debug, Copy, Clone)]
#[visca_encode(max_size = 6, timeout = "Quick")]
enum DerivedPowerCommand {
    #[visca_bytes(0x01, 0x04, 0x00, 0x02)]
    On,
    #[visca_bytes(0x01, 0x04, 0x00, 0x03)]
    Standby,
}

#[test]
fn test_derived_power_on() {
    let cmd = DerivedPowerCommand::On;
    let camera_id = CameraId::default();

    let mut buffer = [0u8; 16];
    let result = cmd.encode_into(camera_id, &mut buffer);

    assert!(result.is_ok(), "Failed to encode: {:?}", result);
    let size = result.expect("Already checked that result is Ok");

    println!("Encoded size: {}", size);
    println!("Buffer: {:02X?}", &buffer[..size]);

    // Should encode to: [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    assert_eq!(size, 6, "Expected 6 bytes, got {}", size);
    assert_eq!(buffer[0], 0x81); // Camera ID
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x04);
    assert_eq!(buffer[3], 0x00);
    assert_eq!(buffer[4], 0x02);
    assert_eq!(buffer[5], 0xFF); // Terminator
}

#[test]
fn test_derived_power_standby() {
    let cmd = DerivedPowerCommand::Standby;
    let camera_id = CameraId::default();

    let mut buffer = [0u8; 16];
    let result = cmd.encode_into(camera_id, &mut buffer);

    assert!(result.is_ok());
    let size = result.expect("Already checked that result is Ok");

    // Should encode to: [0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]
    assert_eq!(size, 6);
    assert_eq!(buffer[0], 0x81); // Camera ID
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x04);
    assert_eq!(buffer[3], 0x00);
    assert_eq!(buffer[4], 0x03);
    assert_eq!(buffer[5], 0xFF); // Terminator
}
