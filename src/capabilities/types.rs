//! Supporting types used across capability traits.

// Re-export types that are used by multiple capability traits
pub use crate::capabilities::exposure::ShutterSpeed;
pub use crate::capabilities::focus::{AutoFocusSensitivity, FocusZone};
pub use crate::capabilities::image_processing::{ImageFlipMode, NoiseReductionLevel, SharpnessMode};
pub use crate::capabilities::nd_filter::NDFilterMode;
pub use crate::capabilities::power::PowerState;
pub use crate::capabilities::presets::PresetTour;
pub use crate::capabilities::white_balance::WhiteBalanceMode;