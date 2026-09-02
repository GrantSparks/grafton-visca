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
//! let camera = session.camera::<PtzOpticsG2>()?;
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
//! let camera = session.camera::<PtzOpticsG2>()?;
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
#[cfg(feature = "dyn-api")]
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
        AdvancedAccessor, AutoWhiteBalanceSensitivity, Camera, CancelRejected, Cancellation, Error,
        ExposureAccessor, ExposureMode, FocusAccessor, ImageAccessor, MenuAccessor, MotionAccessor,
        MotionSyncAccessor, NdFilterAccessor, NdFilterMode, Operation, OperationId,
        PanTiltAccessor, PanTiltDirection, PanTiltLimitCorner, PanTiltLimitUpdate, PowerAccessor,
        PresetNumber, PresetsAccessor, Session, SessionConfig, StateCache, StateEntry, StateKey,
        StateValue, SystemAccessor, TallyAccessor, WhiteBalanceAccessor, WhiteBalanceMode,
        ZoomAccessor,
    };
    // High-level camera construction and configuration
    pub use crate::camera::{CameraConfig, IdleWait, MotionQuery};
    pub use crate::Connect;
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Percentage, Raw, UnitInterval};

    // Owner-backed dynamic noun and custom-operation projections.
    #[cfg(feature = "dyn-api")]
    pub use crate::dynapi::{
        submit_applied, submit_targeted, DynAdvanced, DynAppliedOperation, DynAppliedRequest,
        DynCancellation, DynCustomOperations, DynExposure, DynFocus, DynFuture, DynImage, DynMenu,
        DynMotion, DynMotionSync, DynNdFilter, DynPanTilt, DynPower, DynPresets, DynSessionCamera,
        DynSessionCameraControl, DynSessionCameraNouns, DynSystem, DynTally, DynTargetedOperation,
        DynTargetedRequest, DynWhiteBalance, DynZoom, DYN_NOUN_CONVENIENCE_METHODS,
        DYN_NOUN_CONVENIENCE_METHOD_COUNT, DYN_NOUN_COUNT, DYN_NOUN_INQUIRY_METHOD_COUNT,
        DYN_NOUN_TARGET_METHOD_COUNT,
    };

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
        AutoWhiteBalanceSensitivity, CancelRejected, Error, ExposureMode, MetricsSnapshot,
        MotionSyncMode, NdFilterMode, PanTiltDirection, PanTiltLimitCorner, PanTiltLimitUpdate,
        PresetNumber, SessionStatus, StateCache, StateEntry, StateKey, StateValue,
        WhiteBalanceMode,
    };
    // High-level owner-backed blocking session facade and camera view.
    pub use crate::blocking::{
        AdvancedAccessor, Camera, Cancellation, ExposureAccessor, FocusAccessor, ImageAccessor,
        MenuAccessor, MotionAccessor, MotionSyncAccessor, NdFilterAccessor, Operation, OperationId,
        PanTiltAccessor, PowerAccessor, PresetsAccessor, Session, SessionConfig, SystemAccessor,
        TallyAccessor, WhiteBalanceAccessor, ZoomAccessor,
    };
    #[cfg(feature = "dyn-api")]
    pub use crate::dynapi::BlockingDynSessionCamera;
    // Final owner-backed construction in the blocking-only feature build.
    #[cfg(feature = "transport-serial")]
    pub use crate::blocking::SerialConnectBuilder;
    #[cfg(feature = "blocking")]
    pub use crate::blocking::{
        CameraConfig, Connect, ConnectBuilder, TcpConnectBuilder, UdpConnectBuilder,
    };
    pub use crate::camera::{IdleWait, MotionQuery};
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Percentage, Raw, UnitInterval};
}
