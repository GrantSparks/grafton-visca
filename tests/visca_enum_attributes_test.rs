//! Value-enum attributes share one display and wire conversion contract.

use grafton_visca::{value_enum, wire::WireValue, Error};

#[value_enum]
pub enum AdvancedMode {
    #[value(display = "Automatic Mode")]
    Auto = 0x00,
    #[value(display = "Manual Control")]
    Manual = 0x03,
    #[value(display = "Advanced Shutter")]
    Shutter = 0x0A,
}

#[value_enum]
pub enum ExtensibleMode {
    Active = 0x01,
    Inactive = 0x02,
    Unknown(u8),
}

#[test]
fn custom_display_names_are_preserved() {
    assert_eq!(AdvancedMode::Auto.to_string(), "Automatic Mode");
    assert_eq!(AdvancedMode::Manual.to_string(), "Manual Control");
    assert_eq!(AdvancedMode::Shutter.to_string(), "Advanced Shutter");
}

#[test]
fn closed_enum_rejects_unknown_wire_codes_with_the_crate_error() {
    for code in [0x00, 0x03, 0x0A] {
        let mode = AdvancedMode::from_wire(code).expect("documented code");
        assert_eq!(mode.to_wire(), code);
    }
    for code in [0x01, 0xFF] {
        let error: Error = AdvancedMode::from_wire(code).expect_err("undocumented code");
        assert!(
            matches!(error, Error::InvalidParameter { parameter: "AdvancedMode", value, reason, .. }
            if value == code.to_string() && reason == "unknown wire value")
        );
    }
}

#[test]
fn extensible_enum_preserves_unknown_codes_in_both_directions() {
    assert_eq!(
        ExtensibleMode::from_wire(0x01).unwrap(),
        ExtensibleMode::Active
    );
    assert_eq!(
        ExtensibleMode::from_wire(0x02).unwrap(),
        ExtensibleMode::Inactive
    );
    for code in [0, 3, 0xFF] {
        assert_eq!(
            ExtensibleMode::from_wire(code).unwrap(),
            ExtensibleMode::Unknown(code)
        );
        assert_eq!(ExtensibleMode::Unknown(code).to_wire(), code);
    }
}
