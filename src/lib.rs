//! # grafton-visca
//!
//! A production-ready Rust implementation of the VISCA over IP protocol for controlling PTZ (Pan-Tilt-Zoom) cameras.
#![warn(missing_docs)]
#![allow(missing_docs)] // Temporary allow for Phase G - will be addressed incrementally
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    rust_2018_idioms
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::redundant_pub_crate,
    clippy::cargo_common_metadata,
    clippy::use_self,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::significant_drop_tightening,
    clippy::wildcard_imports,
    clippy::missing_const_for_fn,
    clippy::derive_partial_eq_without_eq,
    clippy::match_same_arms,
    clippy::too_many_lines,
    clippy::match_wild_err_arm,
    clippy::cast_possible_wrap,
    clippy::items_after_statements,
    clippy::uninlined_format_args,
    clippy::return_self_not_must_use,
    clippy::float_cmp,
    clippy::wildcard_enum_match_arm,
    clippy::single_match_else
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
//! use grafton_visca::{AsyncViscaClient, ViscaResponse};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//! use grafton_visca::command::pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed};
//!
//! // Connect to camera
//! let camera = AsyncViscaClient::connect_udp("192.168.1.100:5678").await?;
//!
//! // Send multiple commands concurrently
//! let pan_tilt_cmd = PanTiltCommand::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10)?,
//!     tilt_speed: TiltSpeed::new(0x10)?,
//! };
//! let pan_tilt = camera.send(&pan_tilt_cmd);
//! let zoom = camera.send(&ZoomCommand::TeleStandard);
//!
//! // Both commands execute concurrently (respecting the 2-socket limit)
//! let (pan_result, zoom_result) = tokio::join!(pan_tilt, zoom);
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

// Standard library imports
// (none currently needed at module level)

// Module declarations
pub mod command;
/// Connection management for VISCA communications.
pub mod connection;
// TODO: Update connection_pool to use new transport system
// pub mod connection_pool;
pub mod constants;
pub mod macros;
#[cfg(feature = "async-client")]
pub mod reconnecting_transport;
pub mod timeout;
pub mod transport;

// TODO: Update camera_detection to use new transport system
// mod camera_detection;
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

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod sync_primitives;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod unified_client;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod ptz_builder;

// TODO: Update async_client to use new transport system
// #[cfg(feature = "async-client")]
// mod async_client;

// TODO: Update async_connection_pool to use new transport system
// #[cfg(feature = "async-client")]
// mod async_connection_pool;

// TODO: Update async_control_ext to use new transport system
// #[cfg(feature = "async-client")]
// mod async_control_ext;

// TODO: Update async_inquiry_ext to use new transport system
// #[cfg(feature = "async-client")]
// mod async_inquiry_ext;

// TODO: Update async_reconnecting_transport to use new transport system
// #[cfg(feature = "async-client")]
// mod async_reconnecting_transport;

// TODO: Update async_tcp_transport to use new transport system
// #[cfg(feature = "async-client")]
// mod async_tcp_transport;

#[cfg(feature = "async-client")]
/// Asynchronous transport implementations for VISCA protocol.
pub mod async_transport;

// TODO: Update async_udp_transport to use new transport system
// #[cfg(feature = "async-client")]
// mod async_udp_transport;

#[cfg(feature = "async-client")]
mod async_visca_ext;

// Public re-exports
pub use crate::{
    // camera_detection::detect_camera_model,
    command::{
        pan_tilt::PanTiltDirection,
        response::{parse_visca_response, ViscaResponse},
        ViscaCommand, ViscaInquiryResponse, ViscaResponseType,
    },
    connection::{ConnectionManagement, ConnectionStats, ConnectionStatsSnapshot},
    // connection_pool::{
    //     CameraInfo, PoolConfig, PooledCameraStats, PooledConnectionGuard, ViscaConnectionPool,
    // },
    error::{AppError, ViscaError, ViscaResultExt, ViscaRetry},
    exposure_ext::ViscaExposureExt,
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
    session::ViscaSession,
    timeout::{CommandCategory, TimeoutConfig, TimeoutConfigBuilder},
    transport_ext::ViscaTransportExt,
    white_balance_ext::{ViscaWhiteBalanceExt, WhiteBalancePreset},
    zoom_ext::ViscaZoomExt,
};

#[cfg(feature = "async-client")]
pub use crate::{
    // TODO: Update async_client and re-export after updating to new transport system
    // async_client::AsyncViscaClient,
    // TODO: Update async connection pool and reconnecting transport
    // async_connection_pool::{
    //     AsyncPoolConfig, AsyncPooledCameraStats, AsyncPooledConnectionGuard,
    //     AsyncViscaConnectionPool, CameraInfo as AsyncCameraInfo,
    // },
    // TODO: Re-export async transports after updating them
    // async_tcp_transport::AsyncTcpTransport,
    async_transport::{AsyncViscaTransport, TransportFuture},
    // async_udp_transport::AsyncUdpTransport,
    async_visca_ext::{AsyncViscaExt, PanScanDirection},
    connection::AsyncConnectionManagement,
    reconnecting_transport::{
        ConnectionEvent, ConnectionEventCallback, ReconnectingTransport, ReconnectionConfig,
    },
};
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::{
    ptz_builder::PtzBuilder,
    unified_client::{ViscaClient, ViscaClientPtzExt},
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
