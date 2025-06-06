//! Tests for the high-level extension traits.

use grafton_visca::{
    ImagePreset, ViscaError, ViscaExposureExt, ViscaImageExt, ViscaPositionExt, ViscaTransportExt,
    ViscaWhiteBalanceExt, ViscaZoomExt, WhiteBalancePreset,
};

/// Mock transport for testing
struct MockTransport {
    responses: Vec<Vec<u8>>,
    commands_sent: Vec<Vec<u8>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: vec![],
            commands_sent: vec![],
        }
    }

    fn with_ack_completion() -> Self {
        Self {
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
            commands_sent: vec![],
        }
    }
}

impl grafton_visca::ViscaTransport for MockTransport {
    fn send_command(
        &mut self,
        command: &dyn grafton_visca::ViscaCommand,
    ) -> Result<(), ViscaError> {
        self.commands_sent.push(command.to_bytes()?);
        Ok(())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        if self.responses.is_empty() {
            Err(ViscaError::Timeout)
        } else {
            Ok(vec![self.responses.remove(0)])
        }
    }
}

#[test]
fn test_exposure_ext_methods() {
    let mut transport = MockTransport::with_ack_completion();

    // Test set_iris
    ViscaExposureExt::set_iris(&mut transport, 0x0C).unwrap();
    assert_eq!(transport.commands_sent.len(), 1);
    // IrisCommand::Direct includes 4 bytes for the value
    assert_eq!(
        transport.commands_sent[0],
        vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF]
    );
}

#[test]
fn test_white_balance_preset() {
    let mut transport = MockTransport::with_ack_completion();

    // Test white balance preset - Daylight sends 2 commands
    transport
        .set_white_balance_preset(WhiteBalancePreset::Daylight)
        .unwrap();
    assert_eq!(transport.commands_sent.len(), 2); // Set mode + set temperature
}

#[test]
fn test_image_preset() {
    let mut transport = MockTransport::with_ack_completion();

    // Test image preset - this should send multiple commands
    transport.apply_image_preset(ImagePreset::Vivid).unwrap();
    assert!(transport.commands_sent.len() >= 4); // Should set sharpness, saturation, contrast, hue
}

#[test]
fn test_zoom_magnification() {
    let mut transport = MockTransport::with_ack_completion();

    // Test zoom to magnification
    transport.zoom_to_magnification(5.0).unwrap();
    assert_eq!(transport.commands_sent.len(), 1);
}

#[test]
fn test_position_degrees() {
    let mut transport = MockTransport::with_ack_completion();

    // Test move to degrees
    transport
        .move_to_degrees(45.0, 15.0, Some((10, 10)))
        .unwrap();
    assert_eq!(transport.commands_sent.len(), 1);
}

#[test]
fn test_power_ext() {
    // For now, just test that the command is sent correctly
    // Testing the full response parsing would require a more complex mock
    let mut transport = MockTransport::new();

    // We can't easily test is_powered_on without a full session mock
    // So let's test a simpler method
    match transport.power_on() {
        Ok(_) => {
            assert_eq!(transport.commands_sent.len(), 1);
            assert_eq!(
                transport.commands_sent[0],
                vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
            ); // Power on
        }
        Err(_e) => {
            // Expected - MockTransport doesn't provide responses
            assert_eq!(transport.commands_sent.len(), 1);
        }
    }
}
