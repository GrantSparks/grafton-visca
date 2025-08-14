//! Prelude modules for convenient imports.
//!
//! This module provides two separate preludes for async and blocking APIs.
//! Import only the prelude that matches your use case.
//!
//! # Async Usage
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! use grafton_visca::prelude::r#async::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "rt-tokio")]
//! # {
//! use grafton_visca::transport::tokio::Tcp;
//!
//! let transport = Tcp::connect("192.168.0.110:52381").await?;
//! let camera = PTZOpticsG2Cam::from_transport(transport);
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
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(not(feature = "async"))]
//! # {
//! use grafton_visca::prelude::blocking::*;
//! use grafton_visca::transport::blocking::Tcp;
//!
//! let transport = Tcp::connect("192.168.0.110:52381")?;
//! let camera = PTZOpticsG2Cam::from_transport(transport);
//!
//! // Blocking methods are available directly on the camera
//! camera.power_on()?;
//! camera.zoom_stop()?;
//! camera.pan_tilt_home()?;
//! # }
//! # Ok(())
//! # }
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
    // MovementOps trait has been removed - movement methods are now inherent methods on Camera
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300,
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
    /// PTZOptics G2 camera type alias.
    pub type PTZOpticsG2Cam<T> = Camera<PTZOpticsG2, T>;

    /// PTZOptics G3 camera type alias.
    pub type PTZOpticsG3Cam<T> = Camera<PTZOpticsG3, T>;

    /// PTZOptics 30X camera type alias.
    pub type PTZOptics30XCam<T> = Camera<PTZOptics30X, T>;

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
/// **Note:** This module is only available when the `async` feature is not enabled.
/// You must choose either async or blocking API, not both.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::blocking::*;
/// ```
#[cfg(not(feature = "async"))]
pub mod blocking {
    // MovementOps trait has been removed - movement methods are now inherent methods on Camera
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300,
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
    /// PTZOptics G2 camera type alias.
    pub type PTZOpticsG2Cam<T> = Camera<PTZOpticsG2, T>;

    /// PTZOptics G3 camera type alias.
    pub type PTZOpticsG3Cam<T> = Camera<PTZOpticsG3, T>;

    /// PTZOptics 30X camera type alias.
    pub type PTZOptics30XCam<T> = Camera<PTZOptics30X, T>;

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
