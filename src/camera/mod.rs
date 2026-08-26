//! Camera configuration, construction, profiles, and motion value types.
//!
//! Runtime ownership lives in the async and blocking session types.
//! This module keeps the reusable standard-transport configuration and the
//! profile registry namespace; camera views are exported from their respective
//! async and blocking facades.

mod config;
#[cfg(feature = "async")]
mod construction;
mod movement;
pub mod profiles;

pub use config::{CameraConfig, TransportKind, TransportOptions};
#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
pub use construction::SerialConnectBuilder;
#[cfg(feature = "async")]
pub use construction::{Connect, ConnectBuilder, TcpConnectBuilder, UdpConnectBuilder};
pub use movement::{IdleWait, MotionQuery, MovementTolerance, PanTiltPosition};
