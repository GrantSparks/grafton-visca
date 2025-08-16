//! Prelude modules for convenient imports.
//!
//! This module provides two separate preludes for async and blocking APIs.
//! Import only the prelude that matches your use case.
//!
//! # Async Usage
//!
//! ```ignore
//! # #[cfg(feature = "async")]
//! use grafton_visca::prelude::r#async::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "rt-tokio")]
//! # {
//! use grafton_visca::transport::tokio::tcp::Tcp;
//! use grafton_visca::TokioExecutor;
//!
//! let transport = Tcp::connect("192.168.0.110:52381").await?;
//! let executor = TokioExecutor::from_current()?;
//! let camera = PtzOpticsG2Cam::with_executor(transport, executor);
//!
//! // Async methods are available directly on the camera
//! camera.power_on().await?;
//! camera.zoom_stop().await?;
//! camera.pan_tilt_home().await?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! # Blocking Usage
//!
//! ```ignore
//! use grafton_visca::prelude::blocking::*;
//! use grafton_visca::transport::blocking::Tcp;
//!
//! let transport = Tcp::connect("192.168.0.110:52381")?;
//! let camera = PtzOpticsG2Cam::from_transport(transport);
//!
//! // Blocking methods are available directly on the camera
//! camera.power_on()?;
//! camera.zoom_stop()?;
//! camera.pan_tilt_home()?;
//! ```

/// Async prelude - import this for async camera control.
///
/// This prelude provides everything needed for async camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::r#async::*;
/// ```
#[cfg(feature = "async")]
pub mod r#async {
    // Local imports - camera types and profiles
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::camera::{CameraAsync as Camera, MovementConfig};
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NDFilterMode,
        PanTiltDirection, PanTiltLimitCorner, PresetNumber, ResolutionMode, WhiteBalanceMode,
    };

    // Ergonomic type aliases for specific camera models
    /// PtzOptics G2 camera type alias.
    pub type PtzOpticsG2Cam<T> = Camera<PtzOpticsG2, T>;

    /// PtzOptics G3 camera type alias.
    pub type PtzOpticsG3Cam<T> = Camera<PtzOpticsG3, T>;

    /// PtzOptics 30X camera type alias.
    pub type PtzOptics30XCam<T> = Camera<PtzOptics30X, T>;

    /// Sony FR7 camera type alias.
    pub type SonyFR7Cam<T> = Camera<SonyFR7, T>;

    /// Sony BRC-H900 camera type alias.
    pub type SonyBRCH900Cam<T> = Camera<SonyBRCH900, T>;

    /// Sony EVI-H100 camera type alias.
    pub type SonyEVIH100Cam<T> = Camera<SonyEVIH100, T>;

    /// Sony BRC-300 camera type alias.
    pub type SonyBRC300Cam<T> = Camera<SonyBRC300, T>;

    /// Nearus BRC-300 camera type alias.
    pub type NearusBRC300Cam<T> = Camera<NearusBRC300, T>;

    /// Generic VISCA camera type alias.
    pub type GenericViscaCam<T> = Camera<GenericVisca, T>;
}

/// Blocking prelude - import this for synchronous camera control.
///
/// This prelude provides everything needed for blocking camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::blocking::*;
/// ```
pub mod blocking {
    // Local imports - camera types and profiles
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::camera::{CameraBlocking as Camera, MovementConfig};
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NDFilterMode,
        PanTiltDirection, PanTiltLimitCorner, PresetNumber, ResolutionMode, WhiteBalanceMode,
    };

    // Ergonomic type aliases for specific camera models
    /// PtzOptics G2 camera type alias.
    pub type PtzOpticsG2Cam<T> = Camera<PtzOpticsG2, T>;

    /// PtzOptics G3 camera type alias.
    pub type PtzOpticsG3Cam<T> = Camera<PtzOpticsG3, T>;

    /// PtzOptics 30X camera type alias.
    pub type PtzOptics30XCam<T> = Camera<PtzOptics30X, T>;

    /// Sony FR7 camera type alias.
    pub type SonyFR7Cam<T> = Camera<SonyFR7, T>;

    /// Sony BRC-H900 camera type alias.
    pub type SonyBRCH900Cam<T> = Camera<SonyBRCH900, T>;

    /// Sony EVI-H100 camera type alias.
    pub type SonyEVIH100Cam<T> = Camera<SonyEVIH100, T>;

    /// Sony BRC-300 camera type alias.
    pub type SonyBRC300Cam<T> = Camera<SonyBRC300, T>;

    /// Nearus BRC-300 camera type alias.
    pub type NearusBRC300Cam<T> = Camera<NearusBRC300, T>;

    /// Generic VISCA camera type alias.
    pub type GenericViscaCam<T> = Camera<GenericVisca, T>;
}
