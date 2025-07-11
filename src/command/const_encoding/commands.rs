//! Flat re-exports of command constants for easier access.

// Re-export all command constants with flat names
pub use super::constants::exposure::{AUTO as EXPOSURE_AUTO, MANUAL as EXPOSURE_MANUAL};
pub use super::constants::flip::{PREFIX as FLIP_PREFIX, HFLIP_PREFIX, FREEZE_PREFIX};
pub use super::constants::focus::{
    AUTO as FOCUS_AUTO, FAR as FOCUS_FAR_STD, MANUAL as FOCUS_MANUAL, NEAR as FOCUS_NEAR_STD,
    ONE_PUSH as FOCUS_ONE_PUSH, STOP as FOCUS_STOP,
};
pub use super::constants::pan_tilt::{
    ABSOLUTE_PREFIX as PAN_TILT_ABSOLUTE_PREFIX, HOME as PAN_TILT_HOME, STOP as PAN_TILT_STOP,
};
pub use super::constants::power::{OFF as POWER_OFF, ON as POWER_ON};
pub use super::constants::preset::{
    RECALL_PREFIX as PRESET_RECALL_PREFIX, SET_PREFIX as PRESET_SET_PREFIX,
};
pub use super::constants::white_balance::AUTO as WHITE_BALANCE_AUTO;
pub use super::constants::zoom::{
    DIRECT_PREFIX as ZOOM_ABSOLUTE_PREFIX, STOP as ZOOM_STOP, TELE_VAR_PREFIX as ZOOM_TELE_PREFIX,
    WIDE_VAR_PREFIX as ZOOM_WIDE_PREFIX,
};

// Focus variable speed prefixes (these need to be constructed)
/// Prefix for variable speed focus near commands.
pub const FOCUS_NEAR_PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x08];
/// Prefix for variable speed focus far commands.
pub const FOCUS_FAR_PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x08];
