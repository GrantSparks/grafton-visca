//! Method implementations for cameras based on their capabilities.
//!
//! Each module contains legacy trait definitions that are no longer used.
//! Methods are now implemented directly on the Camera struct with mode markers.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod menu;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod unified_power;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Note: Trait exports have been removed as methods are now implemented
// directly on Camera<AsyncMode, P, T> and Camera<BlockingMode, P, T>
