//! Camera builder for runtime configuration and profile selection.
//!
//! Provides a fluent API for creating cameras with different transports and profiles.

use std::marker::PhantomData;

use super::Camera;
use crate::{capabilities::Profile, error::Error};

/// Runtime selection for transport builders
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Runtime {
    /// Blocking runtime (default)
    Blocking,
    /// Tokio async runtime
    #[cfg(feature = "tokio")]
    Tokio,
}

/// Protocol selection for transport builders
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    /// TCP protocol
    Tcp,
    /// UDP protocol
    Udp,
}

/// Builder for creating camera instances with runtime parameters first.
///
/// This builder allows you to specify the transport type and address first,
/// then apply the compile-time profile type, and finally build the camera.
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
/// // Blocking TCP - port is optional (defaults to profile-specific port)
/// let camera = CameraBuilder::tcp("192.168.0.110")
///     .profile::<PTZOpticsG2>()
///     .build()?;
///     
/// // Or specify a custom port explicitly
/// let camera = CameraBuilder::tcp("192.168.0.110:8080")
///     .profile::<PTZOpticsG2>()
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct CameraBuilder<'a> {
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> CameraBuilder<'a> {
    /// Create a builder for a blocking TCP transport.
    pub fn tcp(addr: &'a str) -> GenericBuilder<'a> {
        GenericBuilder {
            addr,
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
        }
    }

    /// Create a builder for a blocking UDP transport.
    pub fn udp(addr: &'a str) -> GenericBuilder<'a> {
        GenericBuilder {
            addr,
            protocol: Protocol::Udp,
            runtime: Runtime::Blocking,
        }
    }

    /// Create a builder for an async TCP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_tcp(addr: &'a str) -> GenericBuilder<'a> {
        GenericBuilder {
            addr,
            protocol: Protocol::Tcp,
            runtime: Runtime::Tokio,
        }
    }

    /// Create a builder for an async UDP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_udp(addr: &'a str) -> GenericBuilder<'a> {
        GenericBuilder {
            addr,
            protocol: Protocol::Udp,
            runtime: Runtime::Tokio,
        }
    }
}

/// Generic builder for all transport configurations.
#[derive(Debug)]
pub struct GenericBuilder<'a> {
    addr: &'a str,
    protocol: Protocol,
    runtime: Runtime,
}

impl<'a> GenericBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedGenericBuilder<'a, P>
    where
        P: Profile,
    {
        TypedGenericBuilder {
            addr: self.addr,
            protocol: self.protocol,
            runtime: self.runtime,
            _profile: PhantomData,
        }
    }
}

/// Unified typed builder for all transport configurations.
///
/// This is the single generic builder that replaces the old 4 separate builder types.
/// It dynamically chooses the transport type based on runtime and protocol parameters.
#[derive(Debug)]
pub struct TypedGenericBuilder<'a, P> {
    addr: &'a str,
    protocol: Protocol,
    runtime: Runtime,
    _profile: PhantomData<P>,
}

/// Result type for blocking builds.
#[derive(Debug)]
pub enum BlockingCamera<P: Profile> {
    /// TCP transport camera.
    Tcp(Camera<P, crate::transport::blocking::Tcp>),
    /// UDP transport camera.
    Udp(Camera<P, crate::transport::blocking::Udp>),
}

/// Result type for async builds.
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub enum AsyncCamera<P: Profile> {
    /// TCP transport camera.
    Tcp(Camera<P, crate::transport::tokio::Tcp>),
    /// UDP transport camera.
    Udp(Camera<P, crate::transport::tokio::Udp>),
}

impl<'a, P: Profile> TypedGenericBuilder<'a, P> {
    /// Build the camera with the appropriate blocking transport.
    /// 
    /// This method handles both TCP and UDP for blocking runtime.
    pub fn build(self) -> Result<BlockingCamera<P>, Error> {
        // Check runtime
        if self.runtime != Runtime::Blocking {
            return Err(Error::InvalidRequest(
                "Use build_async() for async runtime".into(),
            ));
        }

        let addr = ensure_port::<P>(self.addr, self.protocol);
        
        match self.protocol {
            Protocol::Tcp => {
                let transport = crate::transport::blocking::Tcp::connect(&addr)?;
                Ok(BlockingCamera::Tcp(Camera::from_transport(transport)))
            }
            Protocol::Udp => {
                let transport = crate::transport::blocking::Udp::connect(&addr)?;
                Ok(BlockingCamera::Udp(Camera::from_transport(transport)))
            }
        }
    }

    /// Build the camera with async transport (requires tokio feature).
    #[cfg(feature = "tokio")]
    pub async fn build_async(self) -> Result<AsyncCamera<P>, Error> {
        if self.runtime != Runtime::Tokio {
            return Err(Error::InvalidRequest(
                "Use build() for blocking runtime".into(),
            ));
        }

        let addr = ensure_port::<P>(self.addr, self.protocol);

        match self.protocol {
            Protocol::Tcp => {
                let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
                Ok(AsyncCamera::Tcp(Camera::from_transport(transport)))
            }
            Protocol::Udp => {
                let transport = crate::transport::tokio::Udp::connect(&addr).await?;
                Ok(AsyncCamera::Udp(Camera::from_transport(transport)))
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
        assert_eq!(typed.addr, "192.168.0.110:52381");
    }

    #[test]
    fn test_udp_builder_creation() {
        let builder = CameraBuilder::udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.addr, "239.0.0.1:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_tcp_builder_creation() {
        let builder = CameraBuilder::tokio_tcp("192.168.0.110:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert_eq!(typed.addr, "192.168.0.110:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_udp_builder_creation() {
        let builder = CameraBuilder::tokio_udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.addr, "239.0.0.1:52381");
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
        assert_send_sync::<GenericBuilder>();
        assert_send_sync::<TypedGenericBuilder<GenericVisca>>();
        assert_send_sync::<TypedGenericBuilder<PTZOpticsG2>>();
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