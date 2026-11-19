//! Control pan-tilt-zoom cameras over VISCA using TCP, UDP, or serial.
//!
//! A `CameraSession<P>` owns the connection to one camera. Its profile `P`
//! selects the protocol framing, default port, and available controls. The
//! blocking API is enabled by default. The async API takes a Tokio, smol, or
//! caller-provided executor.
//!
//! ## Quick start
//!
//! Read the power state of a PTZOptics G2 over TCP using its default port.
//!
//! ```no_run
//! # #[cfg(feature = "blocking")]
//! fn read_power(address: &str) -> grafton_visca::Result<()> {
//!     use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
//!
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp(address)?;
//!     println!("Power: {:?}", session.camera().power().status()?);
//!     session.close()
//! }
//! ```
//!
//! With `runtime-tokio`, pass the runtime as the last argument and await each call.
//!
//! ```no_run
//! # #[cfg(feature = "runtime-tokio")]
//! async fn read_power(address: &str) -> grafton_visca::Result<()> {
//!     use grafton_visca::{profiles::PtzOpticsG2, r#async::CameraSession, runtime::TokioRuntime};
//!
//!     let runtime = TokioRuntime::from_current()?;
//!     let session = CameraSession::<PtzOpticsG2>::connect_tcp(address, runtime).await?;
//!     println!("Power: {:?}", session.camera().power().status().await?);
//!     session.close().await
//! }
//! ```
//!
//! ## Connections and profiles
//!
//! - Use [`config::CameraConfig`] to reuse connection settings such as TCP
//!   keepalive, serial baud rate, and camera address. Use [`config::SessionConfig`]
//!   for several targets on a serial bus, or `Session::open` for a caller-owned transport.
//! - A [`profiles::CompileTimeProfile`] exposes model-specific methods. For a
//!   model chosen at runtime, use [`profiles::AnyProfile`] and check optional
//!   controls with [`capabilities::Capabilities::supports`].
//! - Enable `transport-serial` for blocking serial or `transport-serial-tokio`
//!   for Tokio serial. The built-in async network adapters use `runtime-tokio`
//!   or `runtime-smol`. Choose the runtime when opening each session.
//!
//! ## Movement and completion
//!
//! A movement call returns an operation handle. `applied()` waits for the camera
//! to complete the VISCA command. On a targeted move, `settled()` returns the
//! profile’s [`operation::Settlement`] evidence, either protocol completion or
//! stable position samples. Use [`types::UnitInterval`] to specify a fraction of
//! the profile’s optical zoom range.
//!
//! ```no_run
//! # #[cfg(feature = "blocking")]
//! # fn half_zoom(camera: &grafton_visca::blocking::Camera<grafton_visca::profiles::PtzOpticsG2>)
//! #     -> grafton_visca::Result<()> {
//! use grafton_visca::types::{UnitInterval, ZoomDomain};
//!
//! let half = UnitInterval::new(0.5)?;
//! let mut movement = camera.zoom().set_normalized(half, ZoomDomain::Optical)?;
//! let evidence = movement.settled()?;
//! # Ok(())
//! # }
//! ```
//!
//! An [`Error::ObservationTimeout`] ends the wait. The operation keeps running
//! and the handle can wait again.
//! Dropping the handle detaches observation. To end a continuous drive, send
//! the axis’s STOP and wait for `applied()`. For a failed connection, use
//! [`Error::requires_new_session`] to decide when to reopen it and query the
//! camera before restoring settings.
//!
//! ## Custom requests and transports
//!
//! [`request::Request`] describes the wire encoding and policy of a custom
//! request. Use `execute` for a plain command, `inquire` for a typed reply, and
//! `submit` for an operation handle. See [`wire::ResponseParser`] for inquiry
//! decoding, [`request::SubmissionClass`] for queue priority, and [`transport`]
//! for implementing a connection.
//!
//! The [usage guide](https://github.com/GrantSparks/grafton-visca/blob/main/docs/usage_2_0.md)
//! covers configuration and runtime selection. See [`architecture`] for session
//! ownership, deadlines, and response handling.

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

// Compile the published examples as doctests. These guides assume `blocking`
// and gate snippets that need additional features individually.
#[cfg(all(doctest, feature = "blocking"))]
#[doc = include_str!("../README.md")]
mod readme_snippets {}

#[cfg(all(doctest, feature = "blocking"))]
#[doc = include_str!("../docs/migration_2_0.md")]
mod migration_guide_snippets {}

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
