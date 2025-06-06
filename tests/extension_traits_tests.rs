//! Tests for the high-level extension traits.

#[path = "common/mod.rs"]
mod common;

use common::MockDevice;
use grafton_visca::{
    ImagePreset, ViscaCommand, ViscaDevice, ViscaError, ViscaExposureExt, ViscaImageExt,
    ViscaPositionExt, ViscaResponse, ViscaTransportExt, ViscaWhiteBalanceExt, ViscaZoomExt,
    WhiteBalancePreset,
};

#[test]
fn test_exposure_ext_methods() {
    let mut device = MockDevice::with_completion();

    // Test iris control
    ViscaExposureExt::set_iris(&mut device, 0x0C).unwrap();
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x4B]
    );

    // Test shutter speed
    device.set_shutter_speed(0x10).unwrap();
    assert_eq!(
        device.last_command().unwrap()[0..4],
        vec![0x81, 0x01, 0x04, 0x4A]
    );
}

#[test]
fn test_white_balance_ext_methods() {
    let mut device = MockDevice::with_completion();

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
    let mut device = MockDevice::with_completion();

    // Test image preset
    device.apply_image_preset(ImagePreset::Vivid).unwrap();

    // This would be testing a composite operation, just verify a command was sent
    assert!(!device.commands_sent().is_empty());
}

#[test]
fn test_zoom_ext_methods() {
    let mut device = MockDevice::with_completion();

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
    let mut device = MockDevice::with_completion();

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
    let mut device = MockDevice::with_completion();

    // Test power on convenience method
    match device.power_on() {
        Ok(()) => {
            assert_eq!(
                device.last_command().unwrap()[0..4],
                vec![0x81, 0x01, 0x04, 0x00]
            );
            assert_eq!(device.last_command().unwrap()[4], 0x02);
        }
        Err(e) => panic!("Power on failed: {:?}", e),
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
    let mut device = MockDevice::with_completion();

    // Chain multiple operations
    device.power_on().unwrap();
    device.zoom_to_magnification(2.0).unwrap();
    device.move_to_degrees(0.0, 0.0, None).unwrap();

    // Verify multiple commands were sent
    assert!(device.commands_sent().len() >= 3);
}