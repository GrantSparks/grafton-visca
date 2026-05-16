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
//! - **Camera-first API Architecture**: `Connect` and noun accessors across blocking and async modes
//! - **Type-Safe Camera Profiles**: Compile-time validation with camera-specific profiles
//! - **Feature-Gated Methods**: Choose blocking or async at compile time with zero runtime overhead
//! - **Multi-Runtime Support**: Tokio and smol can coexist with priority-based selection
//! - **Complete Command Coverage**: Full VISCA protocol support across all camera types
//! - **Profile-Aware Conversions**: Automatic unit conversions based on camera model
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Transport Abstraction**: TCP, UDP, Serial, and custom transport implementations
//! - **Configuration APIs**: `CameraConfig` for standard transports and `CameraBuilder` for custom transports
//! - **Unified Error Handling**: Consistent error mapping across all transport types
//! - **Configurable Timeouts**: Per-category timeout configuration for different command types
//! - **Command Cancellation**: Cancel specific commands or entire socket operations
//! - **Async Completion Tracking**: Wait for camera movements to complete with await methods
//! - **Serialization Support**: Optional serde/schemars integration for all value types
//!
//! ## 1.0 API Shape
//!
//! The primary API is camera-first:
//!
//! - Use [`camera::Connect`] for simple TCP, UDP, and serial connections.
//! - Use [`camera::CameraConfig`] when a standard transport needs explicit
//!   timeout, retry, keepalive, camera ID, or serial settings.
//! - Use accessor-style controls such as `camera.power().on()` and
//!   `camera.pan_tilt().position()` for normal operation.
//! - Import generic control traits from the crate root, for example
//!   [`PowerControl`] and [`ZoomControl`]; camera implementation submodules are
//!   internal.
//! - Use [`CameraBuilder`] only when you already own a custom transport and
//!   need to attach it to the camera runtime.
//! - Use [`UnitInterval`] for normalized `0.0..=1.0` control values and
//!   [`CameraId`] for configured VISCA camera addresses.
//! - Use [`command::ViscaCommand`] as the raw VISCA escape hatch for custom
//!   command encoding, and [`command::ResponseParser`] for typed custom inquiry
//!   responses.
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
//! use grafton_visca::camera::TransportOptions;
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
//! ### Profile-Based Compile-Time Safety
//! Camera profiles carry model-specific limits and capabilities at compile time:
//! ```ignore
//! use grafton_visca::{camera::Connect, profiles::PtzOpticsG2, units::Degrees, SpeedLevel};
//!
//! let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
//! camera.pan_tilt().absolute(Degrees(45.0), Degrees(10.0), SpeedLevel::Medium)?;
//! ```
//!
//! ### Compile-Time Capability Gating
//! Vendor-specific controls are exposed through the same accessor path and are
//! only available when the selected profile supports them:
//! ```ignore
//! use grafton_visca::{camera::Connect, profiles::PtzOpticsG2};
//!
//! let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
//! camera.focus().lock()?;
//! camera.focus().unlock()?;
//! ```
//!
//! This multi-layered approach ensures:
//! - Early error detection at construction time
//! - Conservative defaults for generic usage
//! - Profile-specific precision through compile-time validation
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
//! ### Explicit Runtime Selection Example
//! ```ignore
//! // Multiple runtime features can coexist, but runtime selection is explicit.
//! [dependencies]
//! grafton-visca = { version = "*", features = ["runtime-tokio", "runtime-smol"] }
//!
//! use grafton_visca::{
//!     Error,
//!     camera::{Connect, profiles::PtzOpticsG2},
//!     runtime::SmolRuntime,
//! };
//!
//! fn main() -> Result<(), Error> {
//!     smol::block_on(async {
//!         let runtime = SmolRuntime::new();
//!         let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(
//!             "192.168.0.110",
//!             runtime,
//!         )
//!             .await?;
//!
//!         camera.power().on().await?;
//!         camera.zoom().tele().await?;
//!
//!         Ok(())
//!     })
//! }
//! ```
//!
//! ### Configured Connection Example
//! ```ignore
//! use grafton_visca::{
//!     Error,
//!     camera::{CameraConfig, profiles::PtzOpticsG2},
//!     runtime::SmolRuntime,
//!     transport::{TcpKeepaliveConfig, TransportConfig},
//! };
//! use std::time::Duration;
//!
//! fn main() -> Result<(), Error> {
//!     smol::block_on(async {
//!         let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!             .transport_config(TransportConfig {
//!                 tcp_keepalive: Some(TcpKeepaliveConfig::new(Duration::from_secs(30))),
//!                 ..TransportConfig::default()
//!             });
//!
//!         let camera = config.open_async(SmolRuntime::new()).await?;
//!
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
//! The library includes pre-defined profiles such as `PtzOpticsG2`,
//! `SonyFR7`, and `GenericVisca`. Profiles are selected at construction time
//! with `Connect` or `CameraConfig` and then drive compile-time capability
//! checks for the accessor API.
//!
//! ### Compile-Time Type Safety
//!
//! The camera profile controls which accessors are available at compile time:
//!
//! ```ignore
//! use grafton_visca::prelude::blocking::*;
//!
//! let sony = Connect::open_udp_blocking::<SonyFR7>("192.168.0.110")?;
//! sony.nd_filter().set_mode(NdFilterMode::Clear)?;
//!
//! let g2 = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.111")?;
//! // g2.nd_filter().set_mode(NdFilterMode::Clear)?; // Compile error: G2 has no ND filter capability
//! ```
//!
//! Runtime discovery metadata is available for every profile through
//! `Capabilities::from_profile::<P>()`. Typed optional vendor controls use
//! separate support markers, so the public API exposes only documented support:
//! `SonyFR7` has typed ND filter and variable speed controls. Built-in
//! PTZOptics profiles are not marked for typed Motion Sync from the current
//! model capability specs.
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
//! - `examples/transports.rs` - Protocol and transport comparisons
//! - `examples/transport_builder_demo.rs` - Transport configuration patterns
//!
//! ## Async Support
//!
//! The library provides runtime-agnostic async support, allowing you to use ANY async runtime
//! (tokio, smol, etc.) or even create your own.
//!
//! ### Feature Flags
//!
//! - `mode-async` - Enables async support without any specific runtime. You must provide your own runtime.
//! - Blocking API - Baseline API when `mode-async` is not enabled.
//! - `runtime-tokio` - Enables async with built-in Tokio runtime support (implies `mode-async`).
//! - `runtime-smol` - Enables async with built-in smol runtime support (implies `mode-async`).
//! - `transport-serial` - Enables serial port support for blocking mode.
//! - `transport-serial-tokio` - Enables serial port support with Tokio (implies `runtime-tokio`).
//! - `test-utils` - Deterministic test transports and executors for crate and downstream tests.
//!
//! **Multiple Runtime Support**: Runtime features can be enabled simultaneously.
//! This allows libraries to support multiple runtime ecosystems without forcing users to choose.
//! Pass the runtime explicitly to `Connect` or `CameraConfig` when multiple
//! runtime features are available.
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
//! ### Runtime Requirements for Async
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
//! grafton-visca = { version = "*", features = ["runtime-smol"] }
//!
//! # Multiple runtimes (choose executor at construction time):
//! grafton-visca = { version = "*", features = ["runtime-tokio", "runtime-smol"] }
//! ```
//!
//! Then pass the runtime explicitly, either through `Connect` for quick setup or
//! `CameraBuilder::with_executor(...)` for advanced BYO-transport flows:
//!
//! ```ignore
//! // Tokio
//! use grafton_visca::{camera::{Connect, profiles::PtzOpticsG2}, runtime::TokioRuntime};
//! let runtime = TokioRuntime::from_current()?;
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!
//! // smol
//! use grafton_visca::{camera::{Connect, profiles::PtzOpticsG2}, runtime::SmolRuntime};
//! let runtime = SmolRuntime::new();
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//! ```
//!
//! #### Option 2: Provide your own runtime (Advanced)
//!
//! For complete runtime independence, implement `Executor` and attach your own
//! async transport with `CameraBuilder::from_transport(...)`:
//!
//! ```ignore
//! use grafton_visca::{
//!     CameraBuilder, Error, ExecError, Executor,
//!     camera::profiles::PtzOpticsG2,
//! };
//! use std::{future::Future, pin::Pin, time::Duration};
//!
//! #[derive(Debug, Clone)]
//! struct MyExecutor;
//!
//! impl Executor for MyExecutor {
//!     type Join<T> = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
//!     where T: Send + 'static;
//!
//!     type Detach = ();
//!
//!     fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
//!     where
//!         F: Future + Send + 'static,
//!         F::Output: Send + 'static,
//!     {
//!         // Spawn on your runtime here
//!     }
//!
//!     fn block_on<F: Future>(&self, fut: F) -> F::Output {
//!         todo!()
//!     }
//!
//!     fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
//!         async move {
//!             let _ = duration;
//!         }
//!     }
//!
//!     fn timeout<'a, F, T>(
//!         &'a self,
//!         duration: Duration,
//!         fut: F,
//!     ) -> impl Future<Output = Result<T, Error>> + Send + 'a
//!     where
//!         F: Future<Output = T> + Send + 'a,
//!         T: Send + 'a,
//!     {
//!         async move {
//!             let _ = duration;
//!             Ok(fut.await)
//!         }
//!     }
//! }
//!
//! async fn main() -> Result<(), Error> {
//!     let transport = MyAsyncTransport::connect("192.168.0.110:5678").await?;
//!     let camera = CameraBuilder::with_executor(MyExecutor)
//!         .from_transport(transport)
//!         .profile::<PtzOpticsG2>()
//!         .open_async()
//!         .await?;
//!
//!     camera.power().on().await?;
//!     Ok(())
//! }
//! ```
//!
//! See `examples/runtime_agnostic.rs` for a complete end-to-end example.
//!
//! ### Common Runtime Errors and Solutions
//!
//! #### Error: `InvalidState("No runtime configured for async operations")`
//! **Cause:** You're using async mode but haven't configured a runtime.
//! **Solution:** Either:
//! - Enable `runtime-tokio` and pass `TokioRuntime::from_current()?` to `Connect` or `CameraConfig`
//! - Use `CameraBuilder::with_executor()` only when attaching your own transport/runtime implementation
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
//! camera.pan_tilt().absolute(Degrees(45.0), Degrees(-15.0), SpeedLevel::Medium)?;
//!
//! // Stop all movement
//! camera.pan_tilt().stop()?;
//!
//! // Move to home position
//! camera.pan_tilt().home()?;
//! ```
//!
//! ## Timeout Configuration
//!
//! Configure timeouts per command category based on your network and camera:
//!
//! ```ignore
//! use grafton_visca::{camera::{CameraConfig, profiles::PtzOpticsG2}, TimeoutConfig};
//! use std::time::Duration;
//!
//! let config = TimeoutConfig::builder()
//!     .ack_timeout(Duration::from_millis(300))
//!     .quick_timeout(Duration::from_secs(3))
//!     .movement_timeout(Duration::from_secs(20))
//!     .preset_timeout(Duration::from_secs(60))
//!     .build();
//!
//! // For async mode
//! use grafton_visca::runtime::TokioRuntime;
//! let runtime = TokioRuntime::from_current()?;
//! let camera = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!     .timeouts(config)
//!     .open_async(runtime)
//!     .await?;
//!
//! // For blocking mode (when async feature is disabled)
//! #[cfg(not(feature = "mode-async"))]
//! let camera = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!     .timeouts(config)
//!     .open_blocking()?;
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
//! ## Movement Completion Tracking
//!
//! Wait for camera movements to complete using [`AwaitConfig`](camera::AwaitConfig):
//!
//! ```ignore
//! use std::time::Duration;
//! use grafton_visca::camera::{AwaitConfig, Axes};
//!
//! // Start a pan/tilt movement
//! camera
//!     .pan_tilt()
//!     .absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Medium)
//!     .await?;
//!
//! // Wait for all movements to complete (pan/tilt, zoom, focus)
//! camera.await_idle(Duration::from_secs(30)).await?;
//!
//! // Or wait for specific axes with custom configuration
//! let config = AwaitConfig::new(Duration::from_secs(10))
//!     .with_axes(Axes::PAN_TILT)
//!     .with_debug();
//! camera.await_with_config(&config).await?;
//!
//! // Convenience methods for common scenarios
//! camera.await_pan_tilt_idle(Duration::from_secs(20)).await?;
//! camera.await_zoom_idle(Duration::from_secs(15)).await?;
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error types for all VISCA error conditions:
//!
//! ```ignore
//! match camera
//!     .pan_tilt()
//!     .absolute(Degrees(180.0), Degrees(0.0), SpeedLevel::Medium)
//!     .await
//! {
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
    cache::{CachedFlipState, PanTiltLimits, StateCache},
    camera::{
        controls::{
            AutoFocusSensitivityControl, AutoFocusSensitivityInquiryControl,
            AutoTrackingWhiteBalanceControl, AutoWhiteBalanceSensitivityControl,
            BacklightCompensationControl, BacklightCompensationInquiryControl, BrightnessControl,
            BrightnessInquiryControl, ColorControl, ColorTemperatureControl,
            ColorTemperatureInquiryControl, ContrastControl, ContrastInquiryControl,
            DigitalZoomControl, DigitalZoomRangeControl, DirectMenuControl, DirectZoomControl,
            ExposureCompensationControl, ExposureCompensationInquiryControl, ExposureControl,
            FocusControl, FocusLockControl, FocusNearLimitInquiryControl, FocusZoneControl,
            FocusZoneInquiryControl, GammaControl, GammaInquiryControl, HueControl,
            HueInquiryControl, ImageFlipControl, ImageFlipInquiryControl, ImageFlipModeControl,
            ImageMirrorControl, InquiryControl, IrisControl, IrisInquiryControl, LuminanceControl,
            LuminanceInquiryControl, MenuControl, MotionControl, MotionSyncControl,
            NdFilterControl, NdFilterInquiryControl, NoiseReduction2DControl,
            NoiseReduction2DInquiryControl, NoiseReduction3DControl,
            NoiseReduction3DInquiryControl, NoiseReductionInquiryControl, OnePushFocusControl,
            OnePushWhiteBalanceControl, PanTiltControl, PanTiltInquiryControl,
            PictureEffectControl, PictureEffectInquiryControl, PowerControl, PresetsControl,
            PushAFControl, RgbGainControl, RgbGainInquiryControl, RgbTuningControl,
            RgbTuningInquiryControl, SaturationControl, SaturationInquiryControl, SharpnessControl,
            SharpnessInquiryControl, SnapFocusControl, StreamingControl, SystemControl,
            TallyControl, VariableSpeedControl, WhiteBalanceControl, WideDynamicRangeControl,
            WideDynamicRangeInquiryControl, ZoomControl,
        },
        Camera, CameraBuilder,
    },
    camera_id::CameraId,
    command::{
        AutoFocusSensitivity, AutoWhiteBalanceSensitivity, ExposureMode, FocusMode, MotionSyncMode,
        MotionSyncPreset, NdFilterMode, NdFilterPosition, PanTiltDirection, PanTiltLimitCorner,
        PictureEffectMode, PresetNumber, ResolutionMode, WhiteBalanceMode,
    },
    error::{Error, ErrorKind, Result},
    inquiry_conversions::{
        zoom_from_normalized, PanTiltPositionDeg, PanTiltPositionRaw, ZoomDomain, ZoomPositionExt,
    },
    types::{Coarse, FocusSpeed, MotionSyncSpeed, SpeedLevel, ZoomSpeed},
    units::UnitInterval,
    visca_socket::ViscaSocket,
};

#[cfg(not(feature = "mode-async"))]
pub use crate::camera::{BlockingCamera, BlockingClient};

#[cfg(feature = "mode-async")]
pub use crate::camera::AsyncCamera;

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use crate::executor::SmolExecutor;
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use crate::executor::TokioExecutor;
#[cfg(feature = "mode-async")]
pub use crate::executor::{ExecError, Executor};

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use crate::runtime::SmolRuntime;
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use crate::runtime::TokioRuntime;
#[cfg(feature = "mode-async")]
pub use crate::runtime::{Runtime, TransportHandle};

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

mod cache;

/// Camera profile system for type-safe, model-specific control
pub mod camera;

mod camera_id;

/// Capability traits for camera feature composition
pub mod capabilities;

/// Low-level VISCA command definitions and extension traits.
///
/// Most users should use the high-level camera accessor API instead:
/// - `camera.power().on()` instead of manual command construction
/// - `camera.zoom().position()` instead of response matching
///
/// This module is the stable low-level extension surface for custom commands,
/// typed response parsing, and integrations that need raw VISCA command control.
pub mod command;

/// Inquiry conversion utilities for raw to user-friendly values
pub mod inquiry_conversions;

pub mod mode;

pub mod prelude;

pub(crate) mod protocol;

/// VISCA runtime with flume-based scheduling
pub mod runtime;

/// Runtime-specific transport adapters
#[cfg(feature = "mode-async")]
pub mod runtime_adapters;

/// Testing utilities for deterministic downstream tests.
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

pub mod timeout;

/// Transport layer for implementing custom transports
pub mod transport;

/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub mod units;

mod visca_socket;

/// Dynamic trait object API with per-operation timeout support.
///
/// This module provides object-safe trait definitions (`dyn DynCameraControl`)
/// for runtime polymorphism. Use this when you need to work with cameras as
/// trait objects rather than concrete generic types.
///
/// Enable with the `dyn-api` feature flag.
#[cfg(feature = "dyn-api")]
pub mod dynapi;

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, ProfileGroup, ProfileId, PtzOptics30X, PtzOpticsG2,
        PtzOpticsG3, SonyBRC300, SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::capabilities::InquirySupport;
}
