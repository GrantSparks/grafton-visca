//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control Ptz cameras.

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
//! VISCA (Video System Control Architecture) is a protocol developed by Sony for controlling Ptz cameras
//! commonly used in robotics, broadcasting, video conferencing, and surveillance applications. This crate
//! implements VISCA over IP, allowing you to control networked Ptz cameras from Rust applications.
//!
//! ## Features
//!
//! - **Unified API Architecture**: Single consistent interface with 17 traits covering 130+ methods
//! - **Type-Safe Camera Profiles**: Compile-time validation with camera-specific profiles
//! - **Feature-Gated Methods**: Choose blocking or async at compile time with zero runtime overhead
//! - **Multi-Runtime Support**: Tokio, async-std, smol can coexist with priority-based selection
//! - **Complete Command Coverage**: Full VISCA protocol support across all camera types
//! - **Profile-Aware Conversions**: Automatic unit conversions based on camera model
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Transport Abstraction**: TCP, UDP, Serial, and custom transport implementations
//! - **Builder Patterns**: Flexible camera and transport configuration
//! - **Unified Error Handling**: Consistent error mapping across all transport types
//! - **Configurable Timeouts**: Per-category timeout configuration for different command types
//! - **Command Cancellation**: Cancel specific commands or entire socket operations
//! - **Async Completion Tracking**: Wait for camera movements to complete with await methods
//!
//! ## Quick Start
//!
//! ### Blocking Example
//! ```ignore
//! use grafton_visca::{
//!     Camera, Error,
//!     camera::profiles::PtzOpticsG2,
//!     transport::builder::TransportBuilder,
//!     // Import unified traits that work for both blocking and async
//!     PowerControl, ZoomControl,
//! };
//!
//! fn main() -> Result<(), Error> {
//!     // Create camera using unified API
//!     let transport = TransportBuilder::tcp()
//!         .address("192.168.0.110:5678")
//!         .build()?;
//!     let mut camera = Camera::<PtzOpticsG2, _>::new(transport);
//!
//!     // Send commands with unified API - same traits work for async mode
//!     camera.power_on()?;
//!     camera.zoom_tele_std()?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Async Example with Multi-Runtime Support
//! ```ignore
//! use grafton_visca::{
//!     CameraBuilder, Error,
//!     camera::profiles::PtzOpticsG2,
//!     transport::Transport,
//!     // Same unified traits work for async mode too
//!     PowerControl, ZoomControl,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Create camera with uniform Transport API (runtime auto-selected)
//!     let transport = Transport::tcp()
//!         .address("192.168.0.110:5678")
//!         .connect()
//!         .await?;
//!     let camera = CameraBuilder::tokio()?
//!         .build_async::<PtzOpticsG2, _>(transport)
//!         .await?;
//!
//!     // Same unified API, just add .await - no separate async traits needed
//!     camera.power_on().await?;
//!     camera.zoom_tele_std().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Runtime Coexistence Example
//! ```ignore
//! // Multiple runtimes can coexist! Priority: tokio → async-std → smol
//! [dependencies]
//! grafton-visca = { version = "*", features = ["rt-tokio", "rt-async-std"] }
//!
//! use grafton_visca::{
//!     CameraBuilder, Error,
//!     camera::profiles::PtzOpticsG2,
//!     transport::Transport,
//!     PowerControl, ZoomControl,
//! };
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Error> {
//!     // Same Transport API - runtime auto-selected based on priority
//!     let transport = Transport::tcp()
//!         .address("192.168.0.110:5678")
//!         .connect()  // Uses tokio if available, async-std otherwise
//!         .await?;
//!     let camera = CameraBuilder::async_std()?
//!         .build_async::<PtzOpticsG2, _>(transport)
//!         .await?;
//!
//!     // Same unified API across all runtimes
//!     camera.power_on().await?;
//!     camera.zoom_tele_std().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Unified Trait Usage Example
//! ```ignore
//! use grafton_visca::{
//!     CameraBuilder, Error,
//!     camera::profiles::PtzOpticsG2,
//!     transport::Transport,
//!     PowerControl, ZoomControl, PanTiltControl,
//! };
//!
//! fn main() -> Result<(), Error> {
//!     smol::block_on(async {
//!         // Same unified Transport API works across all runtimes  
//!         let transport = Transport::tcp()
//!             .address("192.168.0.110:5678")
//!             .connect()  // Runtime auto-selected based on enabled features
//!             .await?;
//!         let camera = CameraBuilder::smol()?
//!             .build_async::<PtzOpticsG2, _>(transport)
//!             .await?;
//!
//!         // All 17 unified traits work consistently across runtimes
//!         camera.power_on().await?;
//!         camera.pan_tilt_home().await?;
//!         camera.zoom_tele_std().await?;
//!         
//!         Ok(())
//!     })
//! }
//! ```
//!
//! ## Camera Profiles
//!
//! The library includes pre-defined profiles with type aliases:
//! - `PtzOpticsG2Cam<T>` - PtzOptics G2 series cameras  
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
//!     P: Profile + NdFilter,
//!     T: Transport + Send + Sync,
//! {
//!     camera.set_nd_filter_mode(NdFilterMode::Clear)
//! }
//!
//! // This would compile for SonyFR7 but not for PtzOpticsG2
//! let sony = SonyFR7Cam::new(transport);
//! adjust_nd_filter(&sony)?; // OK - Sony FR7 has ND filter
//!
//! let g2 = PtzOpticsG2Cam::new(transport);
//! // adjust_nd_filter(&g2)?; // Compile error - G2 doesn't have ND filter
//! ```
//!
//! ## Transport Implementation
//!
//! The library provides transport traits that you can implement for any communication method:
//!
//! ```ignore
//! use grafton_visca::{transport::SyncTransport, Error};
//!
//! struct MyTransport {
//!     // Your transport state
//! }
//!
//! impl SyncTransport for MyTransport {
//!     fn send(&self, data: &[u8]) -> Result<(), Error> {
//!         // Send data over your transport
//!         Ok(())
//!     }
//!     
//!     fn recv(&self) -> Result<Vec<u8>, Error> {
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
//! - `rt-tokio` - Enables async with built-in Tokio runtime support (implies `async`).
//! - `rt-async-std` - Enables async with built-in async-std runtime support (implies `async`).
//! - `rt-smol` - Enables async with built-in smol runtime support (implies `async`).
//! - `test-utils` - Testing utilities including ScriptedTransport and DeterministicExecutor (not for production).
//!
//! **Multiple Runtime Support**: As of version 0.7.0, runtime features can be enabled simultaneously.
//! This allows libraries to support multiple runtime ecosystems without forcing users to choose.
//! Use explicit executor selection (`CameraBuilder::tokio()`, etc.) when multiple runtimes are available.
//!
//! ### Send Future Guarantees
//!
//! All public async traits in this crate guarantee that their returned futures are `Send`.
//! This is enforced through explicit `+ Send` bounds in trait signatures using
//! return-position impl trait in traits (RPITIT).
//!
//! This guarantee ensures spawn-safety across all async runtimes and prevents
//! subtle `!Send` future errors in multi-threaded executors.
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
//! You have multiple options for configuring a runtime:
//!
//! #### Option 1: Use built-in runtime support (Easiest)
//!
//! Choose your runtime(s) and enable the corresponding feature(s) in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! # Single runtime:
//! grafton-visca = { version = "*", features = ["rt-tokio"] }
//! grafton-visca = { version = "*", features = ["rt-async-std"] }
//! grafton-visca = { version = "*", features = ["rt-smol"] }
//!
//! # Multiple runtimes (choose executor at construction time):
//! grafton-visca = { version = "*", features = ["rt-tokio", "rt-async-std"] }
//! grafton-visca = { version = "*", features = ["rt-tokio", "rt-smol", "rt-async-std"] }
//! ```
//!
//! Then use the corresponding `CameraBuilder` method:
//!
//! ```ignore
//! // Tokio
//! let camera = CameraBuilder::tokio()?
//!     .build_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // async-std
//! let camera = CameraBuilder::async_std()
//!     .build_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // smol
//! let camera = CameraBuilder::smol()
//!     .build_async::<PtzOpticsG2, _>(transport)
//!     .await?;
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
//!         .build_async::<PtzOpticsG2, _>(transport)?;
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
//! The library provides a clean separation between blocking and async APIs:
//!
//! - **Blocking mode** (default): No async dependencies, uses synchronous I/O
//!   - When no features are enabled, only blocking types are available
//!   - Zero async runtime overhead or dependencies
//!
//! - **Async mode** (`async` feature): Native async implementation
//!   - When `async` feature is enabled, blocking types are NOT exported
//!   - Provides true async I/O without blocking thread pools
//!   - REQUIRES runtime configuration (see Async Support section above)
//!
//! The API surface changes based on your feature selection - you get either blocking
//! OR async types, never both. This ensures a clean, focused API for your use case.
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
//! ## Timeout Configuration
//!
//! Configure timeouts per command category based on your network and camera:
//!
//! ```ignore
//! use grafton_visca::{CameraBuilder, TimeoutConfig};
//! use std::time::Duration;
//!
//! let config = TimeoutConfig::builder()
//!     .ack_timeout(Duration::from_millis(300))
//!     .quick_commands(Duration::from_secs(3))
//!     .movement_commands(Duration::from_secs(20))
//!     .preset_operations(Duration::from_secs(60))
//!     .build();
//!
//! // For async mode (default)
//! let camera = CameraBuilder::tokio()?
//!     .timeout_config(config)
//!     .build_async::<PtzOpticsG2, _>(transport).await?;
//!
//! // For blocking mode (when async feature is disabled)
//! #[cfg(not(feature = "async"))]
//! let camera = CameraBuilder::new()
//!     .timeout_config(config)
//!     .build_blocking::<PtzOpticsG2, _>(transport)?;
//! ```
//!
//! ## Command Cancellation (Async)
//!
//! Cancel specific commands or entire socket operations:
//!
//! ```ignore
//! // Send a command and get its ID for cancellation
//! let (cmd_id, response_future) = camera.send_command_with_id(command).await?;
//!
//! // Cancel the specific command
//! camera.cancel_command(cmd_id).await?;
//!
//! // Or cancel all commands on a socket
//! use grafton_visca::ViscaSocket;
//! camera.cancel_socket(ViscaSocket::S1).await?;
//! ```
//!
//! ## Async Completion Tracking
//!
//! Wait for camera movements to complete:
//!
//! ```ignore
//! // Start a pan/tilt movement
//! camera.pan_tilt_absolute(45.0, 15.0, 10, 10).await?;
//!
//! // Wait for the movement to complete
//! camera.wait_for_completion().await?;
//!
//! // Or wait with a custom timeout
//! use std::time::Duration;
//! camera.wait_for_completion_with_timeout(Duration::from_secs(10)).await?;
//!
//! // Check if the runtime is idle (no pending commands)
//! if camera.is_idle().await? {
//!     println!("All commands completed");
//! }
//!
//! // Wait for all operations to complete (barrier synchronization)
//! camera.wait_for_idle(Duration::from_secs(30)).await?;
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error types for all VISCA error conditions:
//!
//! ```ignore
//! match camera.pan_tilt_absolute(180.0, 0.0, 10, 10).await {
//!     Ok(_) => println!("Position set successfully"),
//!     Err(Error::SyntaxError) => println!("Position out of range"),
//!     Err(Error::CommandNotExecutable) => println!("Camera busy or powered off"),
//!     Err(Error::CommandBufferFull) => {
//!         // This error is automatically retried by the runtime
//!         println!("Camera buffer full, command will retry");
//!     }
//!     Err(e) => println!("Other error: {e}"),
//! }
//! ```

// Multiple async runtimes can now coexist - users choose which executor to use at construction time
// This flexibility allows libraries to support multiple runtime ecosystems simultaneously

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

/// Runtime-specific transport adapters
#[cfg(feature = "async")]
pub mod runtime_adapters;

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

/// Unified VISCA socket type
pub mod visca_socket;

pub mod timeout;

pub mod mode;

#[cfg(feature = "async")]
pub(crate) mod executor;

pub mod prelude;

// Testing utilities (available with test-utils feature for deterministic testing)
#[cfg(any(feature = "rt-tokio", feature = "test-utils"))]
#[doc(hidden)]
pub mod testing;

pub use grafton_visca_macros::{InquiryCommand, ViscaEncode, ViscaEnum, ViscaValue};

// Core types always exported
pub use crate::command::{
    exposure::ExposureMode,
    focus::{AutoFocusSensitivity, FocusMode},
    nd_filter::NdFilterMode,
    pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    preset::PresetNumber,
    resolution::{PictureEffectMode, ResolutionMode},
    system::{MotionSyncMode, MotionSyncSpeed},
    white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
};
pub use crate::error::{Error, Result};
pub use crate::visca_socket::ViscaSocket;
pub use crate::{camera::Camera, camera::CameraBuilder, camera_id::CameraId};

// Re-export the new concrete camera types
#[cfg(not(feature = "async"))]
pub use crate::camera::BlockingCamera;

#[cfg(feature = "async")]
pub use crate::camera::AsyncCamera;

// Export unified camera control traits that work with both blocking and async cameras
pub use crate::camera::controls::{
    color::ColorControl,
    exposure::ExposureControl,
    focus::FocusControl,
    image_processing::ImageProcessingControl,
    inquiry::{InquiryControl, PanTiltInquiryControl},
    menu::{DirectMenuControl, MenuControl},
    nd_filter::NdFilterControl,
    pan_tilt::PanTiltControl,
    power::PowerControl,
    presets::PresetsControl,
    streaming::StreamingControl,
    system::SystemControl,
    tally::TallyControl,
    variable_speed::VariableSpeedControl,
    white_balance::WhiteBalanceControl,
    zoom::ZoomControl,
};
#[cfg(all(feature = "async", feature = "rt-async-std"))]
pub use crate::executor::AsyncStdExecutor;
#[cfg(all(feature = "async", feature = "rt-smol"))]
pub use crate::executor::SmolExecutor;
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub use crate::executor::TokioExecutor;
#[cfg(feature = "async")]
pub use crate::executor::{ExecError, Executor};

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7};
}

// Re-export camera type aliases for convenience
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub use crate::camera::TokioCamera;

#[cfg(all(feature = "async", feature = "rt-async-std"))]
pub use crate::camera::AsyncStdCamera;

#[cfg(all(feature = "async", feature = "rt-smol"))]
pub use crate::camera::SmolCamera;
