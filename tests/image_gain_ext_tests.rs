//! Tests for image and gain extension trait methods.

#![cfg(feature = "blocking-client")]

use grafton_visca::{
    command::{AntiFlickerCommand, AntiFlickerMode, LuminanceCommand},
    Error, LuminanceLevel, Response, Transport, ViscaCommand, ViscaExposureExt, ViscaImageExt,
};

/// Mock device for testing extension traits
struct MockDevice {
    last_command: Option<Vec<u8>>,
}

impl MockDevice {
    fn new() -> Self {
        Self { last_command: None }
    }

    fn assert_command_bytes(&self, expected: &[u8]) {
        assert_eq!(
            self.last_command.as_ref().unwrap(),
            expected,
            "Command bytes don't match"
        );
    }
}

impl Transport for MockDevice {
    fn execute_command(&mut self, command: &dyn ViscaCommand) -> Result<Response, Error> {
        self.last_command = Some(command.to_bytes()?);
        Ok(Response::Completion)
    }
}

#[test]
fn test_set_anti_flicker() {
    let mut device = MockDevice::new();

    // Test Off mode
    device.set_anti_flicker(AntiFlickerMode::Off).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0x23, 0x00, 0xFF]);

    // Test 50Hz mode
    device.set_anti_flicker(AntiFlickerMode::Hz50).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0x23, 0x01, 0xFF]);

    // Test 60Hz mode
    device.set_anti_flicker(AntiFlickerMode::Hz60).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0x23, 0x02, 0xFF]);
}

#[test]
fn test_set_luminance() {
    let mut device = MockDevice::new();

    // Test minimum luminance
    device.set_luminance(0).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x00, 0xFF]);

    // Test default luminance
    device.set_luminance(7).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF]);

    // Test maximum luminance
    device.set_luminance(14).unwrap();
    device.assert_command_bytes(&[0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x0E, 0xFF]);

    // Test out of range
    let result = device.set_luminance(15);
    assert!(result.is_err());
    if let Err(Error::InvalidParameter(msg)) = result {
        assert!(msg.contains("0-14"));
    } else {
        panic!("Expected InvalidParameter error");
    }
}

#[test]
fn test_anti_flicker_command_direct() {
    // Test command construction directly
    let cmd = AntiFlickerCommand {
        mode: AntiFlickerMode::Hz50,
    };
    assert_eq!(
        cmd.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x23, 0x01, 0xFF]
    );
}

#[test]
fn test_luminance_command_direct() {
    // Test command construction directly
    let cmd = LuminanceCommand {
        value: LuminanceLevel::new(10).unwrap(),
    };
    assert_eq!(
        cmd.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x0A, 0xFF]
    );
}
