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
//! - **Owner-backed Camera API**: `Connect` and noun accessors across the blocking and async facades
//! - **Type-Safe Camera Profiles**: Compile-time validation with camera-specific profiles
//! - **Feature-Gated Methods**: Choose blocking or async at compile time with zero runtime overhead
//! - **Multi-Runtime Support**: Tokio and smol can coexist with explicit runtime selection
//! - **Profile-Gated Command Coverage**: Typed controls follow documented model support
//! - **Profile-Aware Conversions**: Automatic unit conversions based on camera model
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Transport Abstraction**: TCP, UDP, Serial, and custom transport implementations
//! - **Configuration APIs**: `CameraConfig` for standard transports and `Session::open` for caller-owned transports
//! - **Unified Error Handling**: Consistent error mapping across all transport types
//! - **Configurable Timeouts**: Per-category timeout configuration for different command types
//! - **Lifecycle-Exact Operation Handles**: Exact applied waits, physical settled waits,
//!   cancellation, and detach across blocking, async, and dynamic APIs
//! - **Serialization Support**: Optional serde/schemars integration for all value types
//!
//! ## 2.0 API Shape
//!
//! The primary API is camera-first:
//!
//! - Use `camera::Connect` for simple TCP, UDP, and serial connections.
//! - Use [`camera::CameraConfig`] when a standard transport needs explicit
//!   timeout, retry, keepalive, camera ID, or serial settings.
//! - Use accessor-style controls such as `camera.power().on()` and
//!   `camera.pan_tilt().position()` for normal operation.
//! - Use `Session::open` (async) or `blocking::Session::open` when you already
//!   own a custom transport and need to attach it to the owner.
//! - Use [`UnitInterval`] for normalized `0.0..=1.0` control values and
//!   [`CameraId`] for configured VISCA camera addresses.
//! - Use the typed [`Request`] and [`Inquiry`] contracts for built-in and custom
//!   requests, and [`command::ResponseParser`] for typed inquiry responses.
//!
//! ## Typed Request Extensions
//!
//! The final request contract classifies every value at the type level. Implement
//! [`Request`] with one closed [`request`] class, then implement [`Inquiry`] or
//! [`OperationCommand`] only when that class requires it. [`PlainCommand`] is
//! provided automatically for every plain request. The homogeneous built-ins in
//! [`request::builtin`] show the corresponding targeted, applied-only, plain, and
//! inquiry shapes.
//!
//! Request values carry their timeout, retry, and control-policy classes. Camera
//! targets, completion kinds, retry behavior, and priority cannot be overridden
//! at submission time. Supported `ViscaInquiry` derives implement this typed
//! inquiry contract; downstream derives use the conservative inquiry retry class.
//!
//! Runtime-only camera models start with
//! [`capabilities::Capabilities::runtime_baseline`] and must explicitly provide
//! all protocol facts through [`ProfileSpec::builder`]. Built-in and downstream
//! compile-time profiles expose fact-only [`CompileTimeProfile`] implementations
//! and lower through [`ProfileSpec::from_compile_time`] to the same validated
//! immutable representation.
//!
//! ## Serialization Support
//!
//! All public value types support optional serialization through feature-gated `serde` and `schemars` derives:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "2.0.0-rc.1", features = ["serde", "schemars"] }
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
//! use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2, units::Degrees, SpeedLevel};
//!
//! let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//! let camera = session.camera::<PtzOpticsG2>()?;
//! camera.pan_tilt().absolute(Degrees(45.0), Degrees(10.0), SpeedLevel::Medium)?.settled()?;
//! session.close()?;
//! ```
//!
//! ### Compile-Time Capability Gating
//! Vendor-specific controls are exposed through the same accessor path and are
//! only available when the selected profile supports them:
//! ```ignore
//! use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2, FocusLock};
//!
//! let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//! let camera = session.camera::<PtzOpticsG2>()?;
//! camera.focus().set_lock(FocusLock::On)?;
//! camera.focus().set_lock(FocusLock::Off)?;
//! session.close()?;
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
//!     blocking::Connect,
//!     camera::profiles::PtzOpticsG2,
//!     Error,
//! };
//!
//! fn main() -> Result<(), Error> {
//!     // Create one owner-backed session using the convenience Connect helper
//!     let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!
//!     // Quick starts are read-only by default.
//!     let is_on = camera.power().state()?;
//!     let zoom = camera.zoom().position()?;
//!     println!("Power: {is_on}, zoom: 0x{:04X}", zoom.value());
//!
//!     session.close()
//! }
//! ```
//!
//! ### Async Example with Multi-Runtime Support
//! ```ignore
//! use grafton_visca::{
//!     camera::Connect,
//!     camera::profiles::PtzOpticsG2,
//!     runtime::TokioRuntime,
//!     Error,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Create one owner-backed session using Connect with a runtime
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = Connect::open_tcp::<PtzOpticsG2, _>(
//!         "192.168.0.110",
//!         runtime
//!     ).await?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!
//!     let is_on = camera.power().state().await?;
//!     let zoom = camera.zoom().position().await?;
//!     println!("Power: {is_on}, zoom: 0x{:04X}", zoom.value());
//!
//!     session.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Explicit Runtime Selection Example
//! ```ignore
//! // Multiple runtime features can coexist, but runtime selection is explicit.
//! [dependencies]
//! grafton-visca = { version = "2.0.0-rc.1", features = ["runtime-tokio", "runtime-smol"] }
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
//!         let session = Connect::open_tcp::<PtzOpticsG2, _>(
//!             "192.168.0.110",
//!             runtime,
//!         )
//!             .await?;
//!         let camera = session.camera::<PtzOpticsG2>()?;
//!
//!         let is_on = camera.power().state().await?;
//!         println!("Power: {is_on}");
//!
//!         session.close().await?;
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
//!         let session = config.open_async(SmolRuntime::new()).await?;
//!         let camera = session.camera::<PtzOpticsG2>()?;
//!
//!         let is_on = camera.power().state().await?;
//!         println!("Power: {is_on}");
//!
//!         session.close().await?;
//!         Ok(())
//!     })
//! }
//! ```
//!
//! ### Bounded Operation Handles
//!
//! Ordinary noun methods are the simplest command-completion API. Use `submit`
//! when one command needs an exact deadline, cancellation, detach, or a physical
//! settle signal:
//!
//! ```ignore
//! use std::time::Duration;
//! use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2};
//!
//! let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//! let camera = session.camera::<PtzOpticsG2>()?;
//! camera.pan_tilt().home()?.settled_with_timeout(Duration::from_secs(20))?;
//! camera.zoom().stop()?.applied_with_timeout(Duration::from_secs(2))?;
//! session.close()?;
//! ```
//!
//! `submit` manages lifecycle; it does not add profile capability or range
//! validation beyond the command's own checks. Prefer typed noun controls for
//! profile-sensitive ergonomic input.
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
//! let sony = Connect::open_udp::<SonyFR7>("192.168.0.110")?;
//! sony.nd_filter().set_mode(NdFilterMode::Clear)?;
//!
//! let g2 = Connect::open_tcp::<PtzOpticsG2>("192.168.0.111")?;
//! // g2.nd_filter().set_mode(NdFilterMode::Clear)?; // Compile error: G2 has no ND filter capability
//! ```
//!
//! Runtime discovery metadata is available for every profile through
//! `Capabilities::from_profile::<P>()`. Typed optional vendor controls use
//! separate support markers, so the public API exposes only documented support:
//! `SonyFR7` has typed ND filter and variable speed controls. Dyn-api callers
//! can query the same permission model with `Capabilities::supports_typed(...)`;
//! overlapping metadata fields remain discovery facts, not typed permission
//! checks. Built-in PTZOptics profiles are not marked for typed Motion Sync from
//! the current model capability specs.
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
//! - `blocking` - Enables the canonical owner-backed blocking facade (the default).
//! - `async` - Enables runtime-agnostic async support; it may coexist with `blocking`.
//! - `runtime-tokio` - Enables the async facade with built-in Tokio runtime support.
//! - `runtime-smol` - Enables the async facade with built-in smol runtime support.
//! - `transport-serial` - Enables serial port support for the blocking facade.
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
//! Async camera types always carry an executor/runtime selected at construction,
//! so a camera cannot be created in an unconfigured runtime state. The runtime
//! owns background scheduling, timing, transport I/O, and settle polling.
//! There are two supported construction strategies:
//!
//! #### Option 1: Use a built-in runtime adapter
//!
//! Choose your runtime(s) and enable the corresponding feature(s) in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! # Single runtime:
//! grafton-visca = { version = "2.0.0-rc.1", features = ["runtime-tokio"] }
//! grafton-visca = { version = "2.0.0-rc.1", features = ["runtime-smol"] }
//!
//! # Multiple runtimes (choose executor at construction time):
//! grafton-visca = { version = "2.0.0-rc.1", features = ["runtime-tokio", "runtime-smol"] }
//! ```
//!
//! Pass the runtime explicitly through `Connect` or `CameraConfig`; both return
//! the canonical owner-backed `Session`:
//!
//! ```ignore
//! // Tokio
//! use grafton_visca::{camera::{Connect, profiles::PtzOpticsG2}, runtime::TokioRuntime};
//! let runtime = TokioRuntime::from_current()?;
//! let session = Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//! let camera = session.camera::<PtzOpticsG2>()?;
//!
//! // smol
//! use grafton_visca::{camera::{Connect, profiles::PtzOpticsG2}, runtime::SmolRuntime};
//! let runtime = SmolRuntime::new();
//! let session = Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//! let camera = session.camera::<PtzOpticsG2>()?;
//! ```
//!
//! #### Option 2: Provide an executor and transport adapter
//!
//! For complete runtime independence, implement `Executor` and
//! `transport::AsyncTransport` plus `transport::HasTransportConfig`, then
//! attach them with `Session::open`. The executor must provide
//! real spawn, detach, sleep, timeout, and clock behavior from one coherent
//! runtime; placeholder or mixed-runtime implementations are not valid.
//!
//! See `examples/runtime_agnostic.rs` for a runnable executor-contract example
//! and `Session::open` for the advanced caller-owned transport path.
//!
//! ### Blocking and Async Facades
//!
//! The library provides a clean separation between blocking and async APIs:
//!
//! - **Blocking facade** (`blocking`, enabled by default): synchronous owner API.
//! - **Async facade** (`async`): native async owner API with explicit executor
//!   selection. Both facades can be compiled together; their owner/session and
//!   operation handle types remain distinct.
//!
//! Both facades use the same owner/session model while exposing independent
//! blocking and async feature selections.
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
//! // For the async facade
//! use grafton_visca::runtime::TokioRuntime;
//! let runtime = TokioRuntime::from_current()?;
//! let camera = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!     .timeouts(config)
//!     .open_async(runtime)
//!     .await?;
//!
//! // For the blocking facade
//! #[cfg(feature = "blocking")]
//! let camera = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!     .timeouts(config)
//!     .open()?;
//! ```
//!
//! ## Command Cancellation (Async)
//!
//! Cancel specific commands or entire socket operations:
//!
//! ```ignore
//! // Submit one typed operation and retain its owner-backed lifecycle handle.
//! let operation = camera.zoom().tele().await?;
//! let cancellation = operation.cancel().await?;
//! let outcome = cancellation.outcome(Duration::from_secs(2)).await?;
//! println!("cancellation outcome: {outcome:?}");
//! ```
//!
//! Queued commands can always be removed locally. Cancelling a command that has
//! already been sent requires profile support for the standard VISCA socket-cancel
//! command and otherwise returns [`Error::NotSupported`]. For bounded continuous
//! movement, send the relevant STOP command and await its application.
//!
//! ## Movement Completion Tracking
//!
//! Observe or wait for exactly selected physical axes with
//! [`camera::MotionQuery`] and [`camera::IdleWait`]. Unselected axes are never
//! queried:
//!
//! ```ignore
//! use std::time::Duration;
//! use grafton_visca::{AffectedAxes, MotionQuery};
//! use grafton_visca::camera::IdleWait;
//!
//! let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
//! let moving = camera.motion().is_moving(MotionQuery::new(axes)).await?;
//! camera.motion().wait_until_idle(IdleWait::new(axes, Duration::from_secs(30))).await?;
//! # let _ = moving;
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

#[cfg(all(feature = "async", feature = "runtime-smol"))]
pub use crate::executor::SmolExecutor;
#[cfg(all(feature = "async", feature = "runtime-tokio"))]
pub use crate::executor::TokioExecutor;
#[cfg(feature = "async")]
pub use crate::executor::{ExecError, Executor};

#[cfg(all(feature = "async", feature = "runtime-smol"))]
pub use crate::runtime::SmolRuntime;
#[cfg(all(feature = "async", feature = "runtime-tokio"))]
pub use crate::runtime::TokioRuntime;
#[cfg(feature = "async")]
pub use crate::runtime::{Runtime, TransportHandle};

mod error;
pub(crate) mod macros;

#[cfg(feature = "async")]
pub(crate) mod executor;

/// Closed operation completion kinds used by typed requests.
pub mod completion;

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

/// Closed request-class markers and homogeneous built-in request types.
pub mod request;

/// Explicitly classified raw VISCA request escape hatches.
///
/// Raw values are accepted only through the typed `execute`, `inquire`, and
/// `submit` owner methods. They carry explicit policy and cannot select camera
/// targets or completion behavior at runtime.
pub mod raw;

mod requests;
pub use requests::{
    AffectedAxes, AffectedAxis, AffectedAxisIter, ControlClass, EncodeError, Inquiry, InquiryRoute,
    OperationCommand, PlainCommand, Request, ResponseDecoder, RetryClass, TimeoutClass,
};

mod prepared;

/// Bounded owner metrics and diagnostic subscriptions.
pub mod observability;

/// Read-only target-local state projections committed by the owner.
pub mod state_cache;

pub use observability::{
    DiagnosticCancellation, DiagnosticEvent, DiagnosticId, DiagnosticIgnoreReason, DiagnosticLane,
    DiagnosticOutcome, DiagnosticPhase, DiagnosticResponse, DiagnosticSubscription,
    MetricsSnapshot, SessionStatus,
};
pub use state_cache::{StateCache, StateEntry, StateKey, StateValue};

mod session_config;
pub use session_config::SessionConfig;

mod outcome;
pub use outcome::CancellationOutcome;

mod operation_id;
pub use operation_id::OperationId;

#[cfg(feature = "async")]
mod operation;
#[cfg(feature = "async")]
pub use operation::{Cancellation, Operation};

#[cfg(feature = "async")]
mod async_nouns;
#[cfg(feature = "async")]
mod async_session;
#[cfg(feature = "async")]
pub use async_nouns::{
    AdvancedAccessor, ExposureAccessor, FocusAccessor, ImageAccessor, MenuAccessor, MotionAccessor,
    MotionSyncAccessor, NdFilterAccessor, PanTiltAccessor, PowerAccessor, PresetsAccessor,
    SystemAccessor, TallyAccessor, WhiteBalanceAccessor, ZoomAccessor,
};
#[cfg(feature = "async")]
pub use async_session::{Camera, Session};
#[cfg(feature = "async")]
pub mod session {
    //! Profile-generic async session facade.
    #[cfg(feature = "transport-serial-tokio")]
    pub use crate::camera::SerialConnectBuilder;
    pub use crate::camera::{
        CameraConfig, Connect, ConnectBuilder, TcpConnectBuilder, UdpConnectBuilder,
    };
    pub use crate::{async_session::Camera, async_session::Session, SessionConfig};
    pub use crate::{
        AdvancedAccessor, ExposureAccessor, FocusAccessor, ImageAccessor, MenuAccessor,
        MotionAccessor, MotionSyncAccessor, NdFilterAccessor, PanTiltAccessor, PowerAccessor,
        PresetsAccessor, SystemAccessor, TallyAccessor, WhiteBalanceAccessor, ZoomAccessor,
    };
}

// Final construction names are available at the crate root in canonical
// async builds.
#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
pub use crate::camera::SerialConnectBuilder;
#[cfg(feature = "async")]
pub use crate::camera::{CameraConfig, Connect, ConnectBuilder};
#[cfg(feature = "async")]
pub use crate::camera::{TcpConnectBuilder, UdpConnectBuilder};

#[cfg(feature = "blocking")]
/// Synchronous lifecycle handles for blocking sessions.
pub mod blocking;

/// Inquiry conversion utilities for raw to user-friendly values
pub mod inquiry_conversions;

pub mod prelude;

/// Validated runtime and compile-time camera profile lowering.
pub mod profile;

pub use profile::{
    CompileTimeProfile, OperationalTuning, PanTiltCoordinateConversion, PositionInquirySupport,
    ProfileEnvelope, ProfileSpec, ProfileSpecBuilder, ProfileTiming, TransportCompatibility,
};

pub(crate) mod protocol;

/// VISCA runtime with flume-based scheduling
pub mod runtime;

/// Runtime-specific transport adapters
#[cfg(feature = "async")]
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

/// Owner-backed dynamic noun and custom-operation projections.
///
/// Enable this module with the `dyn-api` feature. Dynamic views erase the
/// closed static request/profile types while retaining the canonical session
/// owner and operation lifecycle.
#[cfg(feature = "dyn-api")]
pub mod dynapi;

/// Owner-backed dynamic projections and noun traits.
#[cfg(feature = "dyn-api")]
pub use dynapi::*;

/// Camera profiles with compositional capabilities
pub mod profiles {
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, ProfileGroup, ProfileId, PtzOptics30X, PtzOpticsG2,
        PtzOpticsG3, SonyBRC300, SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::capabilities::InquirySupport;
}

/// Compile gate for the Rust snippets in `README.md`.
///
/// `cfg(doctest)` is set only while rustdoc is collecting doctests, so this
/// module never reaches the compiled library or the rendered documentation. It
/// exists so `cargo test --doc` compiles the crates.io front page and the
/// README cannot drift away from the public API.
///
/// The gate is enabled whenever the default `blocking` feature is on, which
/// covers the README's primary snippets. Snippets needing more than the default
/// feature set carry their own `#[cfg(feature = "...")]`, so they compile under
/// `--all-features` and are compiled away otherwise.
#[cfg(all(doctest, feature = "blocking"))]
#[doc = include_str!("../README.md")]
mod readme_snippets {}
