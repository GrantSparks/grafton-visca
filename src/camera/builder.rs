//! Camera builder for runtime configuration and profile selection.
//!
//! Provides a fluent API for creating cameras with different transports and profiles.

use crate::{capabilities::Profile, error::Error};

use super::Camera;

// ========================================================================================
// Zero-sized type (ZST) markers for type-state pattern
// ========================================================================================

/// Marker for unset profile state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProfileUnset;

/// Marker for blocking TCP transport.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockingTcpMarker;

/// Marker for blocking UDP transport.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockingUdpMarker;

/// Marker for tokio TCP transport.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioTcpMarker;

/// Marker for tokio UDP transport.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioUdpMarker;

/// Marker for unknown/dynamic transport (used with from_config).
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownTransport;

// ========================================================================================
// Transport configuration enum
// ========================================================================================

/// Transport configuration for the camera builder.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Blocking TCP transport configuration.
    BlockingTcp {
        /// Address to connect to (host:port format).
        addr: String,
    },
    /// Blocking UDP transport configuration.
    BlockingUdp {
        /// Address to connect to (host:port format).
        addr: String,
    },
    /// Async TCP transport configuration (tokio).
    #[cfg(feature = "tokio")]
    TokioTcp {
        /// Address to connect to (host:port format).
        addr: String,
    },
    /// Async UDP transport configuration (tokio).
    #[cfg(feature = "tokio")]
    TokioUdp {
        /// Address to connect to (host:port format).
        addr: String,
    },
}

// ========================================================================================
// Main builder struct with type-state pattern
// ========================================================================================

/// Unified builder for creating camera instances.
///
/// This builder provides a simpler, unified API for creating cameras with any transport
/// type. The builder owns the address string, eliminating lifetime parameters.
///
/// ## Automatic Port Selection
///
/// The builder automatically adds the correct default port based on the camera profile
/// if no port is specified. Each camera profile declares its own default ports through
/// the `ProfileMetadata` trait:
/// - PTZOptics cameras (G2, G3, 30X): TCP=5678, UDP=1259  
/// - Generic VISCA: TCP=5678, UDP=1259
/// - Sony cameras: TCP=52381, UDP=52381
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};
///
/// # fn example() -> grafton_visca::Result<()> {
/// // Blocking TCP - strongly typed path
/// let camera = CameraBuilder::tcp("192.168.0.110")
///     .profile::<PTZOpticsG2>()
///     .build()?;  // Returns Camera<PTZOpticsG2, Tcp>
///     
/// // Blocking UDP - strongly typed path
/// let camera = CameraBuilder::udp("192.168.0.110")
///     .profile::<PTZOpticsG2>()
///     .build()?;  // Returns Camera<PTZOpticsG2, Udp>
///
/// // Dynamic path for runtime configuration
/// let config = TransportConfig::BlockingTcp { addr: "192.168.0.110".into() };
/// let camera = CameraBuilder::from_config(config)
///     .profile::<PTZOpticsG2>()
///     .build_dyn()?;  // Returns Camera<PTZOpticsG2, DynTransport>
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CameraBuilder<P = ProfileUnset, K = UnknownTransport> {
    config: TransportConfig,
    #[allow(dead_code)] // Used for type-state pattern
    profile: P,
    marker: K,
}

// ========================================================================================
// CameraBuilder constructors
// ========================================================================================

impl CameraBuilder {
    /// Create a builder for a blocking TCP transport.
    pub fn tcp(addr: impl Into<String>) -> CameraBuilder<ProfileUnset, BlockingTcpMarker> {
        CameraBuilder {
            config: TransportConfig::BlockingTcp { addr: addr.into() },
            profile: ProfileUnset,
            marker: BlockingTcpMarker,
        }
    }

    /// Create a builder for a blocking UDP transport.
    pub fn udp(addr: impl Into<String>) -> CameraBuilder<ProfileUnset, BlockingUdpMarker> {
        CameraBuilder {
            config: TransportConfig::BlockingUdp { addr: addr.into() },
            profile: ProfileUnset,
            marker: BlockingUdpMarker,
        }
    }

    /// Create a builder for an async TCP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_tcp(addr: impl Into<String>) -> CameraBuilder<ProfileUnset, TokioTcpMarker> {
        CameraBuilder {
            config: TransportConfig::TokioTcp { addr: addr.into() },
            profile: ProfileUnset,
            marker: TokioTcpMarker,
        }
    }

    /// Create a builder for an async UDP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_udp(addr: impl Into<String>) -> CameraBuilder<ProfileUnset, TokioUdpMarker> {
        CameraBuilder {
            config: TransportConfig::TokioUdp { addr: addr.into() },
            profile: ProfileUnset,
            marker: TokioUdpMarker,
        }
    }

    /// Create a builder from a runtime configuration.
    /// This is the dynamic path for when transport type is not known at compile time.
    pub fn from_config(config: TransportConfig) -> CameraBuilder<ProfileUnset, UnknownTransport> {
        CameraBuilder {
            config,
            profile: ProfileUnset,
            marker: UnknownTransport,
        }
    }
}

// ========================================================================================
// Profile selection - available for all transport markers
// ========================================================================================

impl<K> CameraBuilder<ProfileUnset, K> {
    /// Lock in the compile-time profile and return a builder with that profile.
    pub fn profile<P>(self) -> CameraBuilder<P, K>
    where
        P: Profile + Default,
    {
        CameraBuilder {
            config: self.config,
            profile: P::default(),
            marker: self.marker,
        }
    }
}

/// Protocol selection for transport builders
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    /// TCP protocol
    Tcp,
    /// UDP protocol
    Udp,
}

// ========================================================================================
// Specialized build() methods for each transport marker - strongly typed path
// ========================================================================================

/// Build implementation for blocking TCP transport.
impl<P: Profile> CameraBuilder<P, BlockingTcpMarker> {
    /// Build the camera with blocking TCP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Tcp>, Error> {
        match self.config {
            TransportConfig::BlockingTcp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Tcp);
                let transport = crate::transport::blocking::Tcp::connect(&addr)?;
                Ok(Camera::from_transport(transport))
            }
            _ => unreachable!("BlockingTcpMarker guarantees BlockingTcp config"),
        }
    }
}

/// Build implementation for blocking UDP transport.
impl<P: Profile> CameraBuilder<P, BlockingUdpMarker> {
    /// Build the camera with blocking UDP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Udp>, Error> {
        match self.config {
            TransportConfig::BlockingUdp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Udp);
                let transport = crate::transport::blocking::Udp::connect(&addr)?;
                Ok(Camera::from_transport(transport))
            }
            _ => unreachable!("BlockingUdpMarker guarantees BlockingUdp config"),
        }
    }
}

/// Build implementation for tokio TCP transport.
#[cfg(feature = "tokio")]
impl<P: Profile> CameraBuilder<P, TokioTcpMarker> {
    /// Build the camera with async TCP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Tcp>, Error> {
        match self.config {
            TransportConfig::TokioTcp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Tcp);
                let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
                Ok(Camera::from_transport(transport))
            }
            _ => unreachable!("TokioTcpMarker guarantees TokioTcp config"),
        }
    }
}

/// Build implementation for tokio UDP transport.
#[cfg(feature = "tokio")]
impl<P: Profile> CameraBuilder<P, TokioUdpMarker> {
    /// Build the camera with async UDP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Udp>, Error> {
        match self.config {
            TransportConfig::TokioUdp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Udp);
                let transport = crate::transport::tokio::Udp::connect(&addr).await?;
                Ok(Camera::from_transport(transport))
            }
            _ => unreachable!("TokioUdpMarker guarantees TokioUdp config"),
        }
    }
}

// ========================================================================================
// Dynamic transport enum for runtime-configured transports
// ========================================================================================

/// Dynamic transport enum that can hold any transport type at runtime.
///
/// This is used when the transport type is determined at runtime (e.g., from configuration
/// files or CLI arguments). It has a small runtime overhead due to dynamic dispatch but
/// provides flexibility when compile-time transport selection is not possible.
#[derive(Debug)]
pub enum DynTransport {
    /// Blocking TCP transport.
    BlockingTcp(crate::transport::blocking::Tcp),
    /// Blocking UDP transport.
    BlockingUdp(crate::transport::blocking::Udp),
    /// Async TCP transport (tokio).
    #[cfg(feature = "tokio")]
    TokioTcp(crate::transport::tokio::Tcp),
    /// Async UDP transport (tokio).
    #[cfg(feature = "tokio")]
    TokioUdp(crate::transport::tokio::Udp),
}

// Implement UnifiedTransport trait for DynTransport
#[async_trait::async_trait]
impl crate::transport::UnifiedTransport for DynTransport {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            DynTransport::BlockingTcp(t) => t.send(bytes).await,
            DynTransport::BlockingUdp(t) => t.send(bytes).await,
            #[cfg(feature = "tokio")]
            DynTransport::TokioTcp(t) => t.send(bytes).await,
            #[cfg(feature = "tokio")]
            DynTransport::TokioUdp(t) => t.send(bytes).await,
        }
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        match self {
            DynTransport::BlockingTcp(t) => t.recv().await,
            DynTransport::BlockingUdp(t) => t.recv().await,
            #[cfg(feature = "tokio")]
            DynTransport::TokioTcp(t) => t.recv().await,
            #[cfg(feature = "tokio")]
            DynTransport::TokioUdp(t) => t.recv().await,
        }
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            DynTransport::BlockingTcp(t) => t.send_blocking(bytes),
            DynTransport::BlockingUdp(t) => t.send_blocking(bytes),
            #[cfg(feature = "tokio")]
            DynTransport::TokioTcp(t) => t.send_blocking(bytes),
            #[cfg(feature = "tokio")]
            DynTransport::TokioUdp(t) => t.send_blocking(bytes),
        }
    }

    fn recv_blocking_timeout(&self, timeout: std::time::Duration) -> Result<bytes::Bytes, Error> {
        match self {
            DynTransport::BlockingTcp(t) => t.recv_blocking_timeout(timeout),
            DynTransport::BlockingUdp(t) => t.recv_blocking_timeout(timeout),
            #[cfg(feature = "tokio")]
            DynTransport::TokioTcp(t) => t.recv_blocking_timeout(timeout),
            #[cfg(feature = "tokio")]
            DynTransport::TokioUdp(t) => t.recv_blocking_timeout(timeout),
        }
    }
}

/// Type alias for a Camera with dynamic transport.
pub type CameraDyn<P> = Camera<P, DynTransport>;

// ========================================================================================
// Dynamic build method for unknown transport marker
// ========================================================================================

impl<P: Profile> CameraBuilder<P, UnknownTransport> {
    /// Build the camera with a dynamic transport determined at runtime.
    ///
    /// This method creates a camera with `DynTransport`, which adds a small runtime
    /// overhead but allows the transport type to be determined from runtime configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub fn build_dyn(self) -> Result<CameraDyn<P>, Error> {
        match self.config {
            TransportConfig::BlockingTcp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Tcp);
                let transport = crate::transport::blocking::Tcp::connect(&addr)?;
                Ok(Camera::from_transport(DynTransport::BlockingTcp(transport)))
            }
            TransportConfig::BlockingUdp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Udp);
                let transport = crate::transport::blocking::Udp::connect(&addr)?;
                Ok(Camera::from_transport(DynTransport::BlockingUdp(transport)))
            }
            #[cfg(feature = "tokio")]
            TransportConfig::TokioTcp { addr: _ } => {
                // For async transports in build_dyn, we need to handle them differently
                // Since build_dyn is sync, we can't await here.
                Err(Error::TransportMismatch {
                    reason: "Async TCP transport requires async build. Use build_dyn_async().await or the typed path with .build().await",
                })
            }
            #[cfg(feature = "tokio")]
            TransportConfig::TokioUdp { addr: _ } => {
                Err(Error::TransportMismatch {
                    reason: "Async UDP transport requires async build. Use build_dyn_async().await or the typed path with .build().await",
                })
            }
        }
    }

    /// Build the camera with a dynamic transport determined at runtime (async version).
    ///
    /// This method creates a camera with `DynTransport`, supporting all transport types
    /// including async ones.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    #[cfg(feature = "tokio")]
    pub async fn build_dyn_async(self) -> Result<CameraDyn<P>, Error> {
        match self.config {
            TransportConfig::BlockingTcp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Tcp);
                let transport = crate::transport::blocking::Tcp::connect(&addr)?;
                Ok(Camera::from_transport(DynTransport::BlockingTcp(transport)))
            }
            TransportConfig::BlockingUdp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Udp);
                let transport = crate::transport::blocking::Udp::connect(&addr)?;
                Ok(Camera::from_transport(DynTransport::BlockingUdp(transport)))
            }
            #[cfg(feature = "tokio")]
            TransportConfig::TokioTcp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Tcp);
                let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
                Ok(Camera::from_transport(DynTransport::TokioTcp(transport)))
            }
            #[cfg(feature = "tokio")]
            TransportConfig::TokioUdp { addr } => {
                let addr = ensure_port::<P>(&addr, Protocol::Udp);
                let transport = crate::transport::tokio::Udp::connect(&addr).await?;
                Ok(Camera::from_transport(DynTransport::TokioUdp(transport)))
            }
        }
    }
}

/// Ensure the address has a port, adding the default if not specified.
///
/// Note: This cannot be const fn in stable Rust due to String operations.
pub fn ensure_port<P: Profile>(addr: &str, protocol: Protocol) -> String {
    if addr.contains(':') {
        addr.to_string()
    } else {
        let default_port = match protocol {
            Protocol::Tcp => P::DEFAULT_TCP_PORT,
            Protocol::Udp => P::DEFAULT_UDP_PORT,
        };
        format!("{}:{}", addr, default_port)
    }
}

#[cfg(test)]
mod tests {
    use crate::camera::profiles::{GenericVisca, PTZOpticsG2, SonyBRC300, SonyEVIH100, SonyFR7};

    use super::*;

    #[test]
    fn test_builder_type_inference() {
        let _builder = CameraBuilder::tcp("127.0.0.1:1234").profile::<GenericVisca>();
    }

    #[test]
    fn test_tcp_builder_creation() {
        let builder = CameraBuilder::tcp("192.168.0.110:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert!(
            matches!(typed.config, TransportConfig::BlockingTcp { ref addr } if addr == "192.168.0.110:52381"),
            "Expected BlockingTcp with correct address"
        );
    }

    #[test]
    fn test_udp_builder_creation() {
        let builder = CameraBuilder::udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert!(
            matches!(typed.config, TransportConfig::BlockingUdp { ref addr } if addr == "239.0.0.1:52381"),
            "Expected BlockingUdp with correct address"
        );
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_tcp_builder_creation() {
        let builder = CameraBuilder::tokio_tcp("192.168.0.110:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert!(
            matches!(typed.config, TransportConfig::TokioTcp { ref addr } if addr == "192.168.0.110:52381"),
            "Expected TokioTcp with correct address"
        );
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_udp_builder_creation() {
        let builder = CameraBuilder::tokio_udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert!(
            matches!(typed.config, TransportConfig::TokioUdp { ref addr } if addr == "239.0.0.1:52381"),
            "Expected TokioUdp with correct address"
        );
    }

    #[test]
    fn test_builder_accepts_string_types() {
        // Test with &str
        let _builder1 = CameraBuilder::tcp("192.168.0.110");

        // Test with String
        let addr = String::from("192.168.0.110");
        let _builder2 = CameraBuilder::tcp(addr);

        // Test with &String
        let addr = String::from("192.168.0.110");
        let _builder3 = CameraBuilder::tcp(&addr);
    }

    #[test]
    fn test_builder_is_cloneable() {
        let builder = CameraBuilder::tcp("192.168.0.110");
        let _builder2 = builder.clone();

        let typed = CameraBuilder::tcp("192.168.0.110").profile::<PTZOpticsG2>();
        let _typed2 = typed.clone();
    }

    #[test]
    fn test_ensure_port_function() {
        // Test PTZOptics profile with TCP port
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110", Protocol::Tcp),
            "192.168.0.110:5678"
        );

        // Test PTZOptics profile with UDP port
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110", Protocol::Udp),
            "192.168.0.110:1259"
        );

        // Test Sony profile (always uses Sony port regardless of transport)
        assert_eq!(
            ensure_port::<SonyFR7>("192.168.0.112", Protocol::Tcp),
            "192.168.0.112:52381"
        );

        assert_eq!(
            ensure_port::<SonyFR7>("192.168.0.112", Protocol::Udp),
            "192.168.0.112:52381"
        );

        // Test with explicit port is preserved
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110:8080", Protocol::Tcp),
            "192.168.0.110:8080"
        );

        // Test with hostname
        assert_eq!(
            ensure_port::<PTZOpticsG2>("camera.local", Protocol::Tcp),
            "camera.local:5678"
        );

        // Test with IPv6 addresses
        assert_eq!(
            ensure_port::<PTZOpticsG2>("[::1]:8080", Protocol::Tcp),
            "[::1]:8080"
        );
    }

    #[test]
    fn test_blocking_tcp_port_defaults() {
        // Test PTZOptics uses correct default TCP port
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110", Protocol::Tcp),
            "192.168.0.110:5678"
        );

        // Test with explicit port is preserved
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110:8080", Protocol::Tcp),
            "192.168.0.110:8080"
        );

        // Test GenericVisca uses correct default TCP port
        assert_eq!(
            ensure_port::<GenericVisca>("192.168.0.111", Protocol::Tcp),
            "192.168.0.111:5678"
        );

        // Test Sony camera uses correct default port
        assert_eq!(
            ensure_port::<SonyFR7>("192.168.0.112", Protocol::Tcp),
            "192.168.0.112:52381"
        );
    }

    #[test]
    fn test_blocking_udp_port_defaults() {
        // Test PTZOptics uses correct default UDP port
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110", Protocol::Udp),
            "192.168.0.110:1259"
        );

        // Test with explicit port is preserved
        assert_eq!(
            ensure_port::<PTZOpticsG2>("192.168.0.110:8080", Protocol::Udp),
            "192.168.0.110:8080"
        );

        // Test GenericVisca uses correct default UDP port
        assert_eq!(
            ensure_port::<GenericVisca>("192.168.0.111", Protocol::Udp),
            "192.168.0.111:1259"
        );

        // Test Sony camera uses correct default port
        assert_eq!(
            ensure_port::<SonyFR7>("192.168.0.112", Protocol::Udp),
            "192.168.0.112:52381"
        );
    }

    #[test]
    fn test_builder_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<CameraBuilder>();
        assert_send_sync::<CameraBuilder<GenericVisca, BlockingTcpMarker>>();
        assert_send_sync::<CameraBuilder<PTZOpticsG2, BlockingUdpMarker>>();
    }

    #[test]
    fn test_from_config() {
        let config = TransportConfig::BlockingTcp {
            addr: "192.168.0.110".into(),
        };
        let builder = CameraBuilder::from_config(config.clone());
        let typed = builder.profile::<PTZOpticsG2>();
        assert!(
            matches!(typed.config, TransportConfig::BlockingTcp { ref addr } if addr == "192.168.0.110"),
            "Expected BlockingTcp with correct address"
        );
    }

    #[test]
    fn test_dynamic_transport_config() {
        // Test that from_config creates correct UnknownTransport marker
        let config = TransportConfig::BlockingTcp {
            addr: "192.168.0.110".into(),
        };
        let builder = CameraBuilder::from_config(config);
        // This should compile - builder has UnknownTransport marker
        let _typed = builder.profile::<PTZOpticsG2>();
        // We can't call .build() on UnknownTransport, only .build_dyn()
    }

    #[test]
    fn test_sony_raw_visca_cameras_use_standard_ports() {
        // SonyEVIH100 uses RawVisca protocol and should use standard VISCA ports
        assert_eq!(
            ensure_port::<SonyEVIH100>("192.168.0.113", Protocol::Tcp),
            "192.168.0.113:5678"
        );

        assert_eq!(
            ensure_port::<SonyEVIH100>("192.168.0.113", Protocol::Udp),
            "192.168.0.113:1259"
        );

        // SonyBRC300 uses RawVisca protocol and should use standard VISCA ports
        assert_eq!(
            ensure_port::<SonyBRC300>("192.168.0.114", Protocol::Tcp),
            "192.168.0.114:5678"
        );

        assert_eq!(
            ensure_port::<SonyBRC300>("192.168.0.114", Protocol::Udp),
            "192.168.0.114:1259"
        );
    }
}
