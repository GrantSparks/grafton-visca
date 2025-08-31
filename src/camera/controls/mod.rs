//! Method implementations for cameras based on their capabilities.
//!
//! Each module contains trait definitions for camera control methods.
//! Methods are implemented directly on the Camera struct with separate async and blocking implementations.

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
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Note: Methods are implemented directly on AsyncCamera and BlockingCamera types
// through the individual control traits defined in each module.
