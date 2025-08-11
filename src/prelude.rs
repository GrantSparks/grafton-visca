//! Prelude modules for convenient imports.
//!
//! This module provides two separate preludes for async and blocking APIs.
//! Import only the prelude that matches your use case to avoid trait conflicts.
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
//! let camera = PTZOpticsG2Cam::new(transport);
//!
//! // Async trait methods are available with clean names
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
//! use grafton_visca::prelude::blocking::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(not(feature = "async"))]
//! # {
//! use grafton_visca::transport::blocking::Tcp;
//!
//! let transport = Tcp::connect("192.168.0.110:52381")?;
//! let camera = PTZOpticsG2Cam::new(transport);
//!
//! // Blocking trait methods are available with clean names
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
/// - All async operation traits with clean names
/// - Camera profiles and type aliases
/// - Common types and error handling
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::r#async::*;
/// ```
#[cfg(feature = "async")]
pub mod r#async {
    pub use crate::camera::helpers::MovementOps;
    pub use crate::camera::methods::{
        ColorOps, ExposureCompensationOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps,
        MenuControlOps, MotionSyncControl, NDFilterOps, PanTiltInquiryOps, PanTiltOps, PowerOps,
        PresetsOps, StreamingOps, SystemOps, TallyOps, VariableSpeedOps, WhiteBalanceOps, ZoomOps,
    };
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::camera::{Camera, CameraState, MovementConfig};
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
/// - All blocking operation traits with clean names (no 'Blocking' suffix)
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
    pub use crate::camera::helpers::MovementOpsBlocking as MovementOps;
    pub use crate::camera::methods::{
        ColorOpsBlocking as ColorOps, ExposureCompensationOpsBlocking as ExposureCompensationOps,
        ExposureOpsBlocking as ExposureOps, FocusOpsBlocking as FocusOps,
        ImageProcessingOpsBlocking as ImageProcessingOps, InquiryOpsBlocking as InquiryOps,
        MenuControlOpsBlocking as MenuControlOps, MotionSyncControlBlocking as MotionSyncControl,
        NDFilterOpsBlocking as NDFilterOps, PanTiltInquiryOpsBlocking as PanTiltInquiryOps,
        PanTiltOpsBlocking as PanTiltOps, PowerOpsBlocking as PowerOps,
        PresetsOpsBlocking as PresetsOps, StreamingOpsBlocking as StreamingOps,
        SystemOpsBlocking as SystemOps, TallyOpsBlocking as TallyOps,
        VariableSpeedOpsBlocking as VariableSpeedOps, WhiteBalanceOpsBlocking as WhiteBalanceOps,
        ZoomOpsBlocking as ZoomOps,
    };
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    pub use crate::camera::{Camera, CameraState, MovementConfig};
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
