//! Prelude modules for convenient imports.
//!
//! This module provides separate preludes for async and blocking camera-first APIs.
//!
//! # High-Level API (Recommended)
//!
//! Most users should use the high-level API which provides type-safe camera control:
//!
//! ## Async Usage
//!
//! ```ignore
//! # #[cfg(feature = "mode-async")]
//! use grafton_visca::prelude::r#async::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "runtime-tokio")]
//! # {
//! let runtime = TokioRuntime::from_current()?;
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(
//!     "192.168.0.110",
//!     runtime,
//! ).await?;
//!
//! camera.power().on().await?;
//! camera.zoom().stop().await?;
//! camera.pan_tilt().home().await?;
//! camera.close().await?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! ## Blocking Usage
//!
//! ```ignore
//! use grafton_visca::prelude::blocking::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
//!
//! camera.power().on()?;
//! camera.zoom().stop()?;
//! camera.pan_tilt().home()?;
//! camera.close()?;
//! # Ok(())
//! # }
//! ```
//!
//! Advanced extension APIs remain available from their owning modules, such as
//! [`camera`](crate::camera), [`command`](crate::command), and
//! [`transport`](crate::transport). They are intentionally not glob-reexported
//! from a raw prelude. For custom commands that are not represented by a typed
//! control trait, use [`Camera::execute`](crate::Camera::execute) or
//! [`Camera::send_command`](crate::Camera::send_command) explicitly.

/// Async prelude - import this for async camera control.
///
/// This prelude provides the high-level API for async camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
/// - Runtime support
///
/// Import advanced extension APIs from their owning modules when needed.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::r#async::*;
/// ```
#[cfg(feature = "mode-async")]
pub mod r#async {
    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NdFilterMode,
        PanTiltDirection, PanTiltLimitCorner, PresetNumber, ResolutionMode, WhiteBalanceMode,
    };
    // High-level camera construction and configuration
    pub use crate::camera::{AwaitConfig, CameraConfig, Connect};
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};

    // Runtime support for async operations
    #[cfg(feature = "runtime-smol")]
    pub use crate::runtime::SmolRuntime;
    #[cfg(feature = "runtime-tokio")]
    pub use crate::runtime::TokioRuntime;
}

/// Blocking prelude - import this for synchronous camera control.
///
/// This prelude provides the high-level API for blocking camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
///
/// Import advanced extension APIs from their owning modules when needed.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::blocking::*;
/// ```
#[cfg(not(feature = "mode-async"))]
pub mod blocking {
    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NdFilterMode,
        PanTiltDirection, PanTiltLimitCorner, PresetNumber, ResolutionMode, WhiteBalanceMode,
    };
    // High-level camera construction, camera type, and configuration
    pub use crate::camera::{AwaitConfig, BlockingCamera as Camera, CameraConfig, Connect};
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};

    // Ergonomic type aliases for specific camera models
    /// Generic VISCA camera type alias.
    pub type GenericViscaCam<T> = Camera<GenericVisca, T>;

    /// Nearus BRC-300 camera type alias.
    pub type NearusBRC300Cam<T> = Camera<NearusBRC300, T>;

    /// PtzOptics 30X camera type alias.
    pub type PtzOptics30XCam<T> = Camera<PtzOptics30X, T>;

    /// PtzOptics G2 camera type alias.
    pub type PtzOpticsG2Cam<T> = Camera<PtzOpticsG2, T>;

    /// PtzOptics G3 camera type alias.
    pub type PtzOpticsG3Cam<T> = Camera<PtzOpticsG3, T>;

    /// Sony BRC-300 camera type alias.
    pub type SonyBRC300Cam<T> = Camera<SonyBRC300, T>;

    /// Sony BRC-H900 camera type alias.
    pub type SonyBRCH900Cam<T> = Camera<SonyBRCH900, T>;

    /// Sony EVI-H100 camera type alias.
    pub type SonyEVIH100Cam<T> = Camera<SonyEVIH100, T>;

    /// Sony FR7 camera type alias.
    pub type SonyFR7Cam<T> = Camera<SonyFR7, T>;
}
