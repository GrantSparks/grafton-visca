//! Camera builder for runtime configuration and profile selection.
//!
//! Provides a fluent API for creating cameras with different transports and profiles.

use std::marker::PhantomData;

use super::Camera;
use crate::{capabilities::Profile, error::Error};

/// Builder for creating camera instances with runtime parameters first.
///
/// This builder allows you to specify the transport type and address first,
/// then apply the compile-time profile type, and finally build the camera.
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};
///
/// # fn example() -> grafton_visca::Result<()> {
/// // Blocking TCP
/// let camera = CameraBuilder::tcp("192.168.1.100:52381")
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
        let transport = crate::transport::blocking::Tcp::connect(self.addr)?;
        Ok(Camera::from_transport(transport))
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
        let transport = crate::transport::blocking::Udp::connect(self.addr)?;
        Ok(Camera::from_transport(transport))
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
        let transport = crate::transport::tokio::Tcp::connect(self.addr).await?;
        Ok(Camera::from_transport(transport))
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
        let transport = crate::transport::tokio::Udp::connect(self.addr).await?;
        Ok(Camera::from_transport(transport))
    }
}

#[cfg(test)]
mod tests {
    use crate::camera::profiles::{GenericVisca, PTZOpticsG2};

    use super::*;

    #[test]
    fn test_builder_type_inference() {
        let _builder = CameraBuilder::tcp("127.0.0.1:1234").profile::<GenericVisca>();
    }

    #[test]
    fn test_tcp_builder_creation() {
        let builder = CameraBuilder::tcp("192.168.1.100:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert_eq!(typed.addr, "192.168.1.100:52381");
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
        let builder = CameraBuilder::tokio_tcp("192.168.1.100:52381");
        let typed = builder.profile::<PTZOpticsG2>();
        assert_eq!(typed.addr, "192.168.1.100:52381");
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_tokio_udp_builder_creation() {
        let builder = CameraBuilder::tokio_udp("239.0.0.1:52381");
        let typed = builder.profile::<GenericVisca>();
        assert_eq!(typed.addr, "239.0.0.1:52381");
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
