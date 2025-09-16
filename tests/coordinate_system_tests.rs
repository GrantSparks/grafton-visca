//! Tests for profile-aware coordinate system handling.

// External crates
use grafton_visca::{
    camera::profiles::{PtzOpticsG2, SonyBRC300},
    capabilities::CoordinateSystem,
    command::{
        pan_tilt::{PanTilt as PanTiltCommand, PanTiltLimitCorner},
        response::{types::Response, ResponseKind},
        ViscaCommand,
    },
    types::{PanSpeed, TiltSpeed},
    CameraId,
};

#[test]
fn test_signed_centered_encoding() {
    // Test encoding absolute position for SignedCentered camera
    let cmd = PanTiltCommand::AbsolutePositionRaw {
        pan_u16: 0x1000_u16,  // Positive value
        tilt_u16: 0xF000_u16, // Negative value (two's complement)
        pan_speed: PanSpeed::new(10).unwrap(),
        tilt_speed: TiltSpeed::new(10).unwrap(),
    };

    let mut buffer = [0u8; 32];
    let camera_id = CameraId::new(1).unwrap();
    let len = cmd.write_into(camera_id, &mut buffer).unwrap();

    // Expected: 81 01 06 02 0A 0A 01 00 00 00 0F 00 00 00 FF
    assert_eq!(buffer[0], 0x81);
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x06);
    assert_eq!(buffer[3], 0x02);
    assert_eq!(buffer[4], 0x0A); // pan speed
    assert_eq!(buffer[5], 0x0A); // tilt speed
                                 // Pan position 0x1000 in VISCA nibbles
    assert_eq!(buffer[6], 0x01);
    assert_eq!(buffer[7], 0x00);
    assert_eq!(buffer[8], 0x00);
    assert_eq!(buffer[9], 0x00);
    // Tilt position 0xF000 in VISCA nibbles
    assert_eq!(buffer[10], 0x0F);
    assert_eq!(buffer[11], 0x00);
    assert_eq!(buffer[12], 0x00);
    assert_eq!(buffer[13], 0x00);
    assert_eq!(buffer[14], 0xFF);
    assert_eq!(len, 15);
}

#[test]
fn test_unsigned_centered_encoding() {
    // For UnsignedCentered, logical (0, 0) should become (0x8000, 0x8000)
    let (pan_u16, tilt_u16) = CoordinateSystem::UnsignedCentered.to_camera_coords(0, 0);
    assert_eq!(pan_u16, 0x8000);
    assert_eq!(tilt_u16, 0x8000);

    let cmd = PanTiltCommand::AbsolutePositionRaw {
        pan_u16,
        tilt_u16,
        pan_speed: PanSpeed::new(10).unwrap(),
        tilt_speed: TiltSpeed::new(10).unwrap(),
    };

    let mut buffer = [0u8; 32];
    let camera_id = CameraId::new(1).unwrap();
    let len = cmd.write_into(camera_id, &mut buffer).unwrap();

    // Expected: 81 01 06 02 0A 0A 08 00 00 00 08 00 00 00 FF
    assert_eq!(buffer[0], 0x81);
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x06);
    assert_eq!(buffer[3], 0x02);
    assert_eq!(buffer[4], 0x0A); // pan speed
    assert_eq!(buffer[5], 0x0A); // tilt speed
                                 // Pan position 0x8000 in VISCA nibbles
    assert_eq!(buffer[6], 0x08);
    assert_eq!(buffer[7], 0x00);
    assert_eq!(buffer[8], 0x00);
    assert_eq!(buffer[9], 0x00);
    // Tilt position 0x8000 in VISCA nibbles
    assert_eq!(buffer[10], 0x08);
    assert_eq!(buffer[11], 0x00);
    assert_eq!(buffer[12], 0x00);
    assert_eq!(buffer[13], 0x00);
    assert_eq!(buffer[14], 0xFF);
    assert_eq!(len, 15);
}

#[test]
fn test_signed_centered_decoding() {
    // Simulate a PanTiltPosition response for SignedCentered camera (PtzOpticsG2)
    // Response: 90 50 01 00 00 00 0F 00 00 00 FF (pan=0x1000, tilt=0xF000)
    let response_bytes = [
        0x90, 0x50, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0xFF,
    ];

    let result = Response::parse_with_profile::<PtzOpticsG2>(
        &response_bytes,
        &ResponseKind::PanTiltPosition,
    );

    assert!(result.is_ok());
    if let Ok(Response::Inquiry(inquiry)) = result {
        if let grafton_visca::command::InquiryResponse::PanTiltPosition { pan, tilt } = inquiry {
            // For SignedCentered, 0x1000 should be 4096 and 0xF000 should be -4096
            assert_eq!(pan, 0x1000_u16 as i16);
            assert_eq!(tilt, 0xF000_u16 as i16);
        } else {
            panic!("Expected PanTiltPosition response");
        }
    } else {
        panic!("Failed to parse response");
    }
}

#[test]
fn test_unsigned_centered_decoding() {
    // Simulate a PanTiltPosition response for UnsignedCentered camera (SonyBRC300)
    // Response: 90 50 08 00 00 00 08 00 00 00 FF (pan=0x8000, tilt=0x8000)
    let response_bytes = [
        0x90, 0x50, 0x08, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xFF,
    ];

    let result =
        Response::parse_with_profile::<SonyBRC300>(&response_bytes, &ResponseKind::PanTiltPosition);

    assert!(result.is_ok());
    if let Ok(Response::Inquiry(inquiry)) = result {
        if let grafton_visca::command::InquiryResponse::PanTiltPosition { pan, tilt } = inquiry {
            // For UnsignedCentered, 0x8000 should be converted to logical 0
            assert_eq!(pan, 0);
            assert_eq!(tilt, 0);
        } else {
            panic!("Expected PanTiltPosition response");
        }
    } else {
        panic!("Failed to parse response");
    }
}

#[test]
fn test_coordinate_roundtrip_signed() {
    let coord_system = CoordinateSystem::SignedCentered;

    // Test various logical positions
    let test_cases = [
        (0, 0),
        (1000, -500),
        (-1000, 500),
        (2448, 1296),
        (-2448, -432),
    ];

    for (logical_pan, logical_tilt) in test_cases {
        let (camera_pan, camera_tilt) = coord_system.to_camera_coords(logical_pan, logical_tilt);
        let (result_pan, result_tilt) =
            coord_system.convert_from_camera_coords(camera_pan, camera_tilt);

        assert_eq!(
            result_pan, logical_pan,
            "Pan roundtrip failed for {}",
            logical_pan
        );
        assert_eq!(
            result_tilt, logical_tilt,
            "Tilt roundtrip failed for {}",
            logical_tilt
        );
    }
}

#[test]
fn test_coordinate_roundtrip_unsigned() {
    let coord_system = CoordinateSystem::UnsignedCentered;

    // Test various logical positions
    let test_cases = [
        (0, 0),
        (1000, -300),
        (-1000, 300),
        (1170, 390),
        (-1170, -390),
    ];

    for (logical_pan, logical_tilt) in test_cases {
        let (camera_pan, camera_tilt) = coord_system.to_camera_coords(logical_pan, logical_tilt);
        let (result_pan, result_tilt) =
            coord_system.convert_from_camera_coords(camera_pan, camera_tilt);

        assert_eq!(
            result_pan, logical_pan,
            "Pan roundtrip failed for {}",
            logical_pan
        );
        assert_eq!(
            result_tilt, logical_tilt,
            "Tilt roundtrip failed for {}",
            logical_tilt
        );
    }
}

#[test]
fn test_limit_set_encoding_with_coordinate_system() {
    // Test that LimitSetRaw encodes correctly for UnsignedCentered
    let (pan_u16, tilt_u16) = CoordinateSystem::UnsignedCentered.to_camera_coords(500, -200);

    let cmd = PanTiltCommand::LimitSetRaw {
        corner: PanTiltLimitCorner::UpRight,
        pan_u16,
        tilt_u16,
    };

    let mut buffer = [0u8; 32];
    let camera_id = CameraId::new(1).unwrap();
    let len = cmd.write_into(camera_id, &mut buffer).unwrap();

    // Verify the command structure
    assert_eq!(buffer[0], 0x81);
    assert_eq!(buffer[1], 0x01);
    assert_eq!(buffer[2], 0x06);
    assert_eq!(buffer[3], 0x07);
    assert_eq!(buffer[4], 0x00);
    assert_eq!(buffer[5], 0x03); // UpRight corner

    // The pan/tilt values should be the converted camera coordinates
    // pan_u16 = 0x8000 + 500 = 0x81F4
    // tilt_u16 = 0x8000 - 200 = 0x7F38
    assert_eq!(pan_u16, 0x81F4);
    assert_eq!(tilt_u16, 0x7F38);

    // Check VISCA nibbles for pan (0x81F4)
    assert_eq!(buffer[6], 0x08);
    assert_eq!(buffer[7], 0x01);
    assert_eq!(buffer[8], 0x0F);
    assert_eq!(buffer[9], 0x04);

    // Check VISCA nibbles for tilt (0x7F38)
    assert_eq!(buffer[10], 0x07);
    assert_eq!(buffer[11], 0x0F);
    assert_eq!(buffer[12], 0x03);
    assert_eq!(buffer[13], 0x08);

    assert_eq!(buffer[14], 0xFF);
    assert_eq!(len, 15);
}
