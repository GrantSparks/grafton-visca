//! Camera configuration, construction, profiles, and motion value types.
//!
//! Runtime ownership lives in the async and blocking session types.
//! This module keeps the reusable standard-transport configuration and the
//! profile registry namespace; camera views are exported from their respective
//! async and blocking facades.

pub(crate) mod config;
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) mod construction;
mod movement;
pub(crate) mod profiles;

pub use config::{CameraConfig, TransportKind, TransportOptions};
pub use movement::{IdleWait, MotionQuery, MovementTolerance, PanTiltPosition, PanTiltUnits};
