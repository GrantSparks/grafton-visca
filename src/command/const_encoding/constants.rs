//! Compile-time VISCA command constants.

use crate::{visca_bytes, visca_prefix};

/// Power command constants.
pub mod power {
    use super::*;
    
    /// Power on command.
    pub const ON: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x02];
    
    /// Power off/standby command.
    pub const OFF: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x03];
    
    /// Power query command.
    pub const QUERY: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x00];
}

/// Pan/Tilt command constants.
pub mod pan_tilt {
    use super::*;
    
    /// Stop all pan/tilt movement.
    pub const STOP: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x01, 0x18, 0x18, 0x03, 0x03];
    
    /// Home position command.
    pub const HOME: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x04];
    
    /// Reset pan/tilt.
    pub const RESET: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x05];
    
    /// Absolute position prefix (needs pan/tilt speeds and positions).
    pub const ABSOLUTE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x02];
    
    /// Relative position prefix.
    pub const RELATIVE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x03];
    
    /// Query position.
    pub const QUERY_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x06, 0x12];
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
    
    /// Variable speed zoom prefix (tele).
    pub const TELE_VAR_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x07];
    
    /// Variable speed zoom prefix (wide).
    pub const WIDE_VAR_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x07];
    
    /// Direct zoom position prefix.
    pub const DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x47];
    
    /// Query zoom position.
    pub const QUERY_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x47];
}

/// Focus command constants.
pub mod focus {
    use super::*;
    
    /// Stop focus.
    pub const STOP: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x08, 0x00];
    
    /// Focus far.
    pub const FAR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x08, 0x02];
    
    /// Focus near.
    pub const NEAR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x08, 0x03];
    
    /// Auto focus mode.
    pub const AUTO: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x38, 0x02];
    
    /// Manual focus mode.
    pub const MANUAL: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x38, 0x03];
    
    /// Auto/Manual toggle.
    pub const TOGGLE: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x38, 0x10];
    
    /// One push auto focus trigger.
    pub const ONE_PUSH: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x18, 0x01];
    
    /// Focus to infinity.
    pub const INFINITY: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x18, 0x02];
    
    /// Direct focus position prefix.
    pub const DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x48];
    
    /// Query focus position.
    pub const QUERY_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x48];
    
    /// Query focus mode.
    pub const QUERY_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x38];
}

/// Preset command constants.
pub mod preset {
    use super::*;
    
    /// Reset preset prefix (needs preset number).
    pub const RESET_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3F, 0x00];
    
    /// Set preset prefix (needs preset number).
    pub const SET_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3F, 0x01];
    
    /// Recall preset prefix (needs preset number).
    pub const RECALL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3F, 0x02];
}

/// Exposure command constants.
pub mod exposure {
    use super::*;
    
    /// Auto exposure mode.
    pub const AUTO: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x39, 0x00];
    
    /// Manual exposure mode.
    pub const MANUAL: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x39, 0x03];
    
    /// Shutter priority mode.
    pub const SHUTTER_PRIORITY: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x39, 0x0A];
    
    /// Iris priority mode.
    pub const IRIS_PRIORITY: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x39, 0x0B];
    
    /// Bright mode.
    pub const BRIGHT: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x39, 0x0D];
    
    /// Iris direct prefix.
    pub const IRIS_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4B];
    
    /// Shutter direct prefix.
    pub const SHUTTER_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4A];
    
    /// Gain direct prefix.
    pub const GAIN_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4C];
}

/// White balance command constants.
pub mod white_balance {
    use super::*;
    
    /// Auto white balance.
    pub const AUTO: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x00];
    
    /// Indoor white balance.
    pub const INDOOR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x01];
    
    /// Outdoor white balance.
    pub const OUTDOOR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x02];
    
    /// One push white balance.
    pub const ONE_PUSH: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x03];
    
    /// Auto tracking white balance.
    pub const ATW: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x04];
    
    /// Manual white balance.
    pub const MANUAL: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x35, 0x05];
    
    /// One push trigger.
    pub const ONE_PUSH_TRIGGER: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x10, 0x05];
}

/// Image flip command constants.
pub mod flip {
    use super::*;
    
    /// Image flip off.
    pub const OFF: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x66, 0x00];
    
    /// Flip image vertically.
    pub const FLIP: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x66, 0x02];
    
    /// Mirror image horizontally.
    pub const MIRROR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x66, 0x01];
    
    /// Flip and mirror (180° rotation).
    pub const FLIP_MIRROR: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x66, 0x03];
}