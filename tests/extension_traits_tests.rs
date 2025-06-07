//! Tests for the high-level extension traits.

#![cfg(feature = "blocking-client")]

#[path = "common/mod.rs"]
mod common;

use common::{MockDevice, MockTransport};
use grafton_visca::{
    ImagePreset, ViscaExposureExt, ViscaImageExt, ViscaPositionExt, ViscaTransportExt,
    ViscaWhiteBalanceExt, ViscaZoomExt, WhiteBalancePreset,
};

#[test]
fn test_exposure_ext_methods() {
    let transport = MockTransport::new();
    // Add responses for both commands
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test iris control
    ViscaExposureExt::set_iris(&mut device, 0x0C).unwrap();
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x4B]
    );

    // Test shutter speed
    device.set_shutter(0x10).unwrap();
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x4A]
    );
}

#[test]
fn test_white_balance_ext_methods() {
    let transport = MockTransport::new();
    // Daylight preset sends 2 commands: mode + temperature
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    // Direct temperature setting sends 1 command
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test white balance preset - Daylight first sets ColorTemperature mode
    device
        .set_white_balance_preset(WhiteBalancePreset::Daylight)
        .unwrap();
    // The last command would be the color temperature direct command
    // since Daylight preset first sets mode to ColorTemperature, then sets temp to 0x1C
    assert_eq!(
        device.last_command().unwrap()[0..5],
        vec![0x81, 0x01, 0x04, 0x20, 0x00]
    );

    // Test direct color temperature setting (5600K)
    device.set_color_temperature_direct(30).unwrap(); // 30 = roughly 5600K
                                                      // The color temperature direct command uses 0x20
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x20]
    );
}

#[test]
fn test_image_ext_methods() {
    let transport = MockTransport::new();
    // Vivid preset sends 4 commands: sharpness, saturation, contrast, hue
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test image preset
    device.apply_image_preset(ImagePreset::Vivid).unwrap();

    // Verify 4 commands were sent for the preset
    assert_eq!(device.commands_sent().len(), 4);

    // Check the last command was hue (the preset sets sharpness, saturation, contrast, hue in that order)
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x4F]
    );
}

#[test]
fn test_zoom_ext_methods() {
    let transport = MockTransport::new();
    // Zoom to magnification sends 1 command
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test zoom to magnification
    device.zoom_to_magnification(5.0).unwrap();

    // Test that zoom position command was sent
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x47]
    );
}

#[test]
fn test_position_ext_methods() {
    let transport = MockTransport::new();
    // Move to degrees sends 1 command
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test move to degrees
    device.move_to_degrees(45.0, 15.0, Some((10, 10))).unwrap();

    // Test that absolute position command was sent
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x06, 0x02]
    );
}

#[test]
fn test_transport_ext_methods() {
    let transport = MockTransport::new();
    // Power on sends 1 command, power off sends 1 command
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Test power on convenience method
    match device.power_on() {
        Ok(()) => {
            assert_eq!(
                device.last_command().unwrap()[0..4],
                vec![0x81, 0x01, 0x04, 0x00]
            );
            assert_eq!(device.last_command().unwrap()[4], 0x02);
        }
        Err(e) => panic!("Power on failed: {e:?}"),
    }

    // Test power off
    device.power_off().unwrap();
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x00]
    );
    assert_eq!(device.last_command().unwrap()[4], 0x03);
}

#[test]
fn test_chained_operations() {
    let transport = MockTransport::new();
    // 3 commands for chained operations
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    transport.add_ack_completion(0);
    let mut device = MockDevice::from_transport(transport);

    // Chain multiple operations
    device.power_on().unwrap();
    device.zoom_to_magnification(2.0).unwrap();
    device.move_to_degrees(0.0, 0.0, None).unwrap();

    // Verify multiple commands were sent
    assert_eq!(device.commands_sent().len(), 3);
}
