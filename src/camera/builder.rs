//! Camera builder for runtime configuration and profile selection.
//!
//! Provides a fluent API for creating cameras with different transports and profiles.

use std::marker::PhantomData;

use super::Camera;
use crate::{
    capabilities::{Profile, ProtocolStyle},
    constants::ports,
    error::Error,
};

/// Builder for creating camera instances with runtime parameters first.
///
/// This builder allows you to specify the transport type and address first,
/// then apply the compile-time profile type, and finally build the camera.
///
/// ## Automatic Port Selection
///
/// The builder automatically adds the correct default port based on the camera profile
/// if no port is specified:
/// - PTZOptics/Generic VISCA: TCP=5678, UDP=1259  
/// - Sony cameras: 52381
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};
///
/// # fn example() -> grafton_visca::Result<()> {
/// // Blocking TCP - port is optional (defaults to 5678 for PTZOpticsG2)
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
    pub fn tcp(addr: &'a str) -> TcpBuilder<'a> {
        TcpBuilder { addr }
    }

    /// Create a builder for a blocking UDP transport.
    pub fn udp(addr: &'a str) -> UdpBuilder<'a> {
        UdpBuilder { addr }
    }

    /// Create a builder for an async TCP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_tcp(addr: &'a str) -> TokioTcpBuilder<'a> {
        TokioTcpBuilder { addr }
    }

    /// Create a builder for an async UDP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_udp(addr: &'a str) -> TokioUdpBuilder<'a> {
        TokioUdpBuilder { addr }
    }
}

/// Builder for blocking TCP transport.
#[derive(Debug)]
pub struct TcpBuilder<'a> {
    addr: &'a str,
}

impl<'a> TcpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedTcpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedTcpBuilder {
            addr: self.addr,
            _p: PhantomData,
        }
    }
}

/// Builder for blocking UDP transport.
#[derive(Debug)]
pub struct UdpBuilder<'a> {
    addr: &'a str,
}

impl<'a> UdpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedUdpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedUdpBuilder {
            addr: self.addr,
            _p: PhantomData,
        }
    }
}

/// Builder for tokio TCP transport.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TokioTcpBuilder<'a> {
    addr: &'a str,
}

#[cfg(feature = "tokio")]
impl<'a> TokioTcpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedTokioTcpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedTokioTcpBuilder {
            addr: self.addr,
            _p: PhantomData,
        }
    }
}

/// Builder for tokio UDP transport.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TokioUdpBuilder<'a> {
    addr: &'a str,
}

#[cfg(feature = "tokio")]
impl<'a> TokioUdpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedTokioUdpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedTokioUdpBuilder {
            addr: self.addr,
            _p: PhantomData,
        }
    }
}

/// Typed builder for blocking TCP.
#[derive(Debug)]
pub struct TypedTcpBuilder<'a, P> {
    addr: &'a str,
    _p: PhantomData<P>,
}

impl<P: Profile> TypedTcpBuilder<'_, P> {
    /// Build the camera with blocking TCP transport.
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Tcp>, Error> {
        let addr = Self::ensure_port(self.addr);
        let transport = crate::transport::blocking::Tcp::connect(&addr)?;
        Ok(Camera::from_transport(transport))
    }

    fn ensure_port(addr: &str) -> String {
        if addr.contains(':') {
            addr.to_string()
        } else {
            let default_port = match P::PROTOCOL_STYLE {
                ProtocolStyle::RawVisca => ports::PTZOPTICS_TCP_PORT,
                ProtocolStyle::SonyEncapsulated { .. } => ports::SONY_VISCA_PORT,
            };
            format!("{}:{}", addr, default_port)
        }
    }
}

/// Typed builder for blocking UDP.
#[derive(Debug)]
pub struct TypedUdpBuilder<'a, P> {
    addr: &'a str,
    _p: PhantomData<P>,
}

impl<P: Profile> TypedUdpBuilder<'_, P> {
    /// Build the camera with blocking UDP transport.
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Udp>, Error> {
        let addr = Self::ensure_port(self.addr);
        let transport = crate::transport::blocking::Udp::connect(&addr)?;
        Ok(Camera::from_transport(transport))
    }

    fn ensure_port(addr: &str) -> String {
        if addr.contains(':') {
            addr.to_string()
        } else {
            let default_port = match P::PROTOCOL_STYLE {
                ProtocolStyle::RawVisca => ports::PTZOPTICS_UDP_PORT,
                ProtocolStyle::SonyEncapsulated { .. } => ports::SONY_VISCA_PORT,
            };
            format!("{}:{}", addr, default_port)
        }
    }
}

/// Typed builder for tokio TCP.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TypedTokioTcpBuilder<'a, P> {
    addr: &'a str,
    _p: PhantomData<P>,
}

#[cfg(feature = "tokio")]
impl<P: Profile> TypedTokioTcpBuilder<'_, P> {
    /// Build the camera with tokio TCP transport.
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Tcp>, Error> {
        let addr = Self::ensure_port(self.addr);
        let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
        Ok(Camera::from_transport(transport))
    }

    fn ensure_port(addr: &str) -> String {
        if addr.contains(':') {
            addr.to_string()
        } else {
            let default_port = match P::PROTOCOL_STYLE {
                ProtocolStyle::RawVisca => ports::PTZOPTICS_TCP_PORT,
                ProtocolStyle::SonyEncapsulated { .. } => ports::SONY_VISCA_PORT,
            };
            format!("{}:{}", addr, default_port)
        }
    }
}

/// Typed builder for tokio UDP.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TypedTokioUdpBuilder<'a, P> {
    addr: &'a str,
    _p: PhantomData<P>,
}

#[cfg(feature = "tokio")]
impl<P: Profile> TypedTokioUdpBuilder<'_, P> {
    /// Build the camera with tokio UDP transport.
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Udp>, Error> {
        let addr = Self::ensure_port(self.addr);
        let transport = crate::transport::tokio::Udp::connect(&addr).await?;
        Ok(Camera::from_transport(transport))
    }

    fn ensure_port(addr: &str) -> String {
        if addr.contains(':') {
            addr.to_string()
        } else {
            let default_port = match P::PROTOCOL_STYLE {
                ProtocolStyle::RawVisca => ports::PTZOPTICS_UDP_PORT,
                ProtocolStyle::SonyEncapsulated { .. } => ports::SONY_VISCA_PORT,
            };
            format!("{}:{}", addr, default_port)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::camera::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};

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
    fn test_blocking_tcp_port_defaults() {
        // Test PTZOptics uses correct default TCP port
        let _typed = CameraBuilder::tcp("192.168.0.110").profile::<PTZOpticsG2>();
        let addr = TypedTcpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110");
        assert_eq!(addr, "192.168.0.110:5678");

        // Test with explicit port is preserved
        let addr = TypedTcpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110:8080");
        assert_eq!(addr, "192.168.0.110:8080");

        // Test GenericVisca uses correct default TCP port
        let addr = TypedTcpBuilder::<GenericVisca>::ensure_port("192.168.0.111");
        assert_eq!(addr, "192.168.0.111:5678");

        // Test Sony camera uses correct default port
        let addr = TypedTcpBuilder::<SonyFR7>::ensure_port("192.168.0.112");
        assert_eq!(addr, "192.168.0.112:52381");
    }

    #[test]
    fn test_blocking_udp_port_defaults() {
        // Test PTZOptics uses correct default UDP port
        let addr = TypedUdpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110");
        assert_eq!(addr, "192.168.0.110:1259");

        // Test with explicit port is preserved
        let addr = TypedUdpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110:8080");
        assert_eq!(addr, "192.168.0.110:8080");

        // Test GenericVisca uses correct default UDP port
        let addr = TypedUdpBuilder::<GenericVisca>::ensure_port("192.168.0.111");
        assert_eq!(addr, "192.168.0.111:1259");

        // Test Sony camera uses correct default port
        let addr = TypedUdpBuilder::<SonyFR7>::ensure_port("192.168.0.112");
        assert_eq!(addr, "192.168.0.112:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_tcp_port_defaults() {
        // Test PTZOptics uses correct default TCP port
        let addr = TypedTokioTcpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110");
        assert_eq!(addr, "192.168.0.110:5678");

        // Test with explicit port is preserved
        let addr = TypedTokioTcpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110:8080");
        assert_eq!(addr, "192.168.0.110:8080");

        // Test Sony camera uses correct default port
        let addr = TypedTokioTcpBuilder::<SonyFR7>::ensure_port("192.168.0.112");
        assert_eq!(addr, "192.168.0.112:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_udp_port_defaults() {
        // Test PTZOptics uses correct default UDP port
        let addr = TypedTokioUdpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110");
        assert_eq!(addr, "192.168.0.110:1259");

        // Test with explicit port is preserved
        let addr = TypedTokioUdpBuilder::<PTZOpticsG2>::ensure_port("192.168.0.110:8080");
        assert_eq!(addr, "192.168.0.110:8080");

        // Test Sony camera uses correct default port
        let addr = TypedTokioUdpBuilder::<SonyFR7>::ensure_port("192.168.0.112");
        assert_eq!(addr, "192.168.0.112:52381");
    }

    #[test]
    fn test_builder_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<CameraBuilder>();
        assert_send_sync::<TcpBuilder>();
        assert_send_sync::<UdpBuilder>();
        assert_send_sync::<TypedTcpBuilder<GenericVisca>>();
        assert_send_sync::<TypedUdpBuilder<PTZOpticsG2>>();

        #[cfg(feature = "tokio")]
        {
            assert_send_sync::<TokioTcpBuilder>();
            assert_send_sync::<TokioUdpBuilder>();
            assert_send_sync::<TypedTokioTcpBuilder<GenericVisca>>();
            assert_send_sync::<TypedTokioUdpBuilder<PTZOpticsG2>>();
        }
    }

    #[test]
    fn test_invalid_address_handling() {
        let typed = CameraBuilder::tcp("invalid:address:format").profile::<GenericVisca>();
        let result = typed.build();
        assert!(result.is_err());
    }
}
