//! Built-in value enums expose wire codes only through `WireValue`.

use grafton_visca::{wire::WireValue, ExposureMode};

#[test]
fn exposure_mode_wire_values_match_the_protocol() {
    for (code, mode) in [
        (0x00, ExposureMode::Auto),
        (0x03, ExposureMode::Manual),
        (0x0A, ExposureMode::Shutter),
        (0x0B, ExposureMode::Iris),
        (0x0D, ExposureMode::Bright),
    ] {
        assert_eq!(ExposureMode::from_wire(code).unwrap(), mode);
        assert_eq!(mode.to_wire(), code);
        assert_eq!(ExposureMode::from_wire(mode.to_wire()).unwrap(), mode);
    }
    assert!(ExposureMode::from_wire(0xFF).is_err());
}
