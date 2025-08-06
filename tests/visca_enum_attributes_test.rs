//! Tests for ViscaEnum derive macro with attribute support

use grafton_visca::ViscaEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[visca_enum(exhaustive = true, error_type = grafton_visca::Error)]
pub enum AdvancedMode {
    #[visca_enum(name = "Automatic Mode")]
    Auto = 0x00,

    #[visca_enum(name = "Manual Control")]
    Manual = 0x03,

    #[visca_enum(name = "Advanced Shutter")]
    Shutter = 0x0A,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[visca_enum(error_type = grafton_visca::Error)]
pub enum ModeWithSkip {
    Active = 0x01,

    Inactive = 0x02,

    #[visca_enum(skip)]
    _Reserved = 0xFF,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_custom_names_in_error_messages() {
        // This should fail with custom names in the error message
        let result = AdvancedMode::try_from(0xFF);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{err}");

        // Check that custom names appear in error message
        assert!(err_msg.contains("Automatic Mode"));
        assert!(err_msg.contains("Manual Control"));
        assert!(err_msg.contains("Advanced Shutter"));
    }

    #[test]
    fn test_is_valid_discriminant() {
        // Test valid discriminants
        assert!(AdvancedMode::is_valid_discriminant(0x00));
        assert!(AdvancedMode::is_valid_discriminant(0x03));
        assert!(AdvancedMode::is_valid_discriminant(0x0A));

        // Test invalid discriminants
        assert!(!AdvancedMode::is_valid_discriminant(0x01));
        assert!(!AdvancedMode::is_valid_discriminant(0xFF));
    }

    #[test]
    fn test_skip_attribute() {
        // Valid values should work
        assert_eq!(ModeWithSkip::try_from(0x01).unwrap(), ModeWithSkip::Active);
        assert_eq!(
            ModeWithSkip::try_from(0x02).unwrap(),
            ModeWithSkip::Inactive
        );

        // Skipped value should not be recognized
        assert!(ModeWithSkip::try_from(0xFF).is_err());

        // is_valid_discriminant should return false for skipped values
        assert!(!ModeWithSkip::is_valid_discriminant(0xFF));
        assert!(ModeWithSkip::is_valid_discriminant(0x01));
        assert!(ModeWithSkip::is_valid_discriminant(0x02));
    }

    #[test]
    fn test_from_enum_to_u8_with_attributes() {
        assert_eq!(u8::from(AdvancedMode::Auto), 0x00);
        assert_eq!(u8::from(AdvancedMode::Manual), 0x03);
        assert_eq!(u8::from(AdvancedMode::Shutter), 0x0A);

        assert_eq!(u8::from(ModeWithSkip::Active), 0x01);
        assert_eq!(u8::from(ModeWithSkip::Inactive), 0x02);
        // Note: _Reserved variant cannot be tested in From conversion as it's not accessible
    }
}
