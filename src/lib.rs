//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.

#![forbid(unsafe_code)]
// The generated noun/surface ledger is deliberately exhaustive and uses
// continuation-style collectors.  Keep a modest budget for its expansion.
#![recursion_limit = "512"]
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
// With no facade feature selected the crate still compiles the protocol
// engine, the owner core, request preparation and the semantic ledger — that
// configuration is deliberately a "does the pure engine/domain still build"
// check (CI's `no-default pure engine/domain` leg), and in it every one of
// those crate-private items is unreferenced *by construction*, because the
// only things that ever call them are the blocking and async facades that the
// leg switches off.
//
// This is the single remaining dead-code exemption in the crate. It is
// conditional, so every configuration anyone actually ships reports dead code
// normally and each surviving unused item has to carry its own targeted
// `#[allow(dead_code)]` naming the consumer it is waiting for (#636).
#![cfg_attr(not(any(feature = "blocking", feature = "async")), allow(dead_code))]

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
//! targets, completion kinds, and retry behavior cannot be overridden at
//! submission time. The scheduling class can: see [Submission priority] below.
//! Supported `ViscaInquiry` derives implement this typed inquiry contract;
//! downstream derives use the conservative inquiry retry class.
//!
//! ## Submission priority
//!
//! [`ControlClass`] names one of the owner's four scheduling lanes. The owner
//! dispatches ready work from the highest occupied class first and FIFO within
//! a class, so the class only decides which **queued** request is written next;
//! it never interrupts, cancels, or reorders a request already on the wire.
//!
//! Every request classifies itself — ordinary control traffic is
//! [`ControlClass::Normal`], direct movement is [`ControlClass::User`], and the
//! typed stops and cancels are [`ControlClass::Urgent`] so an emergency stop
//! preempts queued work with no extra ceremony. Two typed routes select a class
//! explicitly:
//!
//! - Per handle: `set_command_class(Some(class))` on a camera view, its
//!   single-camera session, or the dynamic projection. Every later submission
//!   from *that handle* uses `class`, including the ones its noun accessors
//!   make — except that an urgent request is never demoted.
//! - Per submission: `execute_with_class`, `inquire_with_class`, and
//!   `submit_with_class` (`submit_targeted_with_class` /
//!   `submit_applied_with_class` on the dynamic projection). These replace both
//!   the request's own class and the handle default for one submission, and
//!   they are the only route that can demote an urgent stop.
//!
//! ```ignore
//! use grafton_visca::ControlClass;
//!
//! // Keep a telemetry poller out of the operator's way.
//! let mut poller = camera.clone();
//! poller.set_command_class(Some(ControlClass::Background));
//! let zoom = poller.zoom().position().await?;   // queued behind operator input
//! poller.pan_tilt().stop().await?;              // still urgent
//!
//! // Raise one safety-interlock command without changing the handle.
//! camera.execute_with_class(&command, ControlClass::Urgent).await?;
//! ```
//!
//! [Submission priority]: #submission-priority
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
//! ```rust
//! use grafton_visca::types::{PanSpeed, SpeedLevel, ZoomSpeed};
//!
//! // All range types expose MIN/MAX constants for validation
//! assert_eq!(PanSpeed::MIN.value(), 0);
//! assert_eq!(PanSpeed::MAX.value(), 24);
//!
//! // Validated constructors provide clear error messages
//! let speed = PanSpeed::new(15).expect("0..=24 is in range");
//! assert_eq!(speed.value(), 15);
//! let error = PanSpeed::new(30).expect_err("25..=255 is out of range");
//! println!("{error}");
//!
//! // Speed types work seamlessly with SpeedLevel enum
//! let zoom = ZoomSpeed::from(SpeedLevel::Fast); // Automatic conversion
//! assert_eq!(zoom.value(), 6); // Fast = 6 for zoom
//! ```
//!
//! ### Profile-Based Compile-Time Safety
//! Camera profiles carry model-specific limits and capabilities at compile time:
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn absolute_move() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2, units::Degrees, SpeedLevel};
//!
//!     let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!     camera.pan_tilt().absolute(Degrees(45.0), Degrees(10.0), SpeedLevel::Medium)?.settled()?;
//!     session.close()
//! }
//! ```
//!
//! ### Compile-Time Capability Gating
//! Vendor-specific controls are exposed through the same accessor path and are
//! only available when the selected profile supports them:
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn toggle_focus_lock() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2, command::FocusLock};
//!
//!     let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!     camera.focus().set_lock(FocusLock::On)?;
//!     camera.focus().set_lock(FocusLock::Off)?;
//!     session.close()
//! }
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
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn quick_start() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2};
//!
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
//! ```rust
//! # #[cfg(feature = "runtime-tokio")]
//! async fn quick_start() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{camera::profiles::PtzOpticsG2, runtime::TokioRuntime, Connect};
//!
//!     // Create one owner-backed session using Connect with a runtime
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!
//!     let is_on = camera.power().state().await?;
//!     let zoom = camera.zoom().position().await?;
//!     println!("Power: {is_on}, zoom: 0x{:04X}", zoom.value());
//!
//!     session.close().await
//! }
//! ```
//!
//! ### Explicit Runtime Selection Example
//!
//! Multiple runtime features can coexist, but runtime selection is explicit:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "2.0.0-rc.1", features = ["runtime-tokio", "runtime-smol"] }
//! ```
//!
//! ```rust
//! # #[cfg(feature = "runtime-smol")]
//! fn quick_start() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{camera::profiles::PtzOpticsG2, runtime::SmolRuntime, Connect};
//!
//!     smol::block_on(async {
//!         let runtime = SmolRuntime::new();
//!         let session =
//!             Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!         let camera = session.camera::<PtzOpticsG2>()?;
//!
//!         let is_on = camera.power().state().await?;
//!         println!("Power: {is_on}");
//!
//!         session.close().await
//!     })
//! }
//! ```
//!
//! ### Configured Connection Example
//! ```rust
//! # #[cfg(feature = "runtime-smol")]
//! fn configured() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{
//!         camera::{profiles::PtzOpticsG2, CameraConfig},
//!         runtime::SmolRuntime,
//!         transport::{TcpKeepaliveConfig, TransportConfig},
//!     };
//!     use std::time::Duration;
//!
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
//!         session.close().await
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
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn bounded_moves() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{blocking::Connect, camera::profiles::PtzOpticsG2};
//!     use std::time::Duration;
//!
//!     let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!     camera.pan_tilt().home()?.settled_with_timeout(Duration::from_secs(20))?;
//!     camera.zoom().stop()?.applied_with_timeout(Duration::from_secs(2))?;
//!     session.close()
//! }
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
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn profile_gated_controls() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::prelude::blocking::*;
//!
//!     let sony = Connect::open_udp::<SonyFR7>("192.168.0.110")?;
//!     sony.camera::<SonyFR7>()?
//!         .nd_filter()
//!         .set_mode(NdFilterMode::Preset)?;
//!
//!     let g2 = Connect::open_tcp::<PtzOpticsG2>("192.168.0.111")?;
//!     let g2_camera = g2.camera::<PtzOpticsG2>()?;
//!     // g2_camera.nd_filter().set_mode(NdFilterMode::Preset)?;
//!     // ^ Compile error: G2 has no ND filter capability
//!     let _ = g2_camera;
//!
//!     sony.close()?;
//!     g2.close()
//! }
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
//! The library provides transport traits that you can implement for any
//! communication method. The receive side reads into a caller-provided buffer
//! and returns the byte count, so a transport never allocates per frame.
//! `Ok(0)` means the peer
//! closed the connection; an idle timeout is *no data*, not a fault. A
//! transport also declares its stream/datagram send semantics and its
//! [`transport::TransportConfig`], which is what
//! [`transport::HasTransportConfig`] carries.
//!
//! ```rust
//! use grafton_visca::command::CommandKind;
//! use grafton_visca::transport::{
//!     BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
//! };
//! use grafton_visca::Error;
//! use std::time::Duration;
//!
//! struct MyTransport {
//!     // Your transport state, plus the configuration the runtime reads.
//!     config: TransportConfig,
//! }
//!
//! impl BlockingTransport for MyTransport {
//!     fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error> {
//!         // Write the framed bytes; `kind` selects command vs inquiry framing.
//!         let _ = (bytes, kind);
//!         Ok(())
//!     }
//!
//!     fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
//!         // Read into the caller's buffer. `Ok(0)` reports EOF.
//!         let _ = dst;
//!         Ok(0)
//!     }
//!
//!     fn recv_into_with_timeout(
//!         &mut self,
//!         dst: &mut [u8],
//!         timeout: Duration,
//!     ) -> Result<usize, Error> {
//!         // Prefer an OS-level socket timeout. An expired idle timeout is
//!         // reported as `Error::Timeout`, which the runtime reads as
//!         // "this read produced no frames".
//!         let _ = (dst, timeout);
//!         Err(Error::Timeout)
//!     }
//!
//!     fn send_semantics(&self) -> SendSemantics {
//!         SendSemantics::Stream
//!     }
//! }
//!
//! impl HasTransportConfig for MyTransport {
//!     fn transport_config(&self) -> &TransportConfig {
//!         &self.config
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
//! ```rust
//! # #[cfg(feature = "runtime-tokio")]
//! async fn with_tokio() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{
//!         camera::{profiles::PtzOpticsG2, Connect},
//!         runtime::TokioRuntime,
//!     };
//!
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!     let _ = camera;
//!     session.close().await
//! }
//!
//! # #[cfg(feature = "runtime-smol")]
//! async fn with_smol() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{
//!         camera::{profiles::PtzOpticsG2, Connect},
//!         runtime::SmolRuntime,
//!     };
//!
//!     let runtime = SmolRuntime::new();
//!     let session = Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!     let camera = session.camera::<PtzOpticsG2>()?;
//!     let _ = camera;
//!     session.close().await
//! }
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
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn position_units(
//!     camera: &grafton_visca::blocking::Camera<'_, grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{units::Degrees, SpeedLevel};
//!
//!     // Work in degrees (recommended)
//!     camera
//!         .pan_tilt()
//!         .absolute(Degrees(45.0), Degrees(-15.0), SpeedLevel::Medium)?
//!         .settled()?;
//!
//!     // Stop all movement
//!     camera.pan_tilt().stop()?.applied()?;
//!
//!     // Move to home position
//!     camera.pan_tilt().home()?.settled()
//! }
//! ```
//!
//! ## Timeout Configuration
//!
//! Configure timeouts per command category based on your network and camera:
//!
//! ```rust
//! use grafton_visca::timeout::TimeoutConfig;
//! use std::time::Duration;
//!
//! let timeouts = TimeoutConfig::builder()
//!     .ack_timeout(Duration::from_millis(300))
//!     .quick_timeout(Duration::from_secs(3))
//!     .movement_timeout(Duration::from_secs(20))
//!     .preset_timeout(Duration::from_secs(60))
//!     .build();
//!
//! // For the async facade
//! # #[cfg(feature = "runtime-tokio")]
//! async fn configured_async(timeouts: TimeoutConfig) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::camera::{profiles::PtzOpticsG2, CameraConfig};
//!     use grafton_visca::runtime::TokioRuntime;
//!
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!         .timeouts(timeouts)
//!         .open_async(runtime)
//!         .await?;
//!     session.close().await
//! }
//!
//! // For the blocking facade
//! # #[cfg(feature = "blocking")]
//! fn configured_blocking(timeouts: TimeoutConfig) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::camera::profiles::PtzOpticsG2;
//!     use grafton_visca::blocking::CameraConfig;
//!
//!     let session = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
//!         .timeouts(timeouts)
//!         .open()?;
//!     session.close()
//! }
//! # let _ = timeouts;
//! ```
//!
//! ## Command Cancellation (Async)
//!
//! Cancel specific commands or entire socket operations:
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn cancel_a_zoom(
//!     camera: &grafton_visca::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use std::time::Duration;
//!
//!     // Submit one typed operation and retain its owner-backed lifecycle handle.
//!     let operation = camera.zoom().tele().await?;
//!     let cancellation = operation.cancel().await?;
//!     let outcome = cancellation.outcome(Duration::from_secs(2)).await?;
//!     println!("cancellation outcome: {outcome:?}");
//!     Ok(())
//! }
//! ```
//!
//! Queued commands can always be removed locally. Cancelling a command that has
//! already been sent requires profile support for the standard VISCA socket-cancel
//! command and otherwise fails with [`Error::NotSupported`]. For bounded continuous
//! movement, send the relevant STOP command and await its application.
//!
//! A refused cancellation is not cancellation intent: the owner leaves the
//! original request scheduled and able to complete, so `cancel` consumes the
//! handle only when it succeeds. A refusal returns [`CancelRejected`], which
//! carries the handle back — take it with `into_operation()` to keep waiting or
//! to retry. `?` in a function returning [`Error`] still works and detaches the
//! handle, exactly as dropping it does.
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn stop_a_zoom_the_profile_cannot_cancel(
//!     camera: &grafton_visca::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     let operation = camera.zoom().tele().await?;
//!     let operation = match operation.cancel().await {
//!         Ok(cancellation) => {
//!             cancellation.detach();
//!             return Ok(());
//!         }
//!         // The G2 has no socket-cancel; the handle comes back untouched.
//!         Err(rejected) => match rejected.into_operation() {
//!             Some(operation) => operation,
//!             None => return Ok(()),
//!         },
//!     };
//!     // The recourse that actually ends movement on such a profile.
//!     camera.zoom().stop().await?.applied().await?;
//!     operation.detach();
//!     Ok(())
//! }
//! ```
//!
//! ## Movement Completion Tracking
//!
//! Observe or wait for exactly selected physical axes with
//! [`camera::MotionQuery`] and [`camera::IdleWait`]. Unselected axes are never
//! queried:
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn observe_motion(
//!     camera: &grafton_visca::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::camera::{IdleWait, MotionQuery};
//!     use grafton_visca::AffectedAxes;
//!     use std::time::Duration;
//!
//!     let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
//!     // `is_moving()` takes no argument and samples `AffectedAxes::MOVEMENT`;
//!     // `is_moving_axes` is the axis-selecting form.
//!     let moving = camera.motion().is_moving_axes(MotionQuery::new(axes)).await?;
//!     camera
//!         .motion()
//!         .wait_until_idle(IdleWait::new(axes, Duration::from_secs(30)))
//!         .await?;
//!     let _ = moving;
//!     Ok(())
//! }
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error types for all VISCA error conditions:
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn classify_errors(
//!     camera: &grafton_visca::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) {
//!     use grafton_visca::{units::Degrees, Error, SpeedLevel};
//!
//!     match camera
//!         .pan_tilt()
//!         .absolute(Degrees(180.0), Degrees(0.0), SpeedLevel::Medium)
//!         .await
//!     {
//!         Ok(_) => println!("Position set successfully"),
//!         Err(Error::SyntaxError) => println!("Position out of range"),
//!         Err(Error::CommandNotExecutable) => println!("Camera busy or powered off"),
//!         Err(Error::CommandBufferFull) => {
//!             // This error is automatically retried by the runtime
//!             println!("Camera buffer full, command will retry");
//!         }
//!         Err(e) => println!("Other error: {e}"),
//!     }
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

/// The single row table behind all three noun facades.
pub(crate) mod noun_table;

/// Cross-surface parity gate for the table-driven noun facades.
#[cfg(test)]
mod noun_parity;

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

mod stop_request;

/// Bounded owner metrics and diagnostic subscriptions.
pub mod observability;

/// Read-only target-local state projections committed by the owner.
pub mod state_cache;

pub use observability::{
    DiagnosticCancellation, DiagnosticDeadline, DiagnosticEvent, DiagnosticId,
    DiagnosticIgnoreReason, DiagnosticLane, DiagnosticOutcome, DiagnosticPhase, DiagnosticResponse,
    DiagnosticSubscription, MetricsSnapshot, SessionStatus,
};
pub use state_cache::{PanTiltLimitUpdate, StateCache, StateEntry, StateKey, StateValue};

mod session_config;
pub use session_config::SessionConfig;

mod outcome;
pub use outcome::{CancelRejected, CancellationOutcome};

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
pub use async_session::{Camera, CameraSession, Session};
#[cfg(feature = "async")]
pub mod session {
    //! Profile-generic async session facade.
    #[cfg(feature = "transport-serial-tokio")]
    pub use crate::camera::SerialConnectBuilder;
    pub use crate::camera::{
        CameraConfig, Connect, ConnectBuilder, TcpConnectBuilder, UdpConnectBuilder,
    };
    pub use crate::{
        async_session::Camera, async_session::CameraSession, async_session::Session, SessionConfig,
    };
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

/// Compile gate for the Rust snippets in `docs/migration_2_0.md`.
///
/// The migration guide carries the canonical scoped stop-on-exit guard and the
/// fresh-session recovery shape. Both are patterns callers copy verbatim, so
/// they are compiled here for the same reason the README is: a snippet that
/// only reads correctly is a snippet that drifts. The same feature gating rule
/// applies — snippets needing more than the default feature set carry their own
/// `#[cfg(feature = "...")]`.
#[cfg(all(doctest, feature = "blocking"))]
#[doc = include_str!("../docs/migration_2_0.md")]
mod migration_guide_snippets {}

/// Compile gate for the Rust snippets in `docs/usage_2_0.md`.
///
/// `usage_2_0.md` is the primary construction and calling-convention guide, so
/// it is the page most likely to be copied verbatim and the page whose drift
/// costs the most. It is compiled here for the same reason the README and the
/// migration guide are, and under the same rule: snippets needing more than the
/// default feature set carry their own `#[cfg(feature = "...")]`.
#[cfg(all(doctest, feature = "blocking"))]
#[doc = include_str!("../docs/usage_2_0.md")]
mod usage_guide_snippets {}
