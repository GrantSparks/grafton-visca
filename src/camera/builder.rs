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

/// Internal enum for builder results to enable enum-based dispatch
#[allow(dead_code)]
pub(crate) enum BuilderResult<P: Profile> {
    BlockingTcp(Camera<P, crate::transport::blocking::Tcp>),
    BlockingUdp(Camera<P, crate::transport::blocking::Udp>),
    #[cfg(feature = "tokio")]
    TokioTcp(Camera<P, crate::transport::tokio::Tcp>),
    #[cfg(feature = "tokio")]
    TokioUdp(Camera<P, crate::transport::tokio::Udp>),
}


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
        TokioTcpBuilder {
            inner: TcpBuilder { addr }
        }
    }

    /// Create a builder for an async UDP transport (tokio).
    #[cfg(feature = "tokio")]
    pub fn tokio_udp(addr: &'a str) -> TokioUdpBuilder<'a> {
        TokioUdpBuilder {
            inner: UdpBuilder { addr }
        }
    }
}

/// Builder for TCP transport.
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
            inner: TypedBuilder {
                addr: self.addr,
                protocol: Protocol::Tcp,
                runtime: Runtime::Blocking,
                _profile: PhantomData,
            }
        }
    }
}

/// Builder for UDP transport.
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
            inner: TypedBuilder {
                addr: self.addr,
                protocol: Protocol::Udp,
                runtime: Runtime::Blocking,
                _profile: PhantomData,
            }
        }
    }
}

// Tokio builders now return appropriate tokio typed builders
/// Builder for tokio TCP transport.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TokioTcpBuilder<'a> {
    inner: TcpBuilder<'a>,
}

#[cfg(feature = "tokio")]
impl<'a> TokioTcpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedTokioTcpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedTokioTcpBuilder {
            inner: TypedBuilder {
                addr: self.inner.addr,
                protocol: Protocol::Tcp,
                runtime: Runtime::Tokio,
                _profile: PhantomData,
            }
        }
    }
}

/// Builder for tokio UDP transport.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TokioUdpBuilder<'a> {
    inner: UdpBuilder<'a>,
}

#[cfg(feature = "tokio")]
impl<'a> TokioUdpBuilder<'a> {
    /// Lock in the compile-time profile and return a typed builder.
    pub fn profile<P>(self) -> TypedTokioUdpBuilder<'a, P>
    where
        P: Profile,
    {
        TypedTokioUdpBuilder {
            inner: TypedBuilder {
                addr: self.inner.addr,
                protocol: Protocol::Udp,
                runtime: Runtime::Tokio,
                _profile: PhantomData,
            }
        }
    }
}

/// Unified typed builder for all transport configurations
#[derive(Debug)]
pub struct TypedBuilder<'a, P> {
    addr: &'a str,
    protocol: Protocol,
    runtime: Runtime,
    _profile: PhantomData<P>,
}

impl<P: Profile> TypedBuilder<'_, P> {
    // Unified build method for blocking transports using enum-based dispatch
    pub(crate) fn build_blocking(self) -> Result<BuilderResult<P>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Blocking, "build_blocking called with non-blocking runtime");
        let addr = self.ensure_port();
        
        match self.protocol {
            Protocol::Tcp => {
                let transport = crate::transport::blocking::Tcp::connect(&addr)?;
                Ok(BuilderResult::BlockingTcp(Camera::from_transport(transport)))
            }
            Protocol::Udp => {
                let transport = crate::transport::blocking::Udp::connect(&addr)?;
                Ok(BuilderResult::BlockingUdp(Camera::from_transport(transport)))
            }
        }
    }
    
    #[cfg(feature = "tokio")]
    // Unified build method for async transports using enum-based dispatch
    pub(crate) async fn build_async(self) -> Result<BuilderResult<P>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Tokio, "build_async called with non-tokio runtime");
        let addr = self.ensure_port();
        
        match self.protocol {
            Protocol::Tcp => {
                let transport = crate::transport::tokio::Tcp::connect(&addr).await?;
                Ok(BuilderResult::TokioTcp(Camera::from_transport(transport)))
            }
            Protocol::Udp => {
                let transport = crate::transport::tokio::Udp::connect(&addr).await?;
                Ok(BuilderResult::TokioUdp(Camera::from_transport(transport)))
            }
        }
    }
    
    // Legacy methods for backward compatibility - these now use the unified methods
    pub(crate) fn build_tcp(self) -> Result<Camera<P, crate::transport::blocking::Tcp>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Blocking, "build_tcp called with non-blocking runtime");
        debug_assert_eq!(self.protocol, Protocol::Tcp, "build_tcp called on non-TCP builder");
        match self.build_blocking()? {
            BuilderResult::BlockingTcp(camera) => Ok(camera),
            _ => unreachable!("Protocol mismatch in build_tcp"),
        }
    }
    
    pub(crate) fn build_udp(self) -> Result<Camera<P, crate::transport::blocking::Udp>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Blocking, "build_udp called with non-blocking runtime");
        debug_assert_eq!(self.protocol, Protocol::Udp, "build_udp called on non-UDP builder");
        match self.build_blocking()? {
            BuilderResult::BlockingUdp(camera) => Ok(camera),
            _ => unreachable!("Protocol mismatch in build_udp"),
        }
    }
    
    #[cfg(feature = "tokio")]
    pub(crate) async fn build_tokio_tcp(self) -> Result<Camera<P, crate::transport::tokio::Tcp>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Tokio, "build_tokio_tcp called with non-tokio runtime");
        debug_assert_eq!(self.protocol, Protocol::Tcp, "build_tokio_tcp called on non-TCP builder");
        match self.build_async().await? {
            BuilderResult::TokioTcp(camera) => Ok(camera),
            _ => unreachable!("Protocol mismatch in build_tokio_tcp"),
        }
    }
    
    #[cfg(feature = "tokio")]
    pub(crate) async fn build_tokio_udp(self) -> Result<Camera<P, crate::transport::tokio::Udp>, Error> {
        debug_assert_eq!(self.runtime, Runtime::Tokio, "build_tokio_udp called with non-tokio runtime");
        debug_assert_eq!(self.protocol, Protocol::Udp, "build_tokio_udp called on non-UDP builder");
        match self.build_async().await? {
            BuilderResult::TokioUdp(camera) => Ok(camera),
            _ => unreachable!("Protocol mismatch in build_tokio_udp"),
        }
    }
    
    /// Single implementation of port resolution logic
    fn ensure_port(&self) -> String {
        if self.addr.contains(':') {
            self.addr.to_string()
        } else {
            let default_port = self.get_default_port();
            format!("{}:{}", self.addr, default_port)
        }
    }
    
    fn get_default_port(&self) -> u16 {
        match (self.protocol, P::PROTOCOL_STYLE) {
            (Protocol::Tcp, ProtocolStyle::RawVisca) => ports::PTZOPTICS_TCP_PORT,
            (Protocol::Udp, ProtocolStyle::RawVisca) => ports::PTZOPTICS_UDP_PORT,
            (_, ProtocolStyle::SonyEncapsulated { .. }) => ports::SONY_VISCA_PORT,
        }
    }
    
    // Test helper method
    #[cfg(test)]
    pub(crate) fn test_ensure_port(addr: &str, protocol: Protocol, runtime: Runtime) -> String {
        TypedBuilder::<P> {
            addr,
            protocol,
            runtime,
            _profile: PhantomData,
        }.ensure_port()
    }
}

// Wrapper types for backward compatibility - these delegate to the unified builder

/// Typed builder for blocking TCP.
#[derive(Debug)]
pub struct TypedTcpBuilder<'a, P> {
    inner: TypedBuilder<'a, P>,
}

impl<P: Profile> TypedTcpBuilder<'_, P> {
    /// Build the camera with blocking TCP transport.
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Tcp>, Error> {
        self.inner.build_tcp()
    }
    
    #[cfg(test)]
    fn ensure_port(addr: &str) -> String {
        TypedBuilder::<P>::test_ensure_port(addr, Protocol::Tcp, Runtime::Blocking)
    }
}

/// Typed builder for blocking UDP.
#[derive(Debug)]
pub struct TypedUdpBuilder<'a, P> {
    inner: TypedBuilder<'a, P>,
}

impl<P: Profile> TypedUdpBuilder<'_, P> {
    /// Build the camera with blocking UDP transport.
    pub fn build(self) -> Result<Camera<P, crate::transport::blocking::Udp>, Error> {
        self.inner.build_udp()
    }
    
    #[cfg(test)]
    fn ensure_port(addr: &str) -> String {
        TypedBuilder::<P>::test_ensure_port(addr, Protocol::Udp, Runtime::Blocking)
    }
}

/// Typed builder for tokio TCP.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TypedTokioTcpBuilder<'a, P> {
    inner: TypedBuilder<'a, P>,
}

#[cfg(feature = "tokio")]
impl<P: Profile> TypedTokioTcpBuilder<'_, P> {
    /// Build the camera with tokio TCP transport.
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Tcp>, Error> {
        self.inner.build_tokio_tcp().await
    }
    
    #[cfg(test)]
    fn ensure_port(addr: &str) -> String {
        TypedBuilder::<P>::test_ensure_port(addr, Protocol::Tcp, Runtime::Tokio)
    }
}

/// Typed builder for tokio UDP.
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct TypedTokioUdpBuilder<'a, P> {
    inner: TypedBuilder<'a, P>,
}

#[cfg(feature = "tokio")]
impl<P: Profile> TypedTokioUdpBuilder<'_, P> {
    /// Build the camera with tokio UDP transport.
    pub async fn build(self) -> Result<Camera<P, crate::transport::tokio::Udp>, Error> {
        self.inner.build_tokio_udp().await
    }
    
    #[cfg(test)]
    fn ensure_port(addr: &str) -> String {
        TypedBuilder::<P>::test_ensure_port(addr, Protocol::Udp, Runtime::Tokio)
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
        assert_eq!(typed.inner.addr, "192.168.0.110:52381");
    }

    #[test]
    fn test_udp_builder_creation() {
        let builder = CameraBuilder::udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.inner.addr, "239.0.0.1:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_tcp_builder_creation() {
        let builder = CameraBuilder::tokio_tcp("192.168.0.110:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert_eq!(typed.inner.addr, "192.168.0.110:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_udp_builder_creation() {
        let builder = CameraBuilder::tokio_udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.inner.addr, "239.0.0.1:52381");
    }

    #[test]
    fn test_ensure_port_unified() {
        // Test PTZOptics profile with TCP port
        let builder = TypedBuilder::<PTZOpticsG2> {
            addr: "192.168.0.110",
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "192.168.0.110:5678");

        // Test PTZOptics profile with UDP port
        let builder = TypedBuilder::<PTZOpticsG2> {
            addr: "192.168.0.110",
            protocol: Protocol::Udp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "192.168.0.110:1259");

        // Test Sony profile (always uses Sony port regardless of transport)
        let builder = TypedBuilder::<SonyFR7> {
            addr: "192.168.0.112",
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "192.168.0.112:52381");
        
        let builder = TypedBuilder::<SonyFR7> {
            addr: "192.168.0.112",
            protocol: Protocol::Udp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "192.168.0.112:52381");

        // Test with explicit port is preserved
        let builder = TypedBuilder::<PTZOpticsG2> {
            addr: "192.168.0.110:8080",
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "192.168.0.110:8080");

        // Test with hostname
        let builder = TypedBuilder::<PTZOpticsG2> {
            addr: "camera.local",
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "camera.local:5678");

        // Test with IPv6 addresses
        let builder = TypedBuilder::<PTZOpticsG2> {
            addr: "[::1]:8080",
            protocol: Protocol::Tcp,
            runtime: Runtime::Blocking,
            _profile: PhantomData,
        };
        assert_eq!(builder.ensure_port(), "[::1]:8080");
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
