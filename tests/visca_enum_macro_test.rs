//! Tests for the ViscaEnum derive macro

use grafton_visca::ExposureMode;

#[test]
fn test_exposure_mode_try_from_u8() {
    // Valid conversions
    assert_eq!(ExposureMode::try_from(0x00).unwrap(), ExposureMode::Auto);
    assert_eq!(ExposureMode::try_from(0x03).unwrap(), ExposureMode::Manual);
    assert_eq!(ExposureMode::try_from(0x0A).unwrap(), ExposureMode::Shutter);
    assert_eq!(ExposureMode::try_from(0x0B).unwrap(), ExposureMode::Iris);
    assert_eq!(ExposureMode::try_from(0x0D).unwrap(), ExposureMode::Bright);

    // Invalid conversion
    let result = ExposureMode::try_from(0xFF);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("Invalid response"));
    assert!(err_str.contains("0x00 (Auto)"));
    assert!(err_str.contains("0x0D (Bright)"));
}

#[test]
fn test_exposure_mode_from_enum_to_u8() {
    // Test From<ExposureMode> for u8
    assert_eq!(u8::from(ExposureMode::Auto), 0x00);
    assert_eq!(u8::from(ExposureMode::Manual), 0x03);
    assert_eq!(u8::from(ExposureMode::Shutter), 0x0A);
    assert_eq!(u8::from(ExposureMode::Iris), 0x0B);
    assert_eq!(u8::from(ExposureMode::Bright), 0x0D);
}

#[test]
fn test_exposure_mode_round_trip() {
    // Test that we can convert back and forth
    let modes = [
        ExposureMode::Auto,
        ExposureMode::Manual,
        ExposureMode::Shutter,
        ExposureMode::Iris,
        ExposureMode::Bright,
    ];

    for mode in &modes {
        let byte_value = u8::from(*mode);
        let converted_back = ExposureMode::try_from(byte_value).unwrap();
        assert_eq!(*mode, converted_back);
    }
}
