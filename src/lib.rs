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
//! - **Multiple Transports**: Both UDP and TCP support with async/await
//! - **Builder Patterns**: Create custom camera profiles for any VISCA camera
//!
//! ## Quick Start
//!
//! ```ignore
//! use grafton_visca::{
//!     Camera,
//!     camera::{profiles::PTZOpticsG2, units::Degrees},
//!     transport::{BlockingAdapter, UdpTransport},
//! };
//!
//! // Create a camera with PTZOpticsG2 profile
//! let udp_transport = UdpTransport::new("192.168.1.100:52381")?;
//! let transport = BlockingAdapter(udp_transport);
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
//! use grafton_visca::transport::{BlockingAdapter, UdpTransport};
//!
//! let profile = CustomProfileBuilder::new("My Custom Camera")
//!     .pan_range(-170..=170)
//!     .tilt_range(-90..=90)
//!     .zoom_range(0x0000..=0xA000)
//!     .build();
//!
//! let udp_transport = UdpTransport::new("192.168.1.100:52381")?;
//! let transport = BlockingAdapter(udp_transport);
//! let mut camera = Camera::with_profile(transport, profile);
//! ```
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
mod connection;
mod constants;
mod macros;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod session;

/// Type definitions and abstractions
pub mod types;

// Multi-camera support
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub mod camera_pool;

#[cfg(feature = "async-client")]
pub use connection::ConnectionStats;

#[cfg(feature = "async-client")]
mod transport_future;

pub mod timeout; // Public for use in macros

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
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
