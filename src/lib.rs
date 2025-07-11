//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.

// Lints configuration
#![warn(
    clippy::all,
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
//! ### Blocking API
//! ```ignore
//! use grafton_visca::{
//!     Camera,
//!     profiles::PTZOpticsG2,
//!     units::Degrees,
//!     transport::blocking::create,
//! };
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create blocking transport - no async runtime needed!
//!     let transport = create::udp("192.168.1.100:52381")?;
//!     let mut camera = Camera::<PTZOpticsG2, _>::new(transport);
//!
//!     // Power on and move to home position
//!     camera.power_on()?;
//!     camera.home()?;
//!
//!     // Move to specific position (automatic degree conversion)
//!     camera.set_position(Degrees(45.0), Degrees(-15.0))?;
//!
//!     // Control zoom
//!     camera.zoom_in()?;
//!     camera.zoom_stop()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Camera Profiles
//!
//! The library includes pre-defined profiles for common cameras:
//! - `PTZOpticsG2` - PTZOptics G2 series cameras
//! - `PTZOptics30X` - PTZOptics 30X optical zoom cameras
//! - `SonyEVID70` - Sony EVI-D70 cameras
//! - `GenericVisca` - Generic VISCA-compatible cameras
//!
//! ### Custom Camera Profiles
//!
//! Create profiles for cameras not included in the library:
//!
//! ```ignore
//! use grafton_visca::{Camera, camera::CustomProfileBuilder};
//! use grafton_visca::transport::blocking::create;
//!
//! let profile = CustomProfileBuilder::new("My Custom Camera")
//!     .pan_range(-170..=170)
//!     .tilt_range(-90..=90)
//!     .zoom_range(0x0000..=0xA000)
//!     .build();
//!
//! // Create blocking transport
//! let transport = create::udp("192.168.1.100:52381")?;
//! let mut camera = Camera::with_profile(transport, profile);
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
//! ### With Tokio (built-in implementations)
//! ```ignore
//! use grafton_visca::{Camera, profiles::PTZOpticsG2, transport::create};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let transport = create::tcp("192.168.1.100:5678").await?;
//!     let camera = Camera::<PTZOpticsG2, _>::new(transport);
//!
//!     // All methods are naturally concurrent with &self
//!     camera.power_on().await?;
//!     camera.home().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Custom Runtime Support
//! Implement `AsyncTransport` for any async runtime:
//!
//! ```ignore
//! use grafton_visca::transport::AsyncTransport;
//! use grafton_visca::Error;
//! use std::future::Future;
//! use std::pin::Pin;
//!
//! # struct MyRuntimeStream;
//! # impl MyRuntimeStream {
//! #     async fn write_all(&mut self, _: &[u8]) -> Result<(), std::io::Error> { Ok(()) }
//! #     async fn flush(&mut self) -> Result<(), std::io::Error> { Ok(()) }
//! #     async fn read(&mut self, _: &mut [u8]) -> Result<usize, std::io::Error> { Ok(0) }
//! # }
//! #[derive(Debug)]
//! struct MyTransport {
//!     stream: std::sync::Arc<std::sync::Mutex<MyRuntimeStream>>
//! }
//!
//! impl AsyncTransport for MyTransport {
//!     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
//!     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
//!
//!     fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
//!         Box::pin(async move {
//!             // Your async send implementation
//!             Ok(())
//!         })
//!     }
//!
//!     fn receive(&self) -> Self::ReceiveFuture<'_> {
//!         Box::pin(async move {
//!             // Your async receive implementation
//!             Ok(vec![])
//!         })
//!     }
//! }
//! ```
//!
//! See the examples directory for complete implementations with async-std, smol, and other runtimes.
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

// Public modules - only what's needed for Camera<P> API
/// Camera profile system for type-safe, model-specific control
pub mod camera;

/// Capability traits for composable camera features
pub mod capabilities;

/// VISCA command definitions
pub mod command;

/// Error types
mod error;
pub use error::{Error, Result};

/// Transport layer (most users won't need direct access)
pub mod transport;

// Internal modules - not part of public API
mod constants;
mod macros;
/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub mod units;

pub mod timeout; // Public for use in macros

// Minimal blocking executor
pub mod blocking;

// Core re-exports
pub use camera::async_facade::CameraAsync as Camera;
pub use camera::blocking_facade::CameraBlocking;
pub use command::{Command, InquiryResponse, Response};

// Re-export unit types for convenience
pub use units::{
    Degrees, Fraction, Kelvin, Magnification, Normalized, Percentage, Raw, ViscaUnits,
};
// Re-export FStop from types
pub use types::{FStop, IntoIrisLevel};

// Re-export procedural macros
pub use grafton_visca_macros::{
    dual_native_inquiry, visca_bounded_command, visca_camera_method, visca_command_variants,
    visca_fallible_method, visca_inquiry, visca_method, visca_method_custom, visca_method_generic,
    visca_mock_transport, visca_position_command, visca_speed_command, visca_test_suite,
    InquiryCommand, ViscaValue,
};

// Macros are already exported with #[macro_export] so we don't need to re-export them

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};
}
