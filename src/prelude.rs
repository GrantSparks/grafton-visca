//! Prelude modules for convenient imports.
//!
//! This module provides separate preludes for the async and blocking facades.
//! Both use `Connect` or `CameraConfig` for construction, then accessor-style
//! controls.
//!
//! # High-Level API (Recommended)
//!
//! Most users should use the high-level API which provides type-safe camera
//! control. Movement accessors return `#[must_use]` operation handles: observe
//! them with `applied()` for applied-only commands and `settled()` for targeted
//! commands. A handle that is neither observed nor `detach`ed is a lint, not a
//! shortcut.
//!
//! ## Async Usage
//!
//! ```no_run
//! # #[cfg(feature = "runtime-tokio")]
//! # async fn quick_start() -> Result<(), Box<dyn std::error::Error>> {
//! use grafton_visca::prelude::r#async::*;
//!
//! let runtime = TokioRuntime::from_current()?;
//! let session = Connect::open_tcp::<PtzOpticsG2, _>(
//!     "192.168.0.110",
//!     runtime,
//! ).await?;
//! let camera = session.camera();
//!
//! camera.power().on().await?;
//! camera.zoom().stop().await?.applied().await?;
//! camera.pan_tilt().home().await?.settled().await?;
//! session.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Blocking Usage
//!
//! ```no_run
//! # #[cfg(feature = "blocking")]
//! # fn quick_start() -> Result<(), Box<dyn std::error::Error>> {
//! use grafton_visca::prelude::blocking::*;
//!
//! let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
//! let camera = session.camera();
//!
//! camera.power().on()?;
//! camera.zoom().stop()?.applied()?;
//! camera.pan_tilt().home()?.settled()?;
//! session.close()?;
//! # Ok(())
//! # }
//! ```
//!
//! Advanced extension APIs remain available from their owning modules, such as
//! [`camera`](crate::camera), [`command`](crate::command), and
//! [`transport`](crate::transport). They are intentionally not glob-reexported
//! from a raw prelude. Use `Session::open` for async or
//! `blocking::Session::open` for blocking custom transport attachment, and use
//! [`command`](crate::command) explicitly for custom VISCA commands that are
//! not represented by a typed accessor.

/// Runtime-profile projection prelude.
///
/// With `blocking`, this includes `BlockingDynSessionCamera`.
/// With `async`, it additionally includes the object-safe dynamic noun and
/// custom-operation surface. Every projection keeps its facade's canonical
/// owner and operation lifecycle.
#[cfg(all(feature = "dyn-api", any(feature = "async", feature = "blocking")))]
pub mod dyn_api {
    pub use crate::dynapi::*;
}

/// Async prelude - import this for async camera control.
///
/// This prelude provides the high-level API for async camera control:
/// - Camera profiles and typed camera views
/// - Common types and error handling
/// - Runtime support
///
/// Import advanced extension APIs from their owning modules when needed.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::r#async::*;
/// ```
#[cfg(feature = "async")]
pub mod r#async {
    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, Camera, CancellationOutcome, Error, ExposureMode,
        NdFilterMode, Operation, OperationId, PanTiltDirection, PanTiltLimitCorner,
        PanTiltLimitUpdate, PresetNumber, Session, SessionConfig, StateCache, StateEntry, StateKey,
        StateValue, WhiteBalanceMode,
    };
    // The noun accessors and the motion view, one per noun-table header.
    use crate::noun_table::reexport_nouns;
    crate::noun_table::noun_table!(reexport_nouns, [accessor], [crate]);
    crate::noun_table::motion_table!(reexport_nouns, [accessor], [crate]);
    // High-level camera construction and configuration
    pub use crate::camera::{CameraConfig, IdleWait, MotionQuery};
    pub use crate::Connect;
    // Camera profiles - these are the primary way to configure camera behavior
    crate::camera::profiles::profile_registry::builtin_profile_registry!(
        crate::camera::profiles::profile_registry::reexport_builtin_profiles
    );
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Percentage, Raw, UnitInterval};

    // Owner-backed dynamic noun and custom-operation projections.
    #[cfg(feature = "dyn-api")]
    pub use crate::dynapi::{
        submit_applied, submit_targeted, DynAppliedOperation, DynAppliedRequest,
        DynCustomOperations, DynFuture, DynSessionCamera, DynSessionCameraControl,
        DynSessionCameraNouns, DynTargetedOperation, DynTargetedRequest,
    };
    // The noun traits and the motion trait, one per noun-table header.
    #[cfg(feature = "dyn-api")]
    crate::noun_table::noun_table!(reexport_nouns, [dyn_trait], [crate::dynapi]);
    #[cfg(feature = "dyn-api")]
    crate::noun_table::motion_table!(reexport_nouns, [dyn_trait], [crate::dynapi]);

    // Runtime support for async operations
    #[cfg(feature = "runtime-smol")]
    pub use crate::runtime::SmolRuntime;
    #[cfg(feature = "runtime-tokio")]
    pub use crate::runtime::TokioRuntime;
}

/// Blocking prelude - import this for synchronous camera control.
///
/// This prelude provides the high-level API for blocking camera control:
/// - Camera profiles and borrowed typed camera views
/// - Common types and error handling
///
/// Import advanced extension APIs from their owning modules when needed.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::blocking::*;
/// ```
#[cfg(feature = "blocking")]
pub mod blocking {
    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, CancellationOutcome, Error, ExposureMode, MetricsSnapshot,
        MotionSyncMode, NdFilterMode, PanTiltDirection, PanTiltLimitCorner, PanTiltLimitUpdate,
        PresetNumber, SessionStatus, StateCache, StateEntry, StateKey, StateValue,
        WhiteBalanceMode,
    };
    // High-level owner-backed blocking session facade and camera view.
    pub use crate::blocking::{Camera, Operation, OperationId, Session, SessionConfig};
    // The noun accessors and the motion view, one per noun-table header.
    use crate::noun_table::reexport_nouns;
    crate::noun_table::noun_table!(reexport_nouns, [accessor], [crate::blocking]);
    crate::noun_table::motion_table!(reexport_nouns, [accessor], [crate::blocking]);
    #[cfg(feature = "dyn-api")]
    pub use crate::dynapi::BlockingDynSessionCamera;
    // Final owner-backed construction in the blocking-only feature build.
    #[cfg(feature = "blocking")]
    pub use crate::blocking::{CameraConfig, Connect};
    pub use crate::camera::{IdleWait, MotionQuery};
    // Camera profiles - these are the primary way to configure camera behavior
    crate::camera::profiles::profile_registry::builtin_profile_registry!(
        crate::camera::profiles::profile_registry::reexport_builtin_profiles
    );
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Percentage, Raw, UnitInterval};
}
