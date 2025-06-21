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
//! Transport implementations are provided as examples. You can either:
//! 1. Copy the transport implementations from the examples directory
//! 2. Implement your own transport based on the Transport trait
//!
//! ```ignore
//! use grafton_visca::{
//!     Camera,
//!     camera::{profiles::PTZOpticsG2, units::Degrees},
//!     transport::blocking::create,
//! };
//!
//! // Create blocking transport - no async runtime needed!
//! let transport = create::udp("192.168.1.100:52381")?;
//! let mut camera = Camera::<PTZOpticsG2>::new(transport);
//!
//! // Power on and move to home position
//! camera.power_on()?;
//! camera.home()?;
//!
//! // Move to specific position (automatic degree conversion)
//! camera.set_position(Degrees(45.0), Degrees(-15.0))?;
//!
//! // Control zoom
//! camera.zoom_in()?;
//! camera.zoom_stop()?;
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
//! The library provides a `Transport` trait that you can implement for any communication method:
//!
//! ```ignore
//! use grafton_visca::{Transport, TransportFuture, Command, Response, Error};
//!
//! struct MyTransport {
//!     // Your transport state
//! }
//!
//! impl Transport for MyTransport {
//!     fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
//!         Box::pin(async move {
//!             // Send command bytes
//!             let bytes = command.to_bytes()?;
//!             // ... send bytes ...
//!             // ... receive response ...
//!             Ok(Response::Completion)
//!         })
//!     }
//! }
//! ```
//!
//! Example transport implementations are provided in the `examples/` directory:
//! - `tcp_transport.rs` - TCP/IP transport with session management
//! - `udp_transport.rs` - UDP/IP transport
//! - `serial_transport.rs` - Serial port transport example
//! - `custom_transport_example.rs` - Mock and wrapper transports
//!
//! ## Async Support (Runtime-Agnostic)
//!
//! The library provides optional async support that works with ANY async runtime, not just tokio:
//!
//! ```ignore
//! // Enable async feature in Cargo.toml:
//! // grafton-visca = { version = "0.4", features = ["async"] }
//!
//! use grafton_visca::transport::{AsyncTransport, AsyncTransportAdapter};
//! use std::future::Future;
//! use std::pin::Pin;
//!
//! // Implement AsyncTransport for your runtime's transport type
//! struct MyAsyncTransport { /* ... */ }
//!
//! impl AsyncTransport for MyAsyncTransport {
//!     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
//!     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
//!
//!     fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
//!         Box::pin(async move {
//!             // Your async send implementation
//!             Ok(())
//!         })
//!     }
//!
//!     fn receive(&mut self) -> Self::ReceiveFuture<'_> {
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

pub mod timeout; // Public for use in macros

#[cfg(feature = "async")]
mod sync_primitives;

// Core re-exports for Camera<P> API
pub use camera::{
    Camera, CameraProfile, CustomProfile, CustomProfileBuilder, CustomProfileTypedBuilder,
};
pub use command::{Command, InquiryResponse, Response};

// Re-export unit types from camera module
pub use camera::units::{Degrees, Normalized, ViscaUnits};

/// Camera profiles for common models
pub mod profiles {
    pub use crate::camera::profiles::{GenericVisca, PTZOptics30X, PTZOpticsG2, SonyEVID70};
}
