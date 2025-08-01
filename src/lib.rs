//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.

// Lints configuration
#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications
)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo
)]
#![allow(async_fn_in_trait)]

//! ## What is VISCA?
//!
//! VISCA (Video System Control Architecture) is a protocol developed by Sony for controlling PTZ cameras
//! commonly used in robotics, broadcasting, video conferencing, and surveillance applications. This crate
//! implements VISCA over IP, allowing you to control networked PTZ cameras from Rust applications.
//!
//! ## Features
//!
//! - **Type-Safe Camera Profiles**: Compile-time validation with camera-specific profiles
//! - **Complete Command Coverage**: Full support for PTZOptics G2 and other VISCA cameras
//! - **Profile-Aware Conversions**: Automatic unit conversions based on camera model
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Transport Abstraction**: Implement your own transport (TCP, UDP, serial, etc.)
//! - **Builder Patterns**: Create custom camera profiles for any VISCA camera
//! - **Runtime-Agnostic Async**: Optional async support works with ANY runtime (tokio, async-std, smol, etc.)
//!
//! ## Quick Start
//!
//! ### Camera - No Generics Required!
//! ```ignore
//! use grafton_visca::{Error, prelude::*};
//! use grafton_visca::transport::blocking::Tcp;
//!
//! fn main() -> Result<(), Error> {
//!     // Create camera with default profile (GenericVisca)
//!     let transport = Tcp::connect("192.168.1.100:52381")?;
//!     let camera = GenericViscaCam::new(transport);
//!
//!     // Or create a specific camera model
//!     let transport = Tcp::connect("192.168.1.100:52381")?;
//!     let camera = PTZOpticsG2Cam::new(transport);
//!
//!     // Camera model is known at compile time
//!     println!("Using PTZOptics G2 camera");
//!
//!     // Send commands with clean API
//!     camera.power_on()?;
//!     camera.zoom_in()?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Async Example
//! ```ignore
//! use grafton_visca::{Error, r#async::prelude::*};
//! use grafton_visca::transport::tokio::Tcp;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Create camera with specific profile
//!     let transport = Tcp::connect("192.168.1.100:52381").await?;
//!     let camera = PTZOpticsG2Cam::new(transport);
//!
//!     // Same API, just with .await
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Camera Profiles
//!
//! The library includes pre-defined profiles with type aliases:
//! - `PTZOpticsG2Cam<T>` - PTZOptics G2 series cameras  
//! - `SonyFR7Cam<T>` - Sony FR7 cameras with ND filter support
//! - `GenericViscaCam<T>` - Generic VISCA-compatible cameras (conservative feature set)
//!
//! ### Compile-Time Type Safety
//!
//! The generic API ensures type safety at compile time:
//!
//! ```ignore
//! use grafton_visca::prelude::*;
//!
//! // This function only accepts cameras with ND filter support
//! fn adjust_nd_filter<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
//! where
//!     P: Profile + NDFilter,
//!     T: UnifiedTransport,
//! {
//!     camera.set_nd_filter_mode(NDFilterMode::Clear)
//! }
//!
//! // This would compile for SonyFR7 but not for PTZOpticsG2
//! let sony = SonyFR7Cam::new(transport);
//! adjust_nd_filter(&sony)?; // OK - Sony FR7 has ND filter
//!
//! let g2 = PTZOpticsG2Cam::new(transport);
//! // adjust_nd_filter(&g2)?; // Compile error - G2 doesn't have ND filter
//! ```
//!
//! ## Transport Implementation
//!
//! The library provides a `ViscaProtocol` struct that you can implement for any communication method:
//!
//! ```ignore
//! use grafton_visca::{Command, Response, Error};
//!
//! struct MyTransport {
//!     // Your transport state
//! }
//!
//! impl BlockingTransport for MyTransport {
//!     fn send(&mut self, data: &[u8]) -> Result<(), Error> {
//!         // Send data over your transport
//!         Ok(())
//!     }
//!     
//!     fn receive(&mut self) -> Result<Vec<u8>, Error> {
//!         // Receive response from your transport
//!         Ok(vec![])
//!     }
//! }
//! ```
//!
//! Example transport implementations are provided in the `examples/` directory:
//! - `tcp_transport.rs` - TCP/IP transport with session management
//! - `udp_transport.rs` - UDP/IP transport
//! - `custom_transport_example.rs` - Mock and wrapper transports
//!
//! ## Async Support
//!
//! The library provides optional async support with a runtime-agnostic design:
//!
//! ### Feature Flags
//!
//! - `async` - Enables async support without any specific runtime. You bring your own runtime.
//! - `tokio` - Enables async with built-in tokio implementations (implies `async`).
//!
//! ### With Tokio (built-in implementations)
//!
//! When using the `tokio` feature, the library provides ready-to-use TCP and UDP transports:
//!
//! ```ignore
//! # // Cargo.toml: features = ["tokio"]
//! use grafton_visca::{Camera, CameraModel};
//! use grafton_visca::transport::tokio::Tcp;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let transport = Tcp::connect("192.168.1.100:5678").await?;
//!     let camera = PTZOpticsG2Cam::new(transport);
//!
//!     camera.power_on().await?;
//!     camera.pan_tilt_home().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Runtime-Agnostic Async (bring your own runtime)
//!
//! When using only the `async` feature, the library provides the async traits and protocol
//! handling, but you must provide your own transport implementation and handle timeouts
//! using your runtime's facilities:
//!
//! ```ignore
//! # // Cargo.toml: features = ["async"]
//! use grafton_visca::{Camera, CameraModel};
//! use grafton_visca::transport::Transport;
//! use async_std::net::TcpStream; // or any runtime's stream
//! use async_std::io::{ReadExt, WriteExt};
//! use async_std::future::timeout;
//! use std::time::Duration;
//!
//! // Implement Transport for your runtime's types
//! struct AsyncStdTcp {
//!     stream: TcpStream,
//! }
//!
//! impl Transport for AsyncStdTcp {
//!     type Error = std::io::Error;
//!     type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
//!     type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>;
//!
//!     fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {
//!         Box::pin(async move {
//!             self.stream.write_all(bytes).await?;
//!             self.stream.flush().await
//!         })
//!     }
//!
//!     fn recv<'a>(&'a self) -> Self::RecvFut<'a> {
//!         Box::pin(async move {
//!             // Read VISCA frame (implementation details omitted)
//!             let mut buffer = vec![0u8; 1024];
//!             let n = self.stream.read(&mut buffer).await?;
//!             Ok(bytes::Bytes::from(buffer[..n].to_vec()))
//!         })
//!     }
//! }
//!
//! // Use with your runtime's timeout facilities
//! async fn send_with_timeout(camera: &Camera, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
//!     timeout(duration, camera.power_on()).await??;
//!     Ok(())
//! }
//! ```
//!
//! ### Important Notes on Timeouts
//!
//! When using the `async` feature without `tokio`, the library cannot provide built-in timeout
//! functionality. You must wrap operations with your runtime's timeout mechanism:
//!
//! - **async-std**: Use `async_std::future::timeout`
//! - **smol**: Use `smol::future::or` with `smol::Timer`
//! - **futures-timer**: Use `futures_timer::Delay`
//!
//! The library will log when timeouts are requested but not available. This is not an error,
//! just a reminder to handle timeouts at the application level.
//!
//! ## Supported Commands
//!
//! ### Camera Movement
//! - Pan/Tilt/Zoom control with absolute and relative positioning
//! - Variable speed control for smooth movements
//! - Home position and preset management
//!
//! ### Exposure & Color
//! - Exposure modes: Auto, Manual, Shutter Priority, Iris Priority, Bright
//! - White balance modes including manual color temperature
//! - Color adjustments: saturation, hue, RGB gain tuning
//!
//! ### Image Control
//! - Focus control with auto/manual modes
//! - Sharpness, brightness, and contrast adjustment
//! - Noise reduction (2D and 3D)
//! - Image flip and other effects
//!
//! ## Position Units
//!
//! The Camera API supports multiple position unit types with automatic conversion:
//!
//! ```ignore
//! // Work in degrees (recommended)
//! camera.set_position(Degrees(45.0), Degrees(-15.0))?;
//!
//! // Stop all movement
//! camera.stop()?;
//!
//! // Move to home position
//! camera.home()?;
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error types for all VISCA error conditions:
//!
//! ```ignore
//! match camera.set_position(Degrees(180.0), Degrees(0.0)) {
//!     Ok(_) => println!("Position set successfully"),
//!     Err(Error::SyntaxError) => println!("Position out of range"),
//!     Err(Error::CommandNotExecutable) => println!("Camera busy or powered off"),
//!     Err(e) => println!("Other error: {}", e),
//! }
//! ```

/// Camera profile system for type-safe, model-specific control
pub mod camera;

/// Camera ID type for VISCA protocol addressing.
pub mod camera_id;

/// Capability traits for camera feature composition
pub mod capabilities;

pub(crate) mod command;

/// Error types
mod error;
pub use error::{Error, Result};

/// Transport layer (most users won't need direct access)
pub mod transport;

#[cfg(feature = "async")]
mod channels;
mod constants;
mod macros;
/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub mod units;

pub mod timeout;

#[cfg(feature = "async")]
pub(crate) mod socket_manager;

pub mod executor;

pub mod blocking;

#[cfg(feature = "async")]
pub mod r#async;

pub mod prelude;
pub use camera::Camera;
pub use camera_id::CameraId;

pub use types::{ExposureCompensationLevel, FStop, IntoIrisLevel, NDIQuality};
pub use units::{
    Degrees, Fraction, Kelvin, Magnification, Normalized, Percentage, Raw, ViscaUnits,
};

pub use command::{
    exposure::ExposureMode,
    focus::{AutoFocusSensitivity, FocusMode, FocusRange, FocusZone},
    image_adjustment::{BlackWhiteMode, NrMode, NrSpeed, SharpnessMode},
    nd_filter::NDFilterMode,
    pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    preset::PresetNumber,
    resolution::{NDFilterPosition, PictureEffectMode, ResolutionMode},
    system::{MotionSyncMode, MotionSyncSpeed},
    white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
};

pub use grafton_visca_macros::ViscaValue;

pub use capabilities::ProfileMetadata;

pub use capabilities::CameraFeature;

pub use capabilities::{
    HasAutoExposure, HasAutoFocus, HasBacklightCompensation, HasColorTemperature, HasExposure,
    HasExposureCompensation, HasFocus, HasHue, HasImageProcessing, HasLuminance, HasMenuControl,
    HasMotionSync, HasNDFilter, HasOnePushFocus, HasOnePushWhiteBalance, HasPanTilt, HasPower,
    HasPresets, HasRGBGain, HasVariableSpeed, HasWDR, HasWhiteBalance, HasZoom,
};

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};
}
