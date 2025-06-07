//! # grafton-visca
//!
//! A production-ready Rust implementation of the VISCA over IP protocol for controlling PTZ (Pan-Tilt-Zoom) cameras.
#![warn(missing_docs)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    rust_2018_idioms
)]
// Targeted allows for legitimate patterns
#![allow(
    clippy::module_name_repetitions, // Common in Rust APIs (e.g., ViscaCommand, ViscaError)
    clippy::cast_sign_loss,          // Some are intentional after validation
    clippy::cast_possible_truncation,// Some are intentional after validation
    clippy::cast_possible_wrap,      // Some are intentional after validation
    clippy::cast_precision_loss,     // Some are intentional
    clippy::float_cmp                // Tests need exact float comparisons
)]
// Temporary allows - should be fixed
#![allow(
    clippy::use_self,                // TODO: 160+ instances need systematic refactor
    clippy::must_use_candidate,      // TODO: Add #[must_use] where appropriate
    clippy::missing_panics_doc,      // TODO: Add # Panics sections to docs
    clippy::missing_errors_doc,      // TODO: Add # Errors sections to docs (many instances)
    clippy::uninlined_format_args,   // TODO: Update format strings to use inline syntax
    clippy::return_self_not_must_use,// TODO: Add #[must_use] to builder methods
    clippy::single_match_else,       // TODO: Convert to if let where appropriate
    clippy::significant_drop_tightening, // TODO: Review mutex lock scopes
    missing_docs,                    // TODO: Add documentation for all public items
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
//! ### Using `ViscaClient` (Recommended for Thread Safety)
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::ViscaClient;
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Create a client using the v0.4.0 unified API
//! let client = ViscaClient::connect_udp("192.168.1.100:5678").unwrap();
//!
//! // Send commands through the client
//! client.send(&PanTiltCommand::Home).unwrap();
//! client.send(&ZoomCommand::TeleStandard).unwrap();
//! # }
//! ```
//!
//!
//! ## Advanced Camera Control
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::ViscaClient;
//! use grafton_visca::command::{ExposureCompensationCommand, IrisCommand, SaturationCommand};
//! use grafton_visca::command::exposure::ExposureCompensationLevel;
//!
//! let client = ViscaClient::connect_udp("192.168.1.100:5678").unwrap();
//!
//! // Adjust exposure compensation
//! client.send(&ExposureCompensationCommand::Direct(ExposureCompensationLevel::new(3).unwrap())).unwrap();
//!
//! // Set iris to F4.0
//! client.send(&IrisCommand::Direct(0x06)).unwrap();
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
//!     #[category = "Movement"]
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
//! use grafton_visca::{ViscaClient, ViscaResponse};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//! use grafton_visca::command::pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed};
//!
//! // Connect to camera
//! let camera = ViscaClient::connect_udp_async("192.168.1.100:5678").await?;
//!
//! // Send multiple commands concurrently
//! let pan_tilt_cmd = PanTiltCommand::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10)?,
//!     tilt_speed: TiltSpeed::new(0x10)?,
//! };
//! let pan_tilt = camera.send_async(&pan_tilt_cmd);
//! let zoom = camera.send_async(&ZoomCommand::TeleStandard);
//!
//! // Both commands execute concurrently (respecting the 2-socket limit)
//! let (pan_result, zoom_result): (Result<ViscaResponse, _>, Result<ViscaResponse, _>) = tokio::join!(pan_tilt, zoom);
//! # Ok(())
//! # }
//! ```
//!
//! ## API Overview
//!
//! The crate provides several levels of API for different use cases:
//!
//! ### High-Level Client
//! - [`ViscaClient`] - Thread-safe wrapper for concurrent camera control (recommended)
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
//! - [`ViscaCommand`] trait - Implemented by all command types
//! - Command modules in [`command`] - Organized by functionality
//!
//! ### Response Handling
//! - [`ViscaResponse`] - Enum for all response types (ACK, Completion, Inquiry, Error)
//! - [`ViscaInquiryResponse`] - Specific inquiry response variants
//! - [`ViscaError`] - Comprehensive error types for all failure modes
//!
//! ### Async Support (with `async` feature)
//! - `AsyncViscaClient` - High-level async client with automatic socket management
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
//! The recommended way to use this library is through the unified `ViscaClient`:
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # fn main() -> Result<(), grafton_visca::ViscaError> {
//! # use grafton_visca::{ViscaClient};
//! # use grafton_visca::command::{PowerCommand, power::Power};
//! let client = ViscaClient::connect_udp("192.168.1.100:5678")?;
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
/// Timeout configuration and management
pub mod timeout;
/// Transport layer implementations for VISCA communication
pub mod transport;

/// Async transport layer implementations
#[cfg(feature = "async-client")]
pub mod async_transport;

/// Reconnecting transport wrapper for handling connection failures
#[cfg(feature = "async-client")]
pub mod reconnecting_transport;

// Private modules
mod error;
mod exposure_ext;
mod focus_ext;
mod image_ext;
mod inquiry_ext;
mod pan_tilt_ext;
mod position_ext;
mod power_ext;
mod preset_ext;
mod session;
mod transport_ext;
mod white_balance_ext;
mod zoom_ext;

#[cfg(feature = "async-client")]
mod async_visca_ext;

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
        response::{parse_visca_response, ViscaResponse},
        ViscaCommand, ViscaInquiryResponse, ViscaResponseType,
    },
    error::{AppError, ViscaError, ViscaResultExt, ViscaRetry},
    session::ViscaSession,
};

// Connection and pooling re-exports
pub use crate::{
    connection::{ConnectionManagement, ConnectionStats, ConnectionStatsSnapshot},
    connection_pool::{
        CameraInfo, ConnectionType, PoolConfig, PooledCameraStats, ViscaConnectionPool,
    },
    timeout::{CommandCategory, TimeoutConfig, TimeoutConfigBuilder},
};

// Extension trait re-exports
pub use crate::{
    exposure_ext::{ExposurePreset, ViscaExposureExt},
    focus_ext::ViscaFocusExt,
    image_ext::{ImagePreset, ViscaImageExt},
    inquiry_ext::{
        CameraPosition, CameraState, ExposureState, ImageState, OpticsState, ViscaInquiryExt,
        WhiteBalanceState,
    },
    pan_tilt_ext::ViscaPanTiltExt,
    position_ext::ViscaPositionExt,
    power_ext::ViscaPowerExt,
    preset_ext::ViscaPresetExt,
    transport_ext::ViscaTransportExt,
    white_balance_ext::{ViscaWhiteBalanceExt, WhiteBalancePreset},
    zoom_ext::ViscaZoomExt,
};

// Unified client re-exports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::{
    ptz_builder::PtzBuilder,
    unified_client::{ViscaClient, ViscaClientPtzExt},
};

// Async-specific re-exports
#[cfg(feature = "async-client")]
pub use crate::{
    async_transport::TransportFuture,
    async_visca_ext::{AsyncViscaExt, PanScanDirection},
    connection::AsyncConnectionManagement,
    connection_pool::AsyncViscaConnectionPool,
    reconnecting_transport::{
        ConnectionEvent, ConnectionEventCallback, ReconnectingTransport, ReconnectionConfig,
    },
};

/// Core trait for types that can send and receive VISCA commands.
///
/// This trait provides the minimal interface needed for the extension traits.
/// It is implemented by `ViscaClient` and provides the foundation for all
/// high-level camera control operations.
pub trait ViscaDevice {
    /// Send a command and wait for the response.
    fn execute_command(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError>;
}
