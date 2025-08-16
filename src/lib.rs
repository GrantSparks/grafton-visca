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
//! use grafton_visca::{CameraBuilder, Error, prelude::blocking::*};
//!
//! fn main() -> Result<(), Error> {
//!     // Create camera using the builder pattern
//!     let camera = CameraBuilder::tcp("192.168.0.110:52381")
//!         .profile::<PTZOpticsG2>()
//!         .build()?;
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
//! use grafton_visca::{CameraBuilder, Error, prelude::r#async::*};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Create camera using the builder pattern
//!     let camera = CameraBuilder::tokio_tcp("192.168.0.110:52381")
//!         .profile::<PTZOpticsG2>()
//!         .build()
//!         .await?;
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
//! use grafton_visca::prelude::blocking::*;
//!
//! // This function only accepts cameras with ND filter support
//! fn adjust_nd_filter<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
//! where
//!     P: Profile + NDFilter,
//!     T: Transport + Send + Sync,
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
//! The library provides transport traits that you can implement for any communication method:
//!
//! ```ignore
//! use grafton_visca::{transport::BlockingTransport, Error};
//!
//! struct MyTransport {
//!     // Your transport state
//! }
//!
//! impl BlockingTransport for MyTransport {
//!     fn send_blocking(&self, data: &[u8]) -> Result<(), Error> {
//!         // Send data over your transport
//!         Ok(())
//!     }
//!     
//!     fn recv_blocking(&self) -> Result<Vec<u8>, Error> {
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
//! The library provides runtime-agnostic async support, allowing you to use ANY async runtime
//! (tokio, async-std, smol, etc.) or even create your own.
//!
//! ### Feature Flags
//!
//! - `async` - Enables async support without any specific runtime. You must provide your own runtime.
//! - `rt-tokio` - Enables async with built-in tokio runtime support (implies `async`).
//!
//! ### ⚠️ Important: Runtime Requirements for Async
//!
//! **The async API REQUIRES a runtime to be configured.** Without a runtime, ALL async operations
//! will fail with: `Error::InvalidState("No runtime configured for async operations")`.
//!
//! The runtime is essential for:
//! - **Timeout handling** - All camera commands have configurable timeouts
//! - **Power sequences** - Power on/off operations require delays
//! - **Movement detection** - Polling for pan/tilt/zoom completion
//! - **Background tasks** - Socket manager for concurrent operations
//!
//! ### Runtime Requirements for Async
//!
//! You have two options for configuring a runtime:
//!
//! #### Option 1: Use the built-in tokio runtime support (Easiest)
//!
//! Enable the `rt-tokio` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "*", features = ["rt-tokio"] }
//! ```
//!
//! Then use `CameraBuilder` with tokio support:
//!
//! ```ignore
//! use grafton_visca::{CameraBuilder, prelude::r#async::*};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // The tokio_tcp() method automatically configures the runtime
//!     let camera = CameraBuilder::tokio_tcp("192.168.0.110:52381")
//!         .profile::<PTZOpticsG2>()
//!         .build()
//!         .await?;
//!     
//!     // All async operations will work
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     Ok(())
//! }
//! ```
//!
//! #### Option 2: Provide your own runtime (Advanced)
//!
//! For complete runtime independence, use the unified Executor trait:
//!
//! ```ignore
//! use grafton_visca::{
//!     Camera, CameraBuilder, Executor,
//!     prelude::r#async::*,
//! };
//! use std::{pin::Pin, time::Duration, future::Future};
//!
//! // Example: Custom executor implementation for async-std
//! #[derive(Debug, Clone)]
//! struct AsyncStdExecutor;
//!
//! impl Executor for AsyncStdExecutor {
//!     type Join<T> = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
//!     where T: Send + 'static;
//!
//!     fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
//!     where
//!         F: Future + Send + 'static,
//!         F::Output: Send + 'static,
//!     {
//!         // Implementation using async-std
//!         // ...
//!     }
//!
//!     fn block_on<F: Future>(&self, fut: F) -> F::Output {
//!         async_std::task::block_on(fut)
//!     }
//!
//!     fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
//!         Box::pin(async_std::task::sleep(duration))
//!     }
//!
//!     fn timeout<'a, F, T>(
//!         &'a self,
//!         duration: Duration,
//!         fut: F,
//!     ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
//!     where
//!         F: Future<Output = T> + Send + 'a,
//!         T: Send + 'a,
//!     {
//!         // Implementation using async-std timeout
//!         // ...
//!     }
//! }
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Error> {
//!     // Create camera with custom executor
//!     let executor = AsyncStdExecutor;
//!     let camera = CameraBuilder::with_executor(executor)
//!         .build_async::<PTZOpticsG2, _>(transport)?;
//!
//!     // All async operations now use async-std
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Common Runtime Errors and Solutions
//!
//! #### Error: `InvalidState("No runtime configured for async operations")`
//! **Cause:** You're using async mode but haven't configured a runtime.
//! **Solution:** Either:
//! - Enable `rt-tokio` feature and use `CameraBuilder::tokio_tcp()`
//! - Call `.with_runtime()` on your camera builder with a custom runtime
//!
//! #### Error: `InvalidState("Operation requires runtime for timeout handling")`
//! **Cause:** The operation needs timeout support but no runtime is available.
//! **Solution:** Same as above - configure a runtime.
//!
//! #### Error: Socket manager initialization issues
//! **Cause:** The socket manager requires a runtime to spawn background tasks.
//! **Solution:** Ensure your runtime's `Spawner` implementation is working correctly.
//!
//! ### Blocking vs Async Mode
//!
//! The library enforces a clear separation between blocking and async modes:
//!
//! - **Blocking mode** (default): No runtime needed, uses synchronous I/O
//! - **Async mode** (`async` feature): REQUIRES runtime configuration
//!
//! You cannot use both modes simultaneously - choose one at compile time via features.
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
//!     Err(e) => println!("Other error: {e}"),
//! }
//! ```

/// Camera profile system for type-safe, model-specific control
pub mod camera;

/// Camera ID type for VISCA protocol addressing.
pub mod camera_id;

/// Capability traits for camera feature composition
pub mod capabilities;

/// Command definitions for VISCA protocol
///
/// This module is public for extensibility, allowing users to create custom commands.
/// Most users should use the high-level camera API instead.
pub mod command;

/// Error types
mod error;

/// Transport layer for implementing custom transports
pub mod transport;

/// Protocol encoding and decoding utilities
pub mod protocol;

/// VISCA runtime with flume-based scheduling
pub mod runtime;

/// Constants for VISCA protocol including default ports
pub mod constants;

pub(crate) mod macros;

/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub mod units;

pub mod timeout;

#[cfg(feature = "async")]
pub(crate) mod executor_unified;

pub mod prelude;

// Testing utilities (available with test-utils feature for deterministic testing)
#[cfg(any(feature = "rt-tokio", feature = "test-utils"))]
#[doc(hidden)]
pub mod testing;

pub use grafton_visca_macros::{InquiryCommand, ViscaEncode, ViscaEnum, ViscaValue};

#[cfg(feature = "async")]
pub use camera::methods::{
    focus::FocusOps,
    inquiry::{InquiryOps, PanTiltInquiryOps},
    pan_tilt::PanTiltOps,
    power::PowerOps,
    presets::PresetsOps,
    zoom::ZoomOps,
};
pub use camera::methods::{
    focus::FocusOpsBlocking,
    inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking},
    pan_tilt::PanTiltOpsBlocking,
    power::PowerOpsBlocking,
    presets::PresetsOpsBlocking,
    zoom::ZoomOpsBlocking,
};
pub use camera::{Camera, CameraBuilder};
pub use camera_id::CameraId;
pub use command::{
    exposure::ExposureMode,
    focus::{AutoFocusSensitivity, FocusMode},
    nd_filter::NDFilterMode,
    pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    preset::PresetNumber,
    resolution::{PictureEffectMode, ResolutionMode},
    system::{MotionSyncMode, MotionSyncSpeed},
    white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
};
pub use error::{Error, Result};
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub use executor_unified::TokioExecutor;
#[cfg(feature = "async")]
pub use executor_unified::{ExecError, Executor};

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};
}
