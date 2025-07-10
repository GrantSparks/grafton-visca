//! Flat re-exports of command constants for easier access.

// Re-export all command constants with flat names
pub use super::constants::power::{ON as POWER_ON, OFF as POWER_OFF};
pub use super::constants::pan_tilt::{
    STOP as PAN_TILT_STOP,
    HOME as PAN_TILT_HOME,
    ABSOLUTE_PREFIX as PAN_TILT_ABSOLUTE_PREFIX,
};
pub use super::constants::zoom::{
    STOP as ZOOM_STOP,
    TELE_VAR_PREFIX as ZOOM_TELE_PREFIX,
    WIDE_VAR_PREFIX as ZOOM_WIDE_PREFIX,
    DIRECT_PREFIX as ZOOM_ABSOLUTE_PREFIX,
};
pub use super::constants::focus::{
    STOP as FOCUS_STOP,
    FAR as FOCUS_FAR_STD,
    NEAR as FOCUS_NEAR_STD,
    AUTO as FOCUS_AUTO,
    MANUAL as FOCUS_MANUAL,
    ONE_PUSH as FOCUS_ONE_PUSH,
};
pub use super::constants::exposure::{
    AUTO as EXPOSURE_AUTO,
    MANUAL as EXPOSURE_MANUAL,
};
pub use super::constants::white_balance::{
    AUTO as WHITE_BALANCE_AUTO,
};
pub use super::constants::flip::{
    FLIP as IMAGE_FLIP_ON,
};
pub use super::constants::preset::{
    RECALL_PREFIX as PRESET_RECALL_PREFIX,
    SET_PREFIX as PRESET_SET_PREFIX,
};

// Focus variable speed prefixes (these need to be constructed)
/// Prefix for variable speed focus near commands.
pub const FOCUS_NEAR_PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x08];
/// Prefix for variable speed focus far commands.
pub const FOCUS_FAR_PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x08];