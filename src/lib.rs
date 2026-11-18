//! # grafton-visca
//!
//! Rust library for VISCA over IP protocol to control PTZ cameras.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(unused_crate_dependencies))]
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

//! ## What is VISCA?
//!
//! VISCA (Video System Control Architecture) is a protocol developed by Sony for controlling Ptz cameras
//! commonly used in robotics, broadcasting, video conferencing, and surveillance applications. This crate
//! implements VISCA over IP, allowing you to control networked Ptz cameras from Rust applications.
//!
//! ## Features
//!
//! - **Owner-backed Camera API**: `CameraSession::connect_tcp` and noun accessors across the blocking and async facades
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
//! - **Lifecycle-Exact Operation Handles**: Exact applied waits and profile-selected
//!   protocol-settlement waits, cancellation, and detach across blocking and async APIs
//! - **Serialization Support**: Optional serde/schemars integration for all value types
//!
//! ## 2.0 API Shape
//!
//! The primary API is camera-first:
//!
//! - Use `CameraSession::connect_tcp`, `connect_udp`, or `connect_serial` for a single camera.
//! - Use [`config::CameraConfig`] when a standard transport needs explicit
//!   timeout, retry, keepalive, camera ID, or serial settings.
//! - Use accessor-style controls such as `camera.power().on()` and
//!   `camera.pan_tilt().position()` for normal operation.
//! - Use `r#async::Session::open` or `blocking::Session::open` when you already
//!   own a custom transport and need to attach it to the owner.
//! - Use [`crate::types::UnitInterval`] for normalized `0.0..=1.0` control values and
//!   [`crate::types::IndividualCameraId`] for configured VISCA camera addresses.
//! - Use the typed [`crate::request::Request`] and [`crate::request::Inquiry`] contracts for built-in and custom
//!   requests, and [`wire::ResponseParser`] for typed inquiry responses.
//!
//! ## Terminology
//!
//! The 2.0 lifecycle vocabulary is precise: the same words appear in these APIs,
//! in the design notes, and in the phase/cancellation transition table in
//! `docs/architecture_2_0.md`. This section defines each term once, consistent
//! with the current behavior (issue #671).
//!
//! **Ownership**
//!
//! - **Session** — the sole owner of one transport, its framer/envelope state,
//!   the protocol engine, the observer registry, the target-local state caches,
//!   and the session metrics. A blocking session owns one native worker thread;
//!   an async session owns one detached actor task. Every camera view is a
//!   view onto this one owner, never a second connection.
//! - **Target** — one registered camera with an [`crate::types::IndividualCameraId`] plus a validated
//!   profile. Targets are registered before the owner starts and are immutable
//!   while it runs.
//! - **Prepared** — a request that passed domain, profile, range, encoding,
//!   routing, classification, and settlement selection and now carries every
//!   immutable datum the owner needs. Preparation is pure and shared by the
//!   blocking and async facades; a preparation failure never touches
//!   session state.
//! - **Admitted** — the owner inserted a prepared request into engine state,
//!   assigned its private request id, and reserved its bounded capacity.
//!   Admission does *not* mean any bytes were written, and admission over
//!   capacity fails immediately rather than waiting.
//!
//! **Completion**
//!
//! - **Applied** — the exact submitted command reached its successful VISCA
//!   terminal response. Plain commands normally finish at applied; a raw plain
//!   `NoReply` command instead reports only a successful local transport write
//!   and cannot create an operation handle. Typed operations expose applied
//!   through `applied()`.
//! - **Targeted operation** — an operation with a meaningful target state
//!   (home, absolute/relative move, target position, preset recall). It can be
//!   observed as applied and as settled.
//! - **Applied-only operation** — an operation with no meaningful target state:
//!   a continuous drive, a STOP, or an instantaneous trigger. It exposes
//!   `applied()` and has no `settled()`.
//! - **Settled** — a targeted operation was applied *and* meets the
//!   profile-selected protocol settlement condition: a profile-declared
//!   completion-is-settled signal, or two affected-axis position samples within
//!   tolerance. This is protocol evidence of target rest; it does not
//!   independently prove that physical motion has ended.
//!   Settlement is a caller wait, not an engine phase.
//!
//! **Deadline classes** (three independent owners; every default request
//! observer is sized from immutable retry policy)
//!
//! - **Scheduler deadline** — an engine-owned deadline for ACK, completion,
//!   inquiry reply, cancellation response, retry eligibility, pacing, cooldown,
//!   or ambiguity quarantine. It is stamped at preparation and drives the
//!   protocol state machine.
//! - **Observer deadline** — the caller's wait bound. Ordinary command and
//!   inquiry calls, plus operation `applied()`/`settled()`, use the larger of
//!   their governing completion/reply/settlement deadline and total retry
//!   budget, so a crate-authorized retry remains observable. The
//!   `applied_with_timeout` / `settled_with_timeout` forms replace *only* this
//!   deadline. An observer timeout returns [`Error::ObservationTimeout`] and
//!   ends only that wait: the request keeps running, the handle keeps
//!   observing and can wait again, and nothing sends cancellation or changes
//!   a scheduler deadline. Unlike a terminal [`Error::Timeout`], it is never
//!   retryable: resubmitting would duplicate a request that is still live.
//! - **Transport timeout** — the driver-level bound on one read or write
//!   (`read_timeout`/`write_timeout`), independent of both of the above.
//!
//! **Cancellation**
//!
//! - **Cancel intent** — the owner deliberately accepted a request to cancel one
//!   exact admitted operation. It suppresses every later retry but does not by
//!   itself prove cancellation. A handle has at most one intent: `cancel()`
//!   records it and waits for its conclusion, and calling `cancel()` again
//!   observes the same intent instead of sending another.
//! - **Cancelled** — the engine proved the operation cannot later succeed: it
//!   was removed before transmission, a conclusive rejection left no executing
//!   attempt with retry suppressed, or the camera returned the socket-specific
//!   protocol-cancel terminal. Reported as [`crate::operation::CancellationOutcome::Cancelled`].
//! - **Completed** — the original operation succeeded before a confirmed
//!   cancellation could win, reported as [`crate::operation::CancellationOutcome::Completed`].
//!   Cancellation never claims physical motion stopped.
//! - **Cancellation unconfirmed** — a cancellation could not be assigned a
//!   socket or conclusively resolved before its ambiguity deadline. This is
//!   [`Error::CancellationUnconfirmed`], never `Cancelled`. Its per-request
//!   sibling [`Error::UnsequencedCommandUnconfirmed`] is the default outcome
//!   when a *sent* unsequenced command on a raw-VISCA envelope loses its ACK or
//!   completion correlation with no cancel involved: it fails that one command
//!   on a still-live session
//!   ([`requires_new_session`](Error::requires_new_session) is `false`) and
//!   quarantines the correlation until the ambiguity deadline so a late reply
//!   cannot misbind (issue #671).
//! - **Observation** — every operation wait (`applied`, `settled`, `cancel`)
//!   borrows its handle, and the handle caches the authoritative results it
//!   receives. A repeated wait answers from the cache, and a wait that times
//!   out or is dropped releases only itself.
//! - **Detach** — relinquish the sole observation right without changing any
//!   protocol state. Dropping a handle is exactly detach; it never cancels or
//!   stops hardware.
//!
//! **Termination**
//!
//! - **Close** — consume the session, signal shutdown, and wait for the owner to
//!   drain and drop its transport. It is the deterministic teardown barrier for
//!   reopening an endpoint, distinct from a peer-initiated
//!   [`Error::ConnectionClosed`] and from the deliberate
//!   [`Error::RuntimeShutdown`].
//! - **Poison** — irreversible termination of a stream session after framing or
//!   write progress became unknowable (a stream write failure or an
//!   unrecoverable framing loss). A poisoned session is terminal, reports
//!   [`Error::StreamPoisoned`]
//!   ([`requires_new_session`](Error::requires_new_session) is `true`), and
//!   shares no state with any session built afterward. By default an
//!   unsequenced command on a raw-VISCA envelope that merely loses its
//!   ACK/completion does *not* poison (see *cancellation unconfirmed* above);
//!   the whole-session poison is restored only behind the opt-in
//!   [`crate::config::SessionConfig::with_strict_unconfirmed_poison`] policy.
//!
//! Recovery from a poisoned or closed session builds a fresh session from the
//! reused configuration, starts with an unknown state cache, re-queries camera
//! state, and then deliberately restores desired state; the owner never
//! resubmits automatically. Use
//! [`requires_new_session`](Error::requires_new_session) for the reconnect
//! decision, and see `examples/recovery.rs` for the end-to-end supervisor
//! pattern.
//!
//! ```no_run
//! # #[cfg(feature = "blocking")]
//! # fn terminology_walkthrough() -> Result<(), grafton_visca::Error> {
//! use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
//! use std::time::Duration;
//!
//! // A session owns the transport; the camera is a view onto its one owner.
//! let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
//! let camera = session.camera();
//!
//! // `home` is targeted: `settled` waits past applied for protocol settlement.
//! camera.pan_tilt().home()?.settled()?;
//!
//! // `stop` is applied-only: it has an applied wait and no settled state. The
//! // `_with_timeout` form overrides only the observer deadline.
//! camera
//!     .zoom()
//!     .stop()?
//!     .applied_with_timeout(Duration::from_secs(2))?;
//!
//! session.close()
//! # }
//! ```
//!
//! ## Typed Request Extensions
//!
//! The final request contract classifies every value at the type level. Implement
//! [`crate::request::Request`] with one closed [`request`] class, then implement [`crate::request::Inquiry`] or
//! [`crate::request::OperationCommand`] only when that class requires it. [`crate::request::PlainCommand`] is
//! provided automatically for every plain request. Built-ins under [`command`]
//! show the corresponding targeted, applied-only, plain, and inquiry shapes.
//!
//! Request values carry their timeout, retry, and control-policy classes. Camera
//! targets, completion kinds, and retry behavior cannot be overridden at
//! submission time. Ordinary submission QoS can: see [Submission priority] below.
//! Supported `ViscaInquiry` derives implement this typed inquiry contract;
//! downstream derives use the conservative inquiry retry class.
//!
//! ## Submission priority
//!
//! [`crate::request::SubmissionClass`] selects ordinary scheduling priority. The owner
//! dispatches ready work from the highest occupied class first and FIFO within
//! a class, so the class only decides which **queued** request is written next;
//! it never interrupts, cancels, or reorders a request already on the wire.
//!
//! Ordinary control traffic uses [`crate::request::SubmissionClass::Normal`] and
//! direct movement uses [`crate::request::SubmissionClass::User`]. Typed STOPs
//! and owner-issued cancellation use the urgent lane independently of caller QoS.
//! Two camera-view routes select ordinary submission QoS:
//!
//! - Per handle: `set_submission_class(Some(class))` on a camera view, its
//!   single-camera session. Every later submission
//!   from *that handle* uses `class`, including the ones its noun accessors
//!   make. An urgent request ignores it.
//! - Derived view: `with_submission_class(class)` returns another camera view
//!   whose commands, inquiries, operations, and noun methods all use `class`.
//!   The source view is unchanged, and an urgent request still ignores the
//!   selected ordinary class.
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! # async fn prioritize(
//! #     camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! #     command: &impl grafton_visca::request::PlainCommand,
//! # ) -> Result<(), grafton_visca::Error> {
//! use grafton_visca::request::SubmissionClass;
//!
//! // Keep a telemetry poller out of the operator's way.
//! let mut poller = camera.clone();
//! poller.set_submission_class(Some(SubmissionClass::Background));
//! let _zoom = poller.zoom().position().await?; // queued behind operator input
//! poller.pan_tilt().stop().await?.applied().await?; // still urgent
//!
//! // Raise one ordinary command for direct user interaction.
//! camera
//!     .with_submission_class(SubmissionClass::User)
//!     .execute(command)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! [Submission priority]: #submission-priority
//!
//! Runtime-only camera models start with
//! [`capabilities::Capabilities::runtime_baseline`] and must explicitly provide
//! all protocol facts through [`crate::profiles::ProfileSpec::builder`]. Built-in and downstream
//! compile-time profiles expose fact-only [`crate::profiles::CompileTimeProfile`] implementations
//! and lower through [`crate::profiles::ProfileSpec::from_compile_time`] to the same validated
//! immutable representation.
//!
//! ## Serialization Support
//!
//! All public value types support optional serialization through feature-gated `serde` and `schemars` derives:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "2.0.0", features = ["serde", "schemars"] }
//! ```
//!
//! With these features enabled, you can serialize/deserialize all value types directly:
//!
//! ```rust
//! # #[cfg(feature = "serde")] {
//! use grafton_visca::{types::PanSpeed, types::SpeedLevel, types::ZoomPosition};
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
//! use grafton_visca::config::TransportOptions;
//! use grafton_visca::profiles::ProfileId;
//!
//! // Serialize camera profile
//! let profile = ProfileId::PtzOpticsG2;
//! let json = serde_json::to_string(&profile).unwrap();
//! assert_eq!(json, "\"ptzoptics_g2\"");
//!
//! // Serialize transport configuration
//! let transport = TransportOptions::tcp("192.168.0.110:5678".to_string());
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
//! ### Checked Wire Parameters
//! Parameter types cover the wire values used by the built-in profiles. Request
//! preparation enforces the selected model's range. FR7 pan/tilt speeds above
//! 24 also require session-confirmed Extended speed-step mode.
//! ```rust
//! use grafton_visca::{types::PanSpeed, types::Speed, types::SpeedLevel, types::ZoomSpeed};
//!
//! // All range types expose MIN/MAX constants for validation
//! assert_eq!(PanSpeed::MIN.value(), 0);
//! assert_eq!(PanSpeed::MAX.value(), 50);
//!
//! // Validated constructors provide clear error messages
//! let speed = PanSpeed::new(15).expect("0..=50 is in the wire domain");
//! assert_eq!(speed.value(), 15);
//! let error = PanSpeed::new(51).expect_err("51..=255 is outside the wire domain");
//! println!("{error}");
//!
//! // A level is resolved against the selected profile during preparation.
//! let zoom: Speed<ZoomSpeed> = SpeedLevel::Fast.into();
//! assert_eq!(zoom, Speed::Level(SpeedLevel::Fast));
//! ```
//!
//! ### Profile-Based Compile-Time Safety
//! Camera profiles carry model-specific limits and capabilities at compile time:
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn absolute_move() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{
//!         blocking::CameraSession, profiles::PtzOpticsG2, types::PanTiltPosition,
//!         types::SpeedLevel,
//!     };
//!
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
//!     let camera = session.camera();
//!     camera
//!         .pan_tilt()
//!         .absolute(
//!             PanTiltPosition::from_degrees(45.0, 10.0)?,
//!             SpeedLevel::Medium,
//!         )?
//!         .settled()?;
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
//!     use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
//!
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
//!     let camera = session.camera();
//!     camera.focus().set_lock(true)?;
//!     camera.focus().set_lock(false)?;
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
//!     use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
//!
//!     // Create one owner-backed session with an inherent constructor.
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
//!     let camera = session.camera();
//!
//!     // Quick starts are read-only by default.
//!     let power_status = camera.power().status()?;
//!     let zoom = camera.zoom().position()?;
//!     println!("Power: {power_status:?}, zoom: 0x{:04X}", zoom.value());
//!
//!     session.close()
//! }
//! ```
//!
//! ### Async Example with Multi-Runtime Support
//! ```rust
//! # #[cfg(feature = "runtime-tokio")]
//! async fn quick_start() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{profiles::PtzOpticsG2, r#async::CameraSession, runtime::TokioRuntime};
//!
//!     // Pass the runtime last when constructing an async session.
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
//!     let camera = session.camera();
//!
//!     let power_status = camera.power().status().await?;
//!     let zoom = camera.zoom().position().await?;
//!     println!("Power: {power_status:?}, zoom: 0x{:04X}", zoom.value());
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
//! grafton-visca = { version = "2.0.0", features = ["runtime-tokio", "runtime-smol"] }
//! ```
//!
//! ```rust
//! # #[cfg(feature = "runtime-smol")]
//! fn quick_start() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{profiles::PtzOpticsG2, r#async::CameraSession, runtime::SmolRuntime};
//!
//!     smol::block_on(async {
//!         let runtime = SmolRuntime::new();
//!         let session =
//!             CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
//!         let camera = session.camera();
//!
//!         let power_status = camera.power().status().await?;
//!         println!("Power: {power_status:?}");
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
//!         config::CameraConfig, profiles::PtzOpticsG2, runtime::SmolRuntime,
//!         transport::{BufferConfig, IoBounds, TcpConnect, TcpKeepaliveConfig},
//!     };
//!     use std::time::Duration;
//!
//!     smol::block_on(async {
//!         let config = CameraConfig::<PtzOpticsG2>::tcp_configured(
//!             "192.168.0.110",
//!             TcpConnect::default().with_keepalive(Some(
//!                 TcpKeepaliveConfig::new(Duration::from_secs(30)),
//!             ))?,
//!             IoBounds::default().with_buffer_config(BufferConfig::for_tcp())?,
//!         );
//!
//!         let session =
//!             grafton_visca::r#async::CameraSession::connect(&config, SmolRuntime::new()).await?;
//!         let camera = session.camera();
//!
//!         let power_status = camera.power().status().await?;
//!         println!("Power: {power_status:?}");
//!
//!         session.close().await
//!     })
//! }
//! ```
//!
//! ### Bounded Operation Handles
//!
//! Ordinary noun methods are the simplest command-completion API. Use `submit`
//! when one command needs an exact deadline, cancellation, detach, or a
//! profile-selected protocol-settlement wait:
//!
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn bounded_moves() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
//!     use std::time::Duration;
//!
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
//!     let camera = session.camera();
//!     camera
//!         .pan_tilt()
//!         .home()?
//!         .settled_with_timeout(Duration::from_secs(20))?;
//!     camera
//!         .zoom()
//!         .stop()?
//!         .applied_with_timeout(Duration::from_secs(2))?;
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
//! `SonyFr7`, and `GenericVisca`. Profiles are selected at construction time
//! with `CameraSession::connect_tcp` or `CameraConfig` and then drive compile-time capability
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
//!     let sony = CameraSession::<SonyFr7>::connect_udp("192.168.0.110")?;
//!     sony.camera().nd_filter().set_mode(NdFilterMode::Preset)?;
//!
//!     let g2 = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.111")?;
//!     let g2_camera = g2.camera();
//!     // g2_camera.nd_filter().set_mode(NdFilterMode::Preset)?;
//!     // ^ Compile error: G2 has no ND filter capability
//!     let _ = g2_camera;
//!
//!     sony.close()?;
//!     g2.close()
//! }
//! ```
//!
//! Runtime discovery is available through `camera.capabilities()`. Typed
//! optional controls use
//! separate support markers, so the public API exposes only documented support:
//! `SonyFr7` has typed ND filter and variable speed controls, plus separate
//! exposure, white-balance, image, zoom and preset-setting families.
//! For example, these settings configure white balance and a later preset
//! recall without starting a recall:
//!
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn configure_fr7(
//!     camera: &grafton_visca::blocking::Camera<grafton_visca::profiles::SonyFr7>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{
//!         types::Kelvin, types::PresetNumber, types::PresetSeparateMode,
//!         types::PresetSpeedSelection,
//!     };
//!     camera
//!         .white_balance()
//!         .set_mode(grafton_visca::types::WhiteBalanceMode::Preset)?;
//!     camera
//!         .white_balance()
//!         .set_preset_white(Kelvin::new(5600)?)?;
//!     let preset = PresetNumber::new(0)?;
//!     camera
//!         .presets()
//!         .set_speed_selection(PresetSpeedSelection::Separate)?;
//!     camera
//!         .presets()
//!         .set_separate_mode(preset, PresetSeparateMode::Duration)?;
//!     camera
//!         .presets()
//!         .set_separate_duration(preset, std::time::Duration::from_millis(2_500))?;
//!     Ok(())
//! }
//! ```
//!
//! Runtime-selected `Camera<profiles::AnyProfile>` callers use the same noun
//! methods and inspect `camera.capabilities().supports(surface)` for optional
//! controls. Domain records describe their own supported ranges. Built-in
//! PTZOptics profiles are not marked for typed Motion Sync from the current
//! model capability specs.
//!
//! ## Transport Implementation
//!
//! The library provides transport traits that you can implement for any
//! communication method. The receive side reads into a caller-provided buffer
//! and returns a [`transport::ReceiveOutcome`]: the byte count, and whether a
//! datagram fitted. A transport never allocates per frame. Zero bytes means
//! the peer closed the connection; an idle timeout is *no data*, not a fault. A
//! transport also declares its stream/datagram send semantics and its
//! [`transport::IoBounds`], which is what
//! [`transport::HasTransportConfig`] carries.
//!
//! ```rust
//! use grafton_visca::Error;
//! use grafton_visca::{
//!     transport::BlockingTransport, transport::HasTransportConfig, transport::ReceiveOutcome,
//!     transport::SendSemantics, transport::IoBounds,
//! };
//! use std::time::Duration;
//!
//! struct MyTransport {
//!     // Your transport state, plus the configuration the runtime reads.
//!     config: IoBounds,
//! }
//!
//! impl BlockingTransport for MyTransport {
//!     fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
//!         // Complete the owner-framed write within `timeout`.
//!         let _ = (bytes, timeout);
//!         Ok(())
//!     }
//!
//!     fn recv_into_with_timeout(
//!         &mut self,
//!         dst: &mut [u8],
//!         timeout: Duration,
//!     ) -> Result<ReceiveOutcome, Error> {
//!         // Prefer an OS-level socket timeout. An expired idle timeout is
//!         // reported as `Error::io_timeout()`, which the runtime reads as
//!         // "this read produced no frames".
//!         let _ = (dst, timeout);
//!         Err(Error::io_timeout())
//!     }
//! }
//!
//! impl HasTransportConfig for MyTransport {
//!     fn identity(&self) -> grafton_visca::transport::TransportIdentity<'_> {
//!         grafton_visca::transport::TransportIdentity::Custom {
//!             semantics: SendSemantics::Stream,
//!             addressing: grafton_visca::transport::AddressingMode::Ip,
//!         }
//!     }
//!
//!     fn io_bounds(&self) -> &IoBounds {
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
//! Pass the runtime explicitly to `CameraSession::connect_tcp` or `connect` when multiple
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
//! grafton-visca = { version = "2.0.0", features = ["runtime-tokio"] }
//! grafton-visca = { version = "2.0.0", features = ["runtime-smol"] }
//!
//! # Multiple runtimes (choose executor at construction time):
//! grafton-visca = { version = "2.0.0", features = ["runtime-tokio", "runtime-smol"] }
//! ```
//!
//! Pass the runtime explicitly through `CameraSession::connect_tcp` or `connect`; standard
//! network construction returns the canonical owner-backed `CameraSession<P>`:
//!
//! ```rust
//! # #[cfg(feature = "runtime-tokio")]
//! async fn with_tokio() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{profiles::PtzOpticsG2, r#async::CameraSession, runtime::TokioRuntime};
//!
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
//!     let camera = session.camera();
//!     let _ = camera;
//!     session.close().await
//! }
//!
//! # #[cfg(feature = "runtime-smol")]
//! async fn with_smol() -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{profiles::PtzOpticsG2, r#async::CameraSession, runtime::SmolRuntime};
//!
//!     let runtime = SmolRuntime::new();
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
//!     let camera = session.camera();
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
//! - 2D noise-reduction mode and 2D/3D level inquiries and controls (independently profile-gated)
//! - Image flip and other effects
//!
//! ## Position Units
//!
//! Typed pan/tilt input uses checked finite degrees. The selected profile
//! supplies the conversion to its wire coordinates:
//!
//! ```rust
//! # #[cfg(feature = "blocking")]
//! fn position_units(
//!     camera: &grafton_visca::blocking::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::{types::PanTiltPosition, types::SpeedLevel};
//!
//!     // Work in degrees (recommended)
//!     camera
//!         .pan_tilt()
//!         .absolute(
//!             PanTiltPosition::from_degrees(45.0, -15.0)?,
//!             SpeedLevel::Medium,
//!         )?
//!         .settled()?;
//!
//!     // Stop all movement
//!     camera.pan_tilt().stop()?.applied()?;
//!
//!     // Move to home position
//!     camera.pan_tilt().home()?.settled()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Timeout Configuration
//!
//! Configure validated timeout overrides per command category:
//!
//! ```rust
//! use grafton_visca::config::OperationalTuning;
//! use std::time::Duration;
//!
//! let tuning = OperationalTuning::new()
//!     .with_ack_timeout(Duration::from_millis(600))
//!     .with_quick_timeout(Duration::from_secs(6))
//!     .with_movement_timeout(Duration::from_secs(40))
//!     .with_preset_timeout(Duration::from_secs(90))
//!     .with_long_running_timeout(Duration::from_secs(360))
//!     .with_network_timeout(Duration::from_secs(6));
//!
//! // For the async facade
//! # #[cfg(feature = "runtime-tokio")]
//! async fn configured_async(tuning: OperationalTuning) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::runtime::TokioRuntime;
//!     use grafton_visca::{config::CameraConfig, profiles::PtzOpticsG2};
//!
//!     let runtime = TokioRuntime::from_current()?;
//!     let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110").with_tuning(tuning);
//!     let session = grafton_visca::r#async::CameraSession::connect(&config, runtime).await?;
//!     session.close().await
//! }
//!
//! // For the blocking facade
//! # #[cfg(feature = "blocking")]
//! fn configured_blocking(tuning: OperationalTuning) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::config::CameraConfig;
//!     use grafton_visca::profiles::PtzOpticsG2;
//!
//!     let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110").with_tuning(tuning);
//!     let session = grafton_visca::blocking::CameraSession::connect(&config)?;
//!     session.close()
//! }
//! # let _ = tuning;
//! ```
//!
//! ## Command Cancellation (Async)
//!
//! Cancel specific commands or entire socket operations:
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn cancel_a_zoom(
//!     camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use std::time::Duration;
//!
//!     // Submit one typed operation and retain its owner-backed lifecycle handle.
//!     let mut operation = camera.zoom().tele().await?;
//!     let outcome = operation
//!         .cancel_with_timeout(Duration::from_secs(2))
//!         .await?;
//!     println!("cancellation outcome: {outcome:?}");
//!     Ok(())
//! }
//! ```
//!
//! Queued commands can always be removed locally. Cancelling a command that has
//! already been sent requires profile support for the standard VISCA socket-cancel
//! command and otherwise fails with [`Error::CancellationNotSupported`]. For bounded continuous
//! movement, send the relevant STOP command and await its application.
//!
//! A refused cancellation is not cancellation intent: the owner leaves the
//! original request scheduled and able to complete. Because `cancel` borrows
//! the handle, a refusal is a plain error and the handle keeps observing the
//! operation.
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn stop_a_zoom_the_profile_cannot_cancel(
//!     camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::Error;
//!
//!     let mut operation = camera.zoom().tele().await?;
//!     match operation.cancel().await {
//!         Ok(outcome) => {
//!             println!("cancellation outcome: {outcome:?}");
//!             Ok(())
//!         }
//!         // The G2 has no socket-cancel. The recourse that actually ends
//!         // movement on such a profile is an applied STOP, and the same
//!         // handle still observes the original zoom.
//!         Err(Error::CancellationNotSupported) => {
//!             camera.zoom().stop().await?.applied().await?;
//!             operation.applied().await
//!         }
//!         Err(error) => Err(error),
//!     }
//! }
//! ```
//!
//! ## Movement Completion Tracking
//!
//! Observe or wait for the protocol position samples of exactly selected axes with
//! [`types::MotionQuery`] and [`types::IdleWait`]. Unselected axes are never
//! queried:
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! async fn observe_motion(
//!     camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) -> Result<(), grafton_visca::Error> {
//!     use grafton_visca::types::AffectedAxes;
//!     use grafton_visca::{types::IdleWait, types::MotionQuery};
//!     use std::time::Duration;
//!
//!     let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
//!     // `MotionQuery::default()` samples `AffectedAxes::MOVEMENT`; this query
//!     // selects pan/tilt and zoom only.
//!     let moving = camera.motion().is_moving(MotionQuery::new(axes)).await?;
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
//!     camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::PtzOpticsG2>,
//! ) {
//!     use grafton_visca::{types::PanTiltPosition, types::SpeedLevel, Error};
//!
//!     let position = PanTiltPosition::from_degrees(180.0, 0.0).expect("finite literals");
//!     let applied = match camera
//!         .pan_tilt()
//!         .absolute(position, SpeedLevel::Medium)
//!         .await
//!     {
//!         Ok(mut operation) => operation.applied().await,
//!         Err(error) => Err(error),
//!     };
//!     match applied {
//!         Ok(()) => println!("Position set successfully"),
//!         // The camera refused the command before accepting it; nothing moved.
//!         Err(Error::SyntaxError) => println!("Position out of range"),
//!         Err(Error::CommandNotExecutable) => println!("Camera busy or powered off"),
//!         // The camera accepted the command and then failed it: it may have
//!         // moved part of the way, so reconcile before trying again.
//!         Err(Error::CommandFailedAfterAck { source, .. }) => {
//!             println!("Move failed after it started ({source}); re-read the position");
//!         }
//!         Err(e) => println!("Other error: {e}"),
//!     }
//! }
//! ```

extern crate self as grafton_visca;

// Enabling the executor through `async` lets the runtime-free async testkit
// use it without linking an executor into blocking-only `test-utils` builds.
#[cfg(all(feature = "async", not(feature = "test-utils")))]
use async_executor as _;

pub use grafton_visca_macros::{value, value_enum, ViscaInquiry};

/// Implementation details used by the value attributes and inquiry derive.
///
/// This is not a supported public API. It exists so macro expansions can refer
/// to optional dependencies through the defining crate even when a downstream
/// crate renames dependencies or does not depend on them directly.
#[doc(hidden)]
pub mod __macro_support {
    /// Classifies a derived inquiry's decoded value at the same reply boundary
    /// as the built-in inquiry table.
    #[doc(hidden)]
    pub fn inquiry_reply_error(error: crate::Error, payload: &[u8]) -> crate::Error {
        crate::command::inquiry_structs::reply_value_error(error, payload)
    }

    /// Build the canonical profile-aware inquiry decoder without boxed context.
    #[doc(hidden)]
    pub fn inquiry_decoder<R>(
        kind: InquiryKind,
        profile: &crate::ProfileSpec,
        lift: fn(crate::command::Response) -> crate::Result<R>,
    ) -> crate::Result<crate::ResponseDecoder<R>> {
        crate::ResponseDecoder::for_builtin_profile(kind, profile, lift)
    }

    /// Return structural payload evidence from the canonical inquiry table.
    #[doc(hidden)]
    pub fn inquiry_payload_shape(
        kind: InquiryKind,
        profile: Option<&crate::ProfileSpec>,
    ) -> crate::PayloadShape {
        crate::command::inquiry_structs::payload_shape_for_profile(kind, profile)
    }

    /// Projects a checked f32 value onto the JSON number its serializer emits.
    #[cfg(feature = "schemars")]
    #[doc(hidden)]
    pub fn f32_schema_decimal(value: f32) -> f64 {
        // Widening before serialization would expose the binary rounding
        // error in the schema and can exclude the value's canonical JSON.
        serde_json::to_string(&value)
            .ok()
            .and_then(|decimal| decimal.parse().ok())
            .unwrap_or_else(|| f64::from(value))
    }

    #[doc(hidden)]
    pub use crate::__grafton_visca_value_helpers as value_helpers;
    #[doc(hidden)]
    pub use crate::command::bytes::constants::INQUIRY;
    #[doc(hidden)]
    pub use crate::command::bytes::{frame_len, write_frame};
    #[doc(hidden)]
    pub use crate::command::inquiry_structs::{InquiryData, InquiryKind};
    #[doc(hidden)]
    pub use crate::macros::public::invalid_value;
    #[doc(hidden)]
    pub use grafton_visca_macros::{__capability_encodings, __noun_signature_types, __value};
    #[cfg(feature = "schemars")]
    pub use schemars;
    #[cfg(feature = "serde")]
    pub use serde;
    #[cfg(feature = "ts-rs")]
    pub use ts_rs;
}

pub(crate) use crate::{
    camera_id::{CameraId, IndividualCameraId},
    command::WhiteBalanceMode,
    error::ErrorKind,
    inquiry_conversions::ZoomDomain,
    visca_socket::{CommandSocketCount, ViscaSocket},
};

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use crate::error::{Certainty, FailureContext, FailureStage};
#[cfg(all(
    test,
    feature = "async",
    feature = "runtime-smol",
    feature = "test-utils"
))]
pub(crate) use crate::executor::SmolExecutor;

#[cfg(all(
    test,
    feature = "async",
    feature = "runtime-tokio",
    feature = "test-utils"
))]
pub(crate) use crate::executor::TokioExecutor;
#[cfg(all(test, unix, feature = "transport-serial-tokio"))]
pub(crate) use crate::runtime::{TokioRuntime, TransportHandle};
#[cfg(test)]
pub(crate) use crate::types::PresetNumber;
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) use crate::{
    error::Recovery,
    types::{PanTiltDirection, UnitInterval},
};

pub mod error;
pub use error::{Error, Result};
pub(crate) mod macros;
#[path = "tuning.rs"]
mod operational_tuning;
mod profile_selection;

/// Checked owner retry tuning inputs.
pub mod tuning {
    pub use crate::operational_tuning::RetryLimit;
}

/// The single row table behind all three noun facades.
pub(crate) mod noun_table;

/// Shared native preparation and forwarding for camera views.
#[cfg(any(feature = "blocking", feature = "async"))]
mod camera_core;

/// Shared public camera/session declarations and forwarders.
#[cfg(any(feature = "blocking", feature = "async"))]
mod camera_facade;

/// Shared native operation handle and lifecycle documentation.
#[cfg(any(feature = "blocking", feature = "async"))]
mod operation_handle;

/// Shared expansion helpers for the noun facades.
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) mod noun_facade;

#[cfg(test)]
pub(crate) mod noun_metadata;

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
#[path = "api_conventions.rs"]
mod api_conventions_source;
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
api_conventions_source::api_convention_tests!();

#[cfg(all(test, feature = "async", feature = "blocking"))]
mod facade_parity;

#[cfg(feature = "async")]
pub(crate) mod executor;

/// Closed operation completion kinds used by typed requests.
#[path = "completion.rs"]
pub mod operation;
pub(crate) use operation as completion;

/// Camera profile system for type-safe, model-specific control
pub(crate) mod camera;

mod camera_id;
#[cfg(any(feature = "async", feature = "blocking"))]
mod camera_view;

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

mod position_inquiries;
mod requests;
pub(crate) use requests::{
    AffectedAxes, Inquiry, OperationCommand, PayloadShape, Request, ResponseDecoder,
    SubmissionClass,
};

#[cfg(any(feature = "blocking", feature = "async", test))]
pub(crate) use requests::PlainCommand;

// Request preparation feeds only the owner, so like the engine it is compiled
// only with a facade.
#[cfg(any(feature = "blocking", feature = "async"))]
mod prepared;

mod stop_request;

/// Bounded owner metrics and diagnostic subscriptions.
pub mod observability;

/// Read-only target-local state projections committed by the owner.
pub mod state_cache;

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use observability::MetricsSnapshot;
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) use observability::SessionStatus;
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use state_cache::PanTiltLimitUpdate;
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use state_cache::StateCache;
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) use state_cache::StateEntry;
pub(crate) use state_cache::{KnownState, StateKey};

mod session_config;
mod target;
#[cfg(any(feature = "blocking", feature = "async", test))]
pub(crate) use session_config::SessionConfig;
#[cfg(all(
    any(test, feature = "test-utils"),
    any(feature = "blocking", feature = "async")
))]
pub(crate) use session_config::DEFAULT_ADMISSION_CAPACITY;
mod source_evidence;
mod target_registration;
pub(crate) use target_registration::{MountPosition, TargetRegistration};

mod outcome;
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use outcome::{CancellationOutcome, HaltOutcome, HaltReport, Settlement};

mod operation_id;
pub(crate) use operation_id::OperationId;

#[cfg(feature = "async")]
#[path = "operation.rs"]
mod async_operation;

#[cfg(feature = "async")]
mod async_nouns;
#[cfg(feature = "async")]
mod async_session;
#[cfg(all(test, feature = "async"))]
pub(crate) use async_session::Camera;
/// Owner-backed async session, camera, operation and noun facade.
#[cfg(feature = "async")]
pub mod r#async {
    pub use crate::async_nouns::*;
    pub use crate::async_operation::Operation;
    pub use crate::async_session::{Camera, CameraSession, Diagnostics, Session};
}

/// Connection and owner configuration values.
pub mod config {
    pub use crate::camera::{CameraConfig, TransportKind, TransportOptions};
    pub use crate::operational_tuning::OperationalTuning;
    pub use crate::session_config::{SessionConfig, DEFAULT_ADMISSION_CAPACITY};
    pub use crate::target_registration::{MountPosition, TargetRegistration};
}

// The module documents itself: an outer doc here would make rustdoc resolve
// the module's intra-doc links at the crate root, against the async facade's
// same-named items.
#[cfg(feature = "blocking")]
pub mod blocking;

/// Inquiry conversion utilities for raw to user-friendly values
pub(crate) mod inquiry_conversions;

pub mod prelude;

/// Validated runtime and compile-time camera profile lowering.
#[path = "profile.rs"]
pub mod profiles;

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use operational_tuning::OperationalTuning;
pub(crate) use profiles::{
    CompileTimeProfile, PanTiltCoordinateConversion, ProfileEnvelope, ProfileSpec,
};
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) use profiles::{ProfileTiming, TransportCompatibility};

pub(crate) mod protocol;

/// VISCA runtime with flume-based scheduling
pub mod runtime;

/// Testing utilities for deterministic downstream tests.
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

pub(crate) mod timeout;
pub(crate) use timeout::{CommandTimeoutClass, CommandTimeouts};

/// Transport layer for implementing custom transports
pub mod transport;

/// Type definitions and abstractions
pub mod types;

/// Semantic unit types for intuitive API usage
pub(crate) mod units;

/// Wire-value codecs used by protocol encoders and decoders.
pub mod wire;

mod visca_socket;

/// Detailed owner, protocol-engine, scheduling, and lifecycle architecture.
///
/// This renders the repository's current-state architecture guide on docs.rs,
/// including the engine transition table and operational invariants that are
/// otherwise implemented by private owner types.
#[doc = include_str!("../docs/architecture_2_0.md")]
pub mod architecture {}

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

// Exhaustive literal-byte inventory tied to the built-in semantic ledger.
#[cfg(test)]
mod issue_715_wire_golden;

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
mod issue_871_gates;

#[cfg(test)]
mod generated_docs;

#[cfg(test)]
mod test_support {
    #[allow(clippy::expect_used, clippy::panic)]
    pub mod generated;

    // The integration and standalone checks also use scanner entry points
    // that the library unit tests do not need.
    #[allow(dead_code, clippy::panic)]
    pub mod source_scan;
}

#[cfg(all(test, feature = "blocking", feature = "serde"))]
mod profile_probe;
