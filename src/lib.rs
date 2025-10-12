//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.

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
//! - **Unified API Architecture**: Single consistent interface across blocking and async modes
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
//! - **Serialization Support**: Optional serde/schemars integration for all value types
//!
//! ## Serialization Support
//!
//! All public value types support optional serialization through feature-gated `serde` and `schemars` derives:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "*", features = ["serde", "schemars"] }
//! ```
//!
//! With these features enabled, you can serialize/deserialize all value types directly:
//!
//! ```rust
//! # #[cfg(feature = "serde")] {
//! use grafton_visca::types::{PanSpeed, ZoomPosition, SpeedLevel};
//!
//! // Serialize to JSON
//! let speed = PanSpeed::new(12).unwrap();
//! let json = serde_json::to_string(&speed).unwrap();
//! assert_eq!(json, "12");
//!
//! // Deserialize from JSON
//! let speed: PanSpeed = serde_json::from_str("15").unwrap();
//! assert_eq!(speed.value(), 15);
//!
//! // Works with enums too
//! let level = SpeedLevel::Medium;
//! let json = serde_json::to_string(&level).unwrap();
//! assert_eq!(json, "\"medium\"");
//! # }
//! ```
//!
//! ### Configuration Types with Serialization
//!
//! Camera configuration types also support serialization, making it easy to save and load
//! camera setups from configuration files or APIs:
//!
//! ```rust
//! # #[cfg(feature = "serde")] {
//! use grafton_visca::camera::config::TransportOptions;
//! use grafton_visca::camera::profiles::ProfileId;
//!
//! // Serialize camera profile
//! let profile = ProfileId::PtzOpticsG2;
//! let json = serde_json::to_string(&profile).unwrap();
//! assert_eq!(json, "\"ptz-optics-g2\"");
//!
//! // Serialize transport configuration
//! let transport = TransportOptions::Tcp {
//!     address: "192.168.0.110:5678".to_string(),
//! };
//! let json = serde_json::to_string(&transport).unwrap();
//! // Can be loaded from config files, environment variables, etc.
//! # }
//! ```
//!
//! With `schemars` feature, you can also generate JSON schemas for API documentation:
//!
//! ```rust
//! # #[cfg(all(feature = "serde", feature = "schemars"))] {
//! use grafton_visca::types::PanSpeed;
//! use schemars::schema_for;
//!
//! let schema = schema_for!(PanSpeed);
//! // Use schema for API documentation, validation, etc.
//! # }
//! ```
//!
//! ## Model-Aware Parameter Validation
//!
//! The library provides comprehensive parameter validation at multiple levels, ensuring
//! commands are correct before being sent to the camera:
//!
//! ### Type-Safe Parameters with Conservative Defaults
//! All parameter types provide conservative VISCA-compliant ranges by default:
//! ```ignore
//! use grafton_visca::types::{PanSpeed, ZoomPosition, ZoomSpeed};
//!
//! // All range types expose MIN/MAX constants for validation
//! assert_eq!(PanSpeed::MIN.value(), 0);
//! assert_eq!(PanSpeed::MAX.value(), 24);
//!
//! // Validated constructors provide clear error messages
//! let speed = PanSpeed::new(15)?;  // Valid: 0-24
//! match PanSpeed::new(30) {
//!     Err(e) => println!("{}", e), // "PanSpeed must be between 0 and 24"
//!     _ => {}
//! }
//!
//! // Speed types work seamlessly with SpeedLevel enum
//! let zoom = ZoomSpeed::from(SpeedLevel::Fast); // Automatic conversion
//! assert_eq!(zoom.value(), 6); // Fast = 6 for zoom
//! ```
//!
//! ### Model-Specific Validation
//! For precise control, use model-aware constructors that validate against
//! specific camera capabilities:
//! ```ignore
//! use grafton_visca::types::{PanPosition, TiltSpeed};
//! use grafton_visca::constants::CameraVariant;
//!
//! // Model-specific validation for PTZOptics G2
//! let model = CameraVariant::PtzOpticsG2;
//! let pan = PanPosition::new_for_model(2000, model)?;  // Validates against G2 pan range
//! let speed = TiltSpeed::new_for_model(18, model)?;    // Validates against G2 tilt speed
//! ```
//!
//! ### Profile-Based Compile-Time Safety
//! When using camera profiles, validation happens at compile time through
//! capability traits:
//! ```ignore
//! use grafton_visca::{Camera, camera::profiles::PtzOpticsG2};
//!
//! // Profile provides model-specific constants at compile time
//! let camera = Camera::<PtzOpticsG2, _>::new(transport);
//! // Methods automatically use profile's validated ranges
//! camera.pan_tilt_absolute(1000, 500, 10, 10)?;  // Validated against G2 limits
//! ```
//!
//! ### Command-Level Validation
//! Extended commands validate model support before encoding:
//! ```ignore
//! use grafton_visca::command::focus::FocusLock;
//!
//! // FocusLock validates it's only used with PTZOptics cameras
//! let cmd = FocusLock::On;
//! // validate_for_model() is called automatically during encoding
//! ```
//!
//! This multi-layered approach ensures:
//! - Early error detection at construction time
//! - Model-specific precision when needed
//! - Conservative defaults for generic usage
//! - Zero-cost abstractions through compile-time validation
//!
//! ## Quick Start
//!
//! ### Blocking Example
//! ```ignore
//! use grafton_visca::{
//!     camera::Connect,
//!     camera::profiles::PtzOpticsG2,
//!     Error,
//! };
//!
//! fn main() -> Result<(), Error> {
//!     // Create camera using convenience Connect helper
//!     let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
//!
//!     // Use accessor-style API
//!     camera.power().on()?;
//!     camera.zoom().tele()?;
//!     camera.pan_tilt().home()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Async Example with Multi-Runtime Support
//! ```ignore
//! use grafton_visca::{
//!     camera::Connect,
//!     camera::profiles::PtzOpticsG2,
//!     runtime::{Runtime, TokioRuntime},
//!     Error,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Create camera using Connect helper with runtime
//!     let runtime = TokioRuntime::from_current()?;
//!     let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(
//!         "192.168.0.110",
//!         runtime
//!     ).await?;
//!
//!     // Use accessor-style API with async
//!     camera.power().on().await?;
//!     camera.zoom().tele().await?;
//!     camera.pan_tilt().home().await?;
//!
//!     // Wait for movements to complete
//!     camera.await_idle().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Runtime Coexistence Example
//! ```ignore
//! // Multiple runtimes can coexist! Priority: tokio → async-std → smol
//! [dependencies]
//! grafton-visca = { version = "*", features = ["runtime-tokio", "runtime-async-std"] }
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
//!     use grafton_visca::runtime::{AsyncStdRuntime, Runtime};
//!     let runtime = AsyncStdRuntime::new();
//!     let camera = CameraBuilder::with_executor(runtime)
//!         .open_async::<PtzOpticsG2, _>(transport)
//!         .await?;
//!
//!     // Use accessor-style API across all runtimes
//!     camera.power().on().await?;
//!     camera.zoom().tele().await?;
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
//!         use grafton_visca::runtime::{Runtime, SmolRuntime};
//!         let runtime = SmolRuntime::new();
//!         let camera = CameraBuilder::with_executor(runtime)
//!             .open_async::<PtzOpticsG2, _>(transport)
//!             .await?;
//!
//!         // Use accessor-style API consistently across runtimes
//!         camera.power().on().await?;
//!         camera.pan_tilt().home().await?;
//!         camera.zoom().tele().await?;
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
//! use grafton_visca::{transport::BlockingTransport, command::CommandKind, Error};
//! use bytes::Bytes;
//! use std::time::Duration;
//!
//! struct MyTransport {
//!     // Your transport state
//! }
//!
//! impl BlockingTransport for MyTransport {
//!     fn send_with_kind(&mut self, data: &[u8], kind: CommandKind) -> Result<(), Error> {
//!         // Send data over your transport with proper framing based on kind
//!         Ok(())
//!     }
//!
//!     fn recv(&mut self) -> Result<Bytes, Error> {
//!         // Receive response from your transport
//!         Ok(Bytes::new())
//!     }
//!
//!     fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes, Error> {
//!         // Receive response with timeout
//!         Ok(Bytes::new())
//!     }
//! }
//! ```
//!
//! Example transport implementations are demonstrated in:
//! - `examples/quickstart.rs` - TCP/IP transport with blocking API
//! - `examples/quickstart_async.rs` - TCP/IP transport with async API
//! - `examples-advanced/transports.rs` - Custom and advanced transport examples
//!
//! ## Async Support
//!
//! The library provides runtime-agnostic async support, allowing you to use ANY async runtime
//! (tokio, async-std, smol, etc.) or even create your own.
//!
//! ### Feature Flags
//!
//! - `mode-async` - Enables async support without any specific runtime. You must provide your own runtime.
//! - `mode-blocking` - Explicit feature flag for blocking mode (blocking is always available, this is for feature detection).
//! - `runtime-tokio` - Enables async with built-in Tokio runtime support (implies `mode-async`).
//! - `runtime-async-std` - Enables async with built-in async-std runtime support (implies `mode-async`).
//! - `runtime-smol` - Enables async with built-in smol runtime support (implies `mode-async`).
//! - `transport-serial` - Enables serial port support for blocking mode.
//! - `transport-serial-tokio` - Enables serial port support with Tokio (implies `runtime-tokio`).
//! - `test-utils` - Testing utilities including ScriptedTransport and DeterministicExecutor (not for production).
//!
//! **Multiple Runtime Support**: As of version 0.7.0, runtime features can be enabled simultaneously.
//! This allows libraries to support multiple runtime ecosystems without forcing users to choose.
//! Use explicit executor selection (`CameraBuilder::with_executor(TokioRuntime::from_current())`, etc.) when multiple runtimes are available.
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
//! grafton-visca = { version = "*", features = ["runtime-tokio"] }
//! grafton-visca = { version = "*", features = ["runtime-async-std"] }
//! grafton-visca = { version = "*", features = ["runtime-smol"] }
//!
//! # Multiple runtimes (choose executor at construction time):
//! grafton-visca = { version = "*", features = ["runtime-tokio", "runtime-async-std"] }
//! grafton-visca = { version = "*", features = ["runtime-tokio", "runtime-smol", "runtime-async-std"] }
//! ```
//!
//! Then use the corresponding `CameraBuilder` method:
//!
//! ```ignore
//! // Tokio
//! use grafton_visca::runtime::{Runtime, TokioRuntime};
//! let runtime = TokioRuntime::from_current()?;
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // async-std
//! use grafton_visca::runtime::{AsyncStdRuntime, Runtime};
//! let runtime = AsyncStdRuntime::new();
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // smol
//! use grafton_visca::runtime::{Runtime, SmolRuntime};
//! let runtime = SmolRuntime::new();
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
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
//!         .open_async::<PtzOpticsG2, _>(transport)?;
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
//! - Enable `runtime-tokio` feature and use `CameraBuilder::with_executor(TokioRuntime::from_current())`
//! - Use `CameraBuilder::with_executor()` with your own runtime implementation
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
//! - **Async mode** (`mode-async` feature): Native async implementation
//!   - When `mode-async` feature is enabled, blocking types are NOT exported
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
//!     .open();
//!
//! // For async mode (default)
//! use grafton_visca::runtime::{Runtime, TokioRuntime};
//! let runtime = TokioRuntime::from_current()?;
//! let camera = CameraBuilder::with_executor(runtime)
//!     .timeout_config(config)
//!     .open_async::<PtzOpticsG2, _>(transport).await?;
//!
//! // For blocking mode (when async feature is disabled)
//! #[cfg(not(feature = "mode-async"))]
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

pub use grafton_visca_macros::{ViscaEnum, ViscaInquiry, ViscaValue};

pub use crate::{
    camera::{Camera, CameraBuilder},
    camera_id::CameraId,
    command::{
        exposure::ExposureMode,
        focus::{AutoFocusSensitivity, FocusMode},
        nd_filter::NdFilterMode,
        pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
        preset::PresetNumber,
        resolution::{PictureEffectMode, ResolutionMode},
        system::{MotionSyncMode, MotionSyncPreset},
        white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
    },
    error::{Error, ErrorKind, Result},
    inquiry_conversions::{
        zoom_from_normalized, Normalized, PanTiltPositionDeg, PanTiltPositionRaw, ZoomDomain,
        ZoomPositionExt,
    },
    types::{Coarse, FocusSpeed, MotionSyncSpeed, SpeedLevel, ZoomSpeed},
    visca_socket::ViscaSocket,
};

#[cfg(not(feature = "mode-async"))]
pub use crate::camera::{blocking_api::BlockingClient, BlockingCamera};

#[cfg(feature = "mode-async")]
pub use crate::camera::AsyncCamera;

#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
pub use crate::executor::AsyncStdExecutor;
#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use crate::executor::SmolExecutor;
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use crate::executor::TokioExecutor;
#[cfg(feature = "mode-async")]
pub use crate::executor::{ExecError, Executor};

#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
pub use crate::runtime::AsyncStdRuntime;
#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use crate::runtime::SmolRuntime;
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use crate::runtime::TokioRuntime;
#[cfg(feature = "mode-async")]
pub use crate::runtime::{Runtime, TransportHandle};

#[doc(hidden)]
pub use crate::camera::controls::{
    color::ColorControl,
    exposure::ExposureControl,
    focus::FocusControl,
    image_processing::ImageProcessingControl,
    inquiry::{InquiryControl, PanTiltInquiryControl},
    menu::{DirectMenuControl, MenuControl},
    motion::{MotionControl, MotionGuard},
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

mod error;
pub(crate) mod macros;

#[cfg(feature = "mode-async")]
pub(crate) mod executor;

#[cfg(not(feature = "mode-async"))]
pub(crate) mod executor {
    /// Dummy Executor trait for non-async mode.
    /// This allows the code to remain uniform regardless of feature flags.
    pub trait Executor {}

    /// Unit type implements Executor for blocking mode
    impl Executor for () {}
}

/// Camera profile system for type-safe, model-specific control
pub mod camera;

/// Camera ID type for VISCA protocol addressing
pub mod camera_id;

/// Capability traits for camera feature composition
pub mod capabilities;

/// Command definitions for VISCA protocol (advanced use only)
///
/// **⚠️ Advanced API**: This module contains low-level protocol implementation details.
/// Most users should use the high-level camera accessor API instead:
/// - `camera.power().on()` instead of manual command construction
/// - `camera.zoom().position()` instead of response matching
///
/// This module remains public for extensibility but its direct use is discouraged.
/// Consider it unstable and subject to breaking changes.
pub mod command;

/// Constants for VISCA protocol including default ports
pub mod constants;

/// Diagnostics and health check utilities
pub mod diagnostics;

/// Inquiry conversion utilities for raw to user-friendly values
pub mod inquiry_conversions;

pub mod mode;

pub mod prelude;

/// Protocol encoding and decoding utilities (internal use)
///
/// **⚠️ Internal API**: This module contains protocol-level utilities.
/// Users should not need to interact with this module directly.
pub mod protocol;

/// VISCA runtime with flume-based scheduling
pub mod runtime;

/// Runtime-specific transport adapters
#[cfg(feature = "mode-async")]
pub mod runtime_adapters;

/// Testing utilities (available with test-utils feature for deterministic testing)
#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
pub mod testing;

pub mod timeout;

/// Transport layer for implementing custom transports
pub mod transport;

/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub mod units;

/// Unified VISCA socket type
pub mod visca_socket;

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, ProfileId, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
}
