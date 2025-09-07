//! Prelude modules for convenient imports.
//!
//! This module provides separate preludes for async and blocking APIs,
//! with a clear separation between high-level (blessed path) and low-level (advanced) APIs.
//!
//! # High-Level API (Recommended)
//!
//! Most users should use the high-level API which provides type-safe camera control:
//!
//! ## Async Usage
//!
//! ```ignore
//! # #[cfg(feature = "async")]
//! use grafton_visca::prelude::r#async::*;
//! use grafton_visca::{Camera, mode::Async};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "rt-tokio")]
//! # {
//! // Simple connection
//! let runtime = TokioRuntime::new();
//! let camera = Camera::<Async, PtzOpticsG2, _, _>::connect_tcp(
//!     "192.168.0.110:5678",
//!     runtime
//! ).await?;
//!
//! // Use camera with type-safe controls
//! camera.power_on().await?;
//! camera.zoom_stop().await?;
//! camera.pan_tilt_home().await?;
//! camera.shutdown().await?;
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! ## Blocking Usage
//!
//! ```ignore
//! use grafton_visca::prelude::blocking::*;
//! use grafton_visca::{Camera, mode::Blocking};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Simple connection
//! let camera = Camera::<Blocking, PtzOpticsG2>::open_tcp("192.168.0.110:5678")?;
//!
//! // Use camera with type-safe controls
//! camera.power_on()?;
//! camera.zoom_stop()?;
//! camera.pan_tilt_home()?;
//! camera.close()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Low-Level API (Advanced)
//!
//! Advanced users who need direct access to transports, raw commands, or custom protocols
//! can use the low-level API:
//!
//! ```ignore
//! use grafton_visca::prelude::advanced::*;
//! use grafton_visca::transport::builder::TransportBuilder as Transport;
//!
//! // Build custom transport with specific settings
//! let transport = Transport::tcp()
//!     .address("192.168.0.110:5678")
//!     .connect_timeout(Duration::from_secs(10))
//!     .build_blocking()?;
//!
//! // Use transport with camera builder
//! let camera = CameraBuilder::from_transport(transport)
//!     .profile::<PtzOpticsG2>()
//!     .open()?;
//! ```

/// Async prelude - import this for async camera control.
///
/// This prelude provides the high-level API for async camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
/// - Runtime support
///
/// For advanced features (custom transports, raw commands), use `prelude::advanced`.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::r#async::*;
/// ```
#[cfg(feature = "async")]
pub mod r#async {
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };

    // High-level camera configuration
    pub use crate::camera::MovementConfig;

    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};

    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NdFilterMode,
        PanTiltDirection, PanTiltLimitCorner, PresetNumber, ResolutionMode, WhiteBalanceMode,
    };

    // Runtime support for async operations
    #[cfg(feature = "rt-async-std")]
    pub use crate::runtime_trait::AsyncStdRuntime;
    #[cfg(feature = "rt-smol")]
    pub use crate::runtime_trait::SmolRuntime;
    #[cfg(feature = "rt-tokio")]
    pub use crate::runtime_trait::TokioRuntime;

    // Runtime-specific camera type aliases for convenience
    #[cfg(feature = "rt-async-std")]
    pub use crate::camera::AsyncStdCamera;
    #[cfg(feature = "rt-smol")]
    pub use crate::camera::SmolCamera;
    #[cfg(feature = "rt-tokio")]
    pub use crate::camera::TokioCamera;
}

/// Blocking prelude - import this for synchronous camera control.
///
/// This prelude provides the high-level API for blocking camera control:
/// - Camera profiles and type aliases
/// - Common types and error handling
///
/// For advanced features (custom transports, raw commands), use `prelude::advanced`.
///
/// # Example
/// ```no_run
/// use grafton_visca::prelude::blocking::*;
/// ```
#[cfg(not(feature = "async"))]
pub mod blocking {
    // Camera profiles - these are the primary way to configure camera behavior
    pub use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };

    // High-level camera types and configuration
    pub use crate::camera::{MovementConfig, UnifiedBlockingCamera as Camera};

    // Type-safe parameter types for camera control
    pub use crate::types::{FStop, IrisLevel, PanSpeed, ShutterSpeed, SpeedLevel, TiltSpeed};
    pub use crate::units::{Degrees, Normalized, Percentage, Raw};

    // Common enums for camera settings
    pub use crate::{
        AutoWhiteBalanceSensitivity, Error, ExposureMode, MotionSyncMode, NdFilterMode,
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

/// Advanced prelude - import this for low-level and custom control.
///
/// This prelude provides access to advanced features for power users:
/// - Direct transport builders and configuration
/// - Raw VISCA commands and protocol handling
/// - Socket managers and buffer management
/// - Custom error handling and retry logic
///
/// **Note:** Most users should use the high-level API in `prelude::async` or
/// `prelude::blocking` instead. Only use these APIs if you need custom transport
/// configuration or raw protocol access.
///
/// # Example
/// ```ignore
/// use grafton_visca::prelude::advanced::*;
/// use grafton_visca::CameraBuilder;
/// use std::time::Duration;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Build a custom transport with specific settings
/// let transport = TransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .tcp_nodelay(true)
///     .build_blocking()?;
///
/// // Use the transport with camera builder
/// let camera = CameraBuilder::from_transport(transport)
///     .profile::<PtzOpticsG2>()
///     .protocol_style(ProtocolStyle::RawVisca)
///     .open()?;
/// # Ok(())
/// # }
/// ```
pub mod advanced {
    // Re-export camera profiles for convenience
    pub use crate::camera::profiles::*;

    // Camera builder for advanced configuration
    pub use crate::camera::CameraBuilder;

    // Transport builder and configuration
    pub use crate::transport::builder::TransportBuilder;

    // Protocol configuration
    pub use crate::capabilities::ProtocolStyle;

    // Socket management (for custom implementations)
    pub use crate::visca_socket::ViscaSocket;

    // Raw command types and encoding
    pub use crate::command::{
        CommandKind, InquiryResponse, ViscaEncode, ViscaResponse, ViscaResponseType,
    };

    // Camera ID for multi-camera setups
    pub use crate::camera_id::CameraId;

    // Transport traits for custom implementations
    #[cfg(not(feature = "async"))]
    pub use crate::transport::sync_transport::SyncTransport;
    #[cfg(feature = "async")]
    pub use crate::transport::AsyncTransport;

    // Runtime and executor types for async operations
    #[cfg(feature = "async")]
    pub use crate::executor::Executor;
    #[cfg(feature = "async")]
    pub use crate::runtime_trait::{Runtime, TransportHandle};

    // Error types with retry logic
    pub use crate::error::{Error, Result};

    // Timeout configuration
    pub use crate::timeout::TimeoutConfig;

    // All the type-safe parameter types (same as high-level)
    pub use crate::types::*;
    pub use crate::units::*;
}
