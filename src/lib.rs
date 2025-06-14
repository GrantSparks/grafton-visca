//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo
)]
//!
//! ## What is VISCA?
//!
//! VISCA (Video System Control Architecture) is a protocol developed by Sony for controlling PTZ cameras
//! commonly used in robotics, broadcasting, video conferencing, and surveillance applications. This crate
//! implements VISCA over IP, allowing you to control networked PTZ cameras from Rust applications.
//!
//! ## Features
//!
//! - **Complete Command Coverage**: Full support for `PTZOptics` G2 VISCA commands
//! - **Robust Protocol Handling**: Proper ACK/Completion state machine with socket management
//! - **Multiple Transports**: Both UDP (port 1259 default) and TCP (port 5678 default) support
//! - **Async Support**: Modern async/await API with Tokio (enable with `async` feature)
//! - **Thread Safety**: Safe concurrent access from multiple tasks
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Error Handling**: Detailed error types for all VISCA error conditions
//!
//! ## Supported Commands
//!
//! ### Camera Movement
//! - Pan/Tilt/Zoom control with absolute and relative positioning
//! - Variable speed control for smooth movements
//! - Home position and preset management (up to 90 presets)
//!
//! ### Exposure & Color
//! - Exposure modes: Auto, Manual, Shutter Priority, Iris Priority, Bright
//! - Iris, shutter speed, gain, and brightness control
//! - White balance modes including manual color temperature
//! - Color adjustments: saturation, hue, RGB gain tuning
//!
//! ### Image Control
//! - Focus control with auto/manual modes and zone selection
//! - Sharpness adjustment with auto/manual modes
//! - Noise reduction (2D and 3D)
//! - Image flip (horizontal/vertical)
//! - Black & white mode
//!
//! ## Example Usage
//!
//! ### Using the Prelude
//!
//! The easiest way to get started is to use the prelude module which imports
//! all commonly used types and traits:
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::prelude::*;
//!
//! let mut client = Client::connect_udp("192.168.1.100:5678").unwrap();
//!
//! // All extension traits and types are available
//! client.power_on().unwrap();
//! client.move_to_degrees(45.0, 30.0, None).unwrap();
//!
//! // Create speed parameters easily
//! let pan_speed = PanSpeed::new(10).unwrap();
//! let tilt_speed = TiltSpeed::new(10).unwrap();
//! client.start_moving(PanTiltDirection::UpRight, pan_speed, tilt_speed).unwrap();
//! # }
//! ```
//!
//! ### Using `Client` (Recommended for Thread Safety)
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::Client;
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Create a client using the v0.4.0 unified API
//! let mut client = Client::connect_udp("192.168.1.100:5678").unwrap();
//!
//! // Send commands through the client
//! client.send(&PanTiltCommand::Home).unwrap();
//! client.send(&ZoomCommand::ZoomInStandard).unwrap();
//! # }
//! ```
//!
//!
//! ## Advanced Camera Control
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::{Client, IrisLevel};
//! use grafton_visca::command::{ExposureCompensationCommand, IrisCommand, SaturationCommand};
//! use grafton_visca::command::exposure::ExposureCompensationLevel;
//!
//! let mut client = Client::connect_udp("192.168.1.100:5678").unwrap();
//!
//! // Adjust exposure compensation
//! client.send(&ExposureCompensationCommand::Direct(ExposureCompensationLevel::new(3).unwrap())).unwrap();
//!
//! // Set iris to F4.0
//! client.send(&IrisCommand::Direct(IrisLevel::new(0x06).unwrap())).unwrap();
//!
//! // Adjust color saturation to 150%
//! client.send(&SaturationCommand { level: 0x0A }).unwrap();
//! # }
//! ```
//!
//! ## Creating Custom Commands with Macro
//!
//! For simple commands, you can use the built-in macro to reduce boilerplate:
//!
//! ```no_run
//! use grafton_visca::visca_command;
//!
//! visca_command! {
//!     category = "Movement",
//!     enum CustomCommand {
//!         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
//!         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
//!     }
//! }
//! ```
//!
//! ## Async Usage (with `async` feature)
//!
//! The library provides async support for non-blocking camera control:
//!
//! ```no_run
//! # #[cfg(feature = "async-client")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use grafton_visca::{Client, Response};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//! use grafton_visca::command::pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed};
//!
//! // Connect to camera
//! let camera = Client::connect_udp_async("192.168.1.100:5678").await?;
//!
//! // Send multiple commands concurrently
//! let pan_tilt_cmd = PanTiltCommand::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10)?,
//!     tilt_speed: TiltSpeed::new(0x10)?,
//! };
//! let pan_tilt = camera.send_async(&pan_tilt_cmd);
//! let zoom = camera.send_async(&ZoomCommand::ZoomInStandard);
//!
//! // Both commands execute concurrently (respecting the 2-socket limit)
//! let (pan_result, zoom_result): (Result<Response, _>, Result<Response, _>) = tokio::join!(pan_tilt, zoom);
//! # Ok(())
//! # }
//! ```
//!
//! ## API Overview
//!
//! The crate provides several levels of API for different use cases:
//!
//! ### High-Level Client
//! - [`Client`] - Thread-safe wrapper for concurrent camera control (recommended)
//!   - Eliminates need for `RefCell` in user code
//!   - Supports Clone for sharing between threads
//!   - Provides `send()`, `try_send()`, and `send_with_timeout()` methods
//!
//! ### Transport Layer
//! - Transport implementations are available in the [`transport`] module
//! - UDP transport (default port 1259 for VISCA over IP)
//! - TCP transport (default port 5678 for VISCA over IP)
//!
//! ### Command Layer
//! - [`Command`] trait - Implemented by all command types
//! - Command modules in [`command`] - Organized by functionality
//!
//! ### Response Handling
//! - [`Response`] - Enum for all response types (ACK, Completion, Inquiry, Error)
//! - [`InquiryResponse`] - Specific inquiry response variants
//! - [`Error`] - Comprehensive error types for all failure modes
//!
//! ### Async Support (with `async` feature)
//! - `AsyncClient` - High-level async client with automatic socket management
//!
//! ## Connection Setup
//!
//! Cameras typically listen on standard ports:
//! - **UDP**: Port 1259 (`PTZOptics` default for VISCA over IP)
//! - **TCP**: Port 5678 (Alternative port, check your camera's configuration)
//!
//! Ensure your camera is configured for VISCA over IP and note its IP address.
//!
//! ## Troubleshooting
//!
//! ### Common Errors
//!
//! - **`CommandBufferFull`**: The camera can only process 2 commands simultaneously.
//!   Solution: Wait for previous commands to complete before sending new ones.
//!
//! - **`NoSocket`**: No command is currently executing in the requested socket.
//!   This usually indicates a protocol synchronization issue.
//!
//! - **`CommandNotExecutable`**: The command cannot be executed in the current camera state.
//!   Example: Trying to zoom while the camera is powered off.
//!
//! - **`SyntaxError`**: The command format is incorrect or parameters are out of range.
//!   Check that speed values and positions are within valid ranges.
//!
//! ### Best Practices
//!
//! 1. **Connection Management**: Reuse transport instances when possible rather than
//!    creating new connections for each command.
//!
//! 2. **Error Handling**: Always handle errors appropriately - cameras may reject
//!    commands due to mechanical limits or current state.
//!
//! 3. **Timing**: Allow time for mechanical movements to complete. The library handles
//!    protocol-level completion, but physical movement takes time.
//!
//! 4. **Concurrent Commands**: When using async, the library automatically manages
//!    the 2-socket limitation, but be aware that commands may queue.
//!
//! ## Feature Flags
//!
//! - `async` - Enables async/await support with Tokio
//! - `sync` - Enables synchronous API (default)
//! - `full` - Enables both sync and async APIs
//!
//! ## Usage
//!
//! The recommended way to use this library is through the unified `Client`:
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # fn main() -> Result<(), grafton_visca::Error> {
//! # use grafton_visca::{Client};
//! # use grafton_visca::command::{PowerCommand, power::Power};
//! let client = Client::connect_udp("192.168.1.100:5678")?;
//! let response = client.send(&PowerCommand { power: Power::On })?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "blocking-client"))]
//! # fn main() {}
//! ```
//!

// Public modules
/// VISCA command definitions and implementations
pub mod command;
/// Connection management for VISCA devices
pub mod connection;
/// Connection pooling for managing multiple VISCA device connections
pub mod connection_pool;
/// VISCA protocol constants and definitions
pub mod constants;
/// Utility macros for VISCA operations
pub mod macros;
/// Common imports for grafton-visca users
pub mod prelude;
/// Timeout configuration and management
pub mod timeout;
/// Transport layer implementations for VISCA communication
pub mod transport;

/// Async transport layer implementations
#[cfg(feature = "async-client")]
pub mod transport_future;

/// Reconnecting transport wrapper for handling connection failures
#[cfg(feature = "async-client")]
pub mod reconnecting_transport;

// Private modules
mod api;
mod error;
mod ext;
mod session;
mod types;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod ptz_builder;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod sync_primitives;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod unified_client;

// Core re-exports
pub use crate::{
    command::{
        pan_tilt::PanTiltDirection,
        response::{parse_visca_response, Response},
        Command, InquiryResponse, ResponseType,
    },
    error::{Error, ResultExt, ViscaRetry},
    session::Session,
};

// Parameter types re-exports
pub use crate::command::{
    exposure::DynamicRangeLevel,
    focus::FocusSpeed,
    pan_tilt::{PanSpeed, TiltSpeed},
    preset::PresetNumber,
    zoom::ZoomSpeed,
};

// Type safety re-exports
pub use crate::types::{
    BrightnessLevel, ContrastLevel, GainLimit, GainValue, IrisLevel, LuminanceLevel,
    NoiseReduction2DLevel, NoiseReduction3DLevel, SharpnessLevel, ShutterSpeed, SocketId,
};

// Connection and pooling re-exports
pub use crate::{
    connection::{ConnectionManagement, ConnectionStats, ConnectionStatsSnapshot},
    connection_pool::{CameraInfo, ConnectionPool, ConnectionType, PoolConfig, PooledCameraStats},
    timeout::{CommandCategory, TimeoutConfig, TimeoutConfigBuilder},
};

// Extension trait re-exports
pub use crate::{
    api::{CameraControl, GainLevel, IrisValue, NoiseReductionStrength, PanTiltBuilder, Speed},
    ext::{
        exposure_ext::{ExposureExt, ExposurePreset},
        focus_ext::FocusExt,
        image_ext::{ImageExt, ImagePreset},
        inquiry_ext::{
            CameraPosition, CameraState, ExposureState, ImageState, InquiryExt, OpticsState,
            WhiteBalanceState,
        },
        pan_tilt_ext::PanTiltExt,
        position_ext::PositionExt,
        power_ext::PowerExt,
        preset_ext::PresetExt,
        transport_ext::TransportExt,
        white_balance_ext::{WhiteBalanceExt, WhiteBalancePreset},
        zoom_ext::ZoomExt,
    },
};

// Unified extension trait re-exports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::ext::unified_ext::CameraExt;

#[cfg(feature = "async-client")]
pub use crate::ext::unified_ext::AsyncCameraExt;

// Unified client re-exports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::{
    ptz_builder::PtzBuilder,
    unified_client::{Client, ClientPtzExt},
};

// Async-specific re-exports
#[cfg(feature = "async-client")]
pub use crate::{
    connection::AsyncConnectionManagement,
    connection_pool::AsyncConnectionPool,
    ext::async_visca_ext::{AsyncExt, PanScanDirection},
    reconnecting_transport::{
        ConnectionEvent, ConnectionEventCallback, ReconnectingTransport, ReconnectionConfig,
    },
    transport_future::TransportFuture,
};

/// Core trait for types that can send and receive VISCA commands.
///
/// This trait provides the minimal interface needed for the extension traits.
/// It is implemented by `Client` and provides the foundation for all
/// high-level camera control operations.
pub trait Transport {
    /// Send a command and wait for the response.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send, the camera returns an error,
    /// or if communication with the camera fails.
    fn execute_command(&mut self, command: &dyn Command) -> Result<Response, Error>;
}
