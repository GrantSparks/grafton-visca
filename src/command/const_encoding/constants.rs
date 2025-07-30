//! Compile-time VISCA command constants.

use crate::{visca_bytes, visca_prefix};

/// Power command constants.
pub mod power {
    use super::*;

    /// Power on command.
    pub const ON: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x02];

    /// Power off/standby command.
    pub const OFF: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x03];
}

/// Pan/Tilt command constants.
pub mod pan_tilt {
    use super::*;

    /// Home position command.
    pub const HOME: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x04];

    /// Reset pan/tilt.
    pub const RESET: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x05];
}

/// Zoom command constants.
pub mod zoom {
    use super::*;

    /// Stop zoom.
    pub const STOP: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x00];

    /// Zoom in (telephoto) standard speed.
    pub const TELE_STD: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x02];

    /// Zoom out (wide) standard speed.
    pub const WIDE_STD: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x03];

    /// Digital zoom control prefix.
    pub const DIGITAL_ZOOM_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x06];
}

/// Focus command constants.
pub mod focus {
    use super::*;

    /// Focus lock control prefix.
    pub const LOCK_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x04, 0x68];
}

/// Exposure command constants.
pub mod exposure {
    use super::*;

    /// Spotlight prefix (Sony models).
    pub const SPOTLIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3A];
}

/// Image flip command constants.
pub mod flip {
    use super::*;

    /// Image flip prefix.
    pub const PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x66];

    /// Horizontal flip prefix.
    pub const HFLIP_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x61];

    /// Image freeze prefix.
    pub const FREEZE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x62];
}

/// Image adjustment command constants.
pub mod image {
    use super::*;

    /// 2D noise reduction prefix.
    pub const NOISE_REDUCTION_2D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x53];

    /// 3D noise reduction prefix.
    pub const NOISE_REDUCTION_3D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x54];

    /// Luminance/brightness adjustment prefix.
    pub const LUMINANCE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00];

    /// Contrast adjustment prefix.
    pub const CONTRAST_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00];
}

/// System command constants.
pub mod system {
    // Constants moved to macro-based implementations in system.rs:
    // - ADDRESS_SET → AddressSetCommand using visca_const_command!
    // - INTERFACE_CLEAR → InterfaceClearCommand using visca_const_command!
    // - COMMAND_CANCEL_PREFIX → CommandCancelCommand with direct encoding
}

/// Color adjustment command constants.
pub mod color {
    use super::*;

    /// One push white balance trigger.
    #[allow(dead_code)]
    pub const WB_ONE_PUSH_TRIGGER: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x10, 0x05];

    /// Color saturation prefix.
    pub const SATURATION_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00];

    /// Color hue prefix.
    pub const HUE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00];

    /// Red gain direct prefix (for WB fine-tuning).
    #[allow(dead_code)]
    pub const RED_GAIN_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x43, 0x00, 0x00];

    /// Blue gain direct prefix (for WB fine-tuning).
    #[allow(dead_code)]
    pub const BLUE_GAIN_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x44, 0x00, 0x00];
}

/// Gain command constants.
pub mod gain {
    use super::*;

    /// Gain limit prefix.
    pub const GAIN_LIMIT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x2C];
}

/// Tally command constants.
pub mod tally {
    use super::*;

    /// Tally control prefix.
    pub const TALLY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00];

    /// Tally brightness prefix (Sony BRC models).
    pub const TALLY_BRIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01];

    /// Green tally prefix (Sony FR7).
    pub const TALLY_GREEN_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00];

    /// PTZOptics tally prefix.
    pub const TALLY_PTZO_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x02, 0x02];
}
