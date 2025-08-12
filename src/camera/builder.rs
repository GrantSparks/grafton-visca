//! Camera builder for runtime configuration and profile selection.
//!
//! Provides a fluent API for creating cameras with different transports and profiles.

use crate::{capabilities::Profile, error::Error};

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
#[cfg(feature = "rt-tokio")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioTcpMarker;

/// Marker for tokio UDP transport.
#[cfg(feature = "rt-tokio")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioUdpMarker;

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
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CameraBuilder<K, P = ProfileUnset> {
    addr: String,
    #[allow(dead_code)] // Used for type-state pattern
    profile: P,
    marker: K,
}

// ========================================================================================
// CameraBuilder constructors
// ========================================================================================

impl CameraBuilder<(), ProfileUnset> {
    /// Create a builder for a blocking TCP transport.
    pub fn tcp(addr: impl Into<String>) -> CameraBuilder<BlockingTcpMarker, ProfileUnset> {
        CameraBuilder {
            addr: addr.into(),
            profile: ProfileUnset,
            marker: BlockingTcpMarker,
        }
    }

    /// Create a builder for a blocking UDP transport.
    pub fn udp(addr: impl Into<String>) -> CameraBuilder<BlockingUdpMarker, ProfileUnset> {
        CameraBuilder {
            addr: addr.into(),
            profile: ProfileUnset,
            marker: BlockingUdpMarker,
        }
    }

    /// Create a builder for an async TCP transport (tokio).
    #[cfg(feature = "rt-tokio")]
    pub fn tokio_tcp(addr: impl Into<String>) -> CameraBuilder<TokioTcpMarker, ProfileUnset> {
        CameraBuilder {
            addr: addr.into(),
            profile: ProfileUnset,
            marker: TokioTcpMarker,
        }
    }

    /// Create a builder for an async UDP transport (tokio).
    #[cfg(feature = "rt-tokio")]
    pub fn tokio_udp(addr: impl Into<String>) -> CameraBuilder<TokioUdpMarker, ProfileUnset> {
        CameraBuilder {
            addr: addr.into(),
            profile: ProfileUnset,
            marker: TokioUdpMarker,
        }
    }
}

// ========================================================================================
// Profile selection - available for all transport markers
// ========================================================================================

impl<K> CameraBuilder<K, ProfileUnset> {
    /// Lock in the compile-time profile and return a builder with that profile.
    pub fn profile<P>(self) -> CameraBuilder<K, P>
    where
        P: Profile + Default,
    {
        CameraBuilder {
            addr: self.addr,
            profile: P::default(),
            marker: self.marker,
        }
    }
}

impl<K, P> CameraBuilder<K, P> {}

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
impl<P: Profile> CameraBuilder<BlockingTcpMarker, P> {
    /// Build the camera with blocking TCP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub fn build(
        self,
    ) -> Result<crate::camera::CameraBlocking<P, crate::transport::blocking::Tcp>, Error> {
        let addr = ensure_port::<P>(&self.addr, Protocol::Tcp);
        let transport = crate::transport::blocking::Tcp::connect(&addr)?;
        Ok(crate::camera::CameraBlocking::from_transport(transport))
    }
}

/// Build implementation for blocking UDP transport.
impl<P: Profile> CameraBuilder<BlockingUdpMarker, P> {
    /// Build the camera with blocking UDP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub fn build(
        self,
    ) -> Result<crate::camera::CameraBlocking<P, crate::transport::blocking::Udp>, Error> {
        let addr = ensure_port::<P>(&self.addr, Protocol::Udp);
        let transport = crate::transport::blocking::Udp::connect(&addr)?;
        Ok(crate::camera::CameraBlocking::from_transport(transport))
    }
}

/// Build implementation for tokio TCP transport.
#[cfg(feature = "rt-tokio")]
impl<P: Profile> CameraBuilder<TokioTcpMarker, P> {
    /// Build the camera with async TCP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub async fn build(
        self,
    ) -> Result<crate::camera::CameraAsync<P, crate::transport::tokio::Tcp>, Error> {
        let addr = ensure_port::<P>(&self.addr, Protocol::Tcp);
        let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
        let mut camera = crate::camera::CameraAsync::from_transport(transport);

        // Always set up runtime and initialize socket manager (actor) for async transports
        #[cfg(feature = "rt-tokio")]
        let runtime = crate::runtime::default_runtime();
        #[cfg(not(feature = "rt-tokio"))]
        let runtime = crate::runtime::default_runtime()?;
        camera = camera.with_runtime(runtime);

        Ok(camera)
    }
}

/// Build implementation for tokio UDP transport.
#[cfg(feature = "rt-tokio")]
impl<P: Profile> CameraBuilder<TokioUdpMarker, P> {
    /// Build the camera with async UDP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address is invalid
    /// - Connection to the camera fails
    pub async fn build(
        self,
    ) -> Result<crate::camera::CameraAsync<P, crate::transport::tokio::Udp>, Error> {
        let addr = ensure_port::<P>(&self.addr, Protocol::Udp);
        let transport = crate::transport::tokio::Udp::connect(&addr).await?;
        let mut camera = crate::camera::CameraAsync::from_transport(transport);

        // Always set up runtime and initialize socket manager (actor) for async transports
        #[cfg(feature = "rt-tokio")]
        let runtime = crate::runtime::default_runtime();
        #[cfg(not(feature = "rt-tokio"))]
        let runtime = crate::runtime::default_runtime()?;
        camera = camera.with_runtime(runtime);

        Ok(camera)
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
        assert_eq!(typed.addr, "192.168.0.110:52381");
    }

    #[test]
    fn test_udp_builder_creation() {
        let builder = CameraBuilder::udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.addr, "239.0.0.1:52381");
    }

    #[cfg(feature = "rt-tokio")]
    #[test]
    fn test_tokio_tcp_builder_creation() {
        let builder = CameraBuilder::tokio_tcp("192.168.0.110:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert_eq!(typed.addr, "192.168.0.110:52381");
    }

    #[cfg(feature = "rt-tokio")]
    #[test]
    fn test_tokio_udp_builder_creation() {
        let builder = CameraBuilder::tokio_udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.addr, "239.0.0.1:52381");
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

        assert_send_sync::<CameraBuilder<BlockingTcpMarker, GenericVisca>>();
        assert_send_sync::<CameraBuilder<BlockingUdpMarker, PTZOpticsG2>>();
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
