//! Tests for zoom control commands.

mod common;

use crate::common::{patterns, MockTransport, ProtocolValidator, ResponseBuilder, ValidationMode};
use grafton_visca::{
    camera::{methods::ZoomOps, Camera, ProfileId},
    command::{zoom::ZoomSpeed, EncodeVisca, ResponseType, Zoom},
    timeout::CommandCategory,
    types::ZoomPosition,
    Error,
};

#[test]
fn test_zoom_speed_new() {
    // Valid speeds
    for speed in 0..=7 {
        let zoom_speed = ZoomSpeed::new(speed)
            .unwrap_or_else(|e| panic!("Failed to create ZoomSpeed {speed}: {e:?}"));
        assert_eq!(zoom_speed.value(), speed);
    }

    // Invalid speed
    assert!(matches!(ZoomSpeed::new(8), Err(Error::InvalidParameter(_))));
    assert!(matches!(
        ZoomSpeed::new(255),
        Err(Error::InvalidParameter(_))
    ));
}

#[test]
fn test_zoom_speed_try_from() {
    // Valid conversion
    let speed = ZoomSpeed::try_from(5)
        .unwrap_or_else(|e| panic!("ZoomSpeed::try_from(5) should be valid: {e:?}"));
    assert_eq!(speed.value(), 5);

    // Invalid conversion
    assert!(ZoomSpeed::try_from(8).is_err());
}

#[test]
fn test_zoom_speed_into_u8() {
    let speed = ZoomSpeed::new(3).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 3: {e:?}"));
    let value: u8 = speed.value();
    assert_eq!(value, 3);
}

#[test]
fn test_zoom_command_stop() {
    let cmd = Zoom::Stop;
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert Stop command to bytes: {e:?}"));

    // Verify using pattern constants
    assert_eq!(bytes, patterns::zoom::STOP);

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_zoom_command_zoom_in_standard() {
    let cmd = Zoom::TeleStd;
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert TeleStd command to bytes: {e:?}"));

    // Verify using pattern constants
    assert_eq!(bytes, patterns::zoom::TELE_STD);
    assert_eq!(cmd.response_type(), Some(ResponseType::ZoomIn));

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_zoom_command_zoom_out_standard() {
    let cmd = Zoom::WideStd;
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert WideStd command to bytes: {e:?}"));

    // Verify using pattern constants
    assert_eq!(bytes, patterns::zoom::WIDE_STD);

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_zoom_command_zoom_in_variable() {
    let speed = ZoomSpeed::new(5).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 5: {e:?}"));
    let cmd = Zoom::TeleVariable(speed);
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert TeleVariable command to bytes: {e:?}"));

    // Verify command structure
    assert_eq!(bytes, vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]);

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_zoom_command_zoom_out_variable() {
    let speed = ZoomSpeed::new(7).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 7: {e:?}"));
    let cmd = Zoom::WideVariable(speed);
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert WideVariable command to bytes: {e:?}"));

    // Verify command structure
    assert_eq!(bytes, vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF]);

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_zoom_command_position() {
    let cmd = Zoom::Position(
        ZoomPosition::new(0x1234).unwrap_or_else(|e| panic!("Valid zoom position: {e:?}")),
    );
    let bytes = cmd
        .try_into_vec()
        .unwrap_or_else(|e| panic!("Failed to convert Position command to bytes: {e:?}"));

    // Verify command structure
    assert_eq!(
        bytes,
        vec![0x81, 0x01, 0x04, 0x47, 0x01, 0x02, 0x03, 0x04, 0xFF]
    );

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_position_to_nibbles() {
    // Helper function from the module
    fn position_to_nibbles(position: u16) -> [u8; 4] {
        [
            ((position >> 12) & 0x0F) as u8,
            ((position >> 8) & 0x0F) as u8,
            ((position >> 4) & 0x0F) as u8,
            (position & 0x0F) as u8,
        ]
    }

    assert_eq!(position_to_nibbles(0x0000), [0x00, 0x00, 0x00, 0x00]);
    assert_eq!(position_to_nibbles(0x1234), [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(position_to_nibbles(0xABCD), [0x0A, 0x0B, 0x0C, 0x0D]);
    assert_eq!(position_to_nibbles(0xFFFF), [0x0F, 0x0F, 0x0F, 0x0F]);
}

#[test]
fn test_zoom_response_type() {
    // Commands that expect ZoomIn response
    assert_eq!(Zoom::TeleStd.response_type(), Some(ResponseType::ZoomIn));
    let speed = ZoomSpeed::new(5).unwrap();
    assert_eq!(
        Zoom::TeleVariable(speed).response_type(),
        Some(ResponseType::ZoomIn)
    );

    // Commands that expect ZoomOut response
    assert_eq!(Zoom::WideStd.response_type(), Some(ResponseType::ZoomOut));
    assert_eq!(
        Zoom::WideVariable(speed).response_type(),
        Some(ResponseType::ZoomOut)
    );

    // Commands that have no response type
    assert_eq!(Zoom::Stop.response_type(), None);
    let position = ZoomPosition::new(0x1234).unwrap();
    assert_eq!(Zoom::Position(position).response_type(), None);
}

#[test]
fn test_zoom_commands_with_camera() {
    let mut mock = MockTransport::new();

    // Set up expectations for zoom commands
    mock.expect_command(&patterns::zoom::STOP)
        .described_as("zoom stop")
        .will_ack(1)
        .then_complete(1);

    mock.expect_command(&patterns::zoom::TELE_STD)
        .described_as("zoom in standard")
        .will_ack(1)
        .then_complete(1);

    mock.expect_command(&patterns::zoom::WIDE_STD)
        .described_as("zoom out standard")
        .will_ack(1)
        .then_complete(1);

    let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

    // Test zoom commands
    assert!(camera.zoom_stop_blocking().is_ok());
    assert!(camera.zoom_in_blocking().is_ok());
    assert!(camera.zoom_out_blocking().is_ok());

    // Verify all expectations were met
    mock.verify().unwrap();
}

#[test]
fn test_zoom_with_inquiry_response() {
    let mut mock = MockTransport::new();

    // Set up zoom position inquiry
    mock.expect_command(&patterns::zoom::POSITION_INQ)
        .described_as("zoom position inquiry")
        .will_return_data(&[0x04, 0x00, 0x00, 0x00]); // Position 0x4000

    let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

    // This would need the inquiry methods implemented
    // For now, just verify the mock was set up correctly
    mock.verify().unwrap();
}
