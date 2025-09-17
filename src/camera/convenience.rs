//! One-liner convenience methods for quick camera setup.
//!
//! This module provides simple, one-line methods to quickly connect to cameras
//! with auto-detection and sensible defaults.

#[cfg(feature = "mode-async")]
use crate::camera::CameraSession;
use crate::{camera::config::CameraConfig, capabilities::Profile, error::Error};

/// Convenience methods for connecting to cameras with one-liner setup.
#[derive(Debug, Clone, Copy)]
pub struct Connect;

#[cfg(feature = "mode-async")]
impl Connect {
    /// Open an async camera connection with automatic protocol detection.
    ///
    /// This is the simplest way to connect to a camera asynchronously. It will:
    /// 1. Try Sony encapsulated protocol on port 52381 (UDP)
    /// 2. Try raw VISCA on port 1259 (UDP)
    /// 3. Try raw VISCA on port 5678 (TCP)
    ///
    /// # Arguments
    ///
    /// * `addr` - The camera address (hostname or IP, port optional)
    /// * `runtime` - The async runtime to use
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::SonyFR7};
    ///
    /// let cam = Camera::open_auto_async::<SonyFR7>("192.168.0.108", &tokio_runtime).await?;
    /// cam.power().on().await?;
    /// cam.close().await?;
    /// ```
    pub async fn open_auto_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        use crate::camera::config::TransportOptions;

        let address = addr.into();
        CameraConfig::<P>::new()
            .transport(TransportOptions::Auto { address })
            .auto_protocol()
            .open_async(runtime)
            .await
    }

    /// Open a TCP async camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// let cam = Camera::open_tcp_async::<PtzOpticsG2>("192.168.0.110:5678", &tokio_runtime).await?;
    /// ```
    pub async fn open_tcp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::new()
            .tcp()
            .address(addr)
            .open_async(runtime)
            .await
    }

    /// Open a UDP async camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::GenericVisca};
    ///
    /// let cam = Camera::open_udp_async::<GenericVisca>("192.168.0.110:1259", &tokio_runtime).await?;
    /// ```
    pub async fn open_udp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::new()
            .udp()
            .address(addr)
            .open_async(runtime)
            .await
    }

    /// Open a serial async camera connection.
    ///
    /// Connects to the camera using serial port with the profile's default protocol style.
    /// This method is generic over any runtime that implements `RuntimeSerial`.
    ///
    /// # Arguments
    ///
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    /// * `runtime` - The async runtime (must implement RuntimeSerial trait)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let cam = Camera::open_serial_async::<PtzOpticsG2, _>("/dev/ttyUSB0", 9600, runtime).await?;
    /// cam.power().on().await?;
    /// cam.close().await?;
    /// ```
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_async<P, R>(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: R,
    ) -> Result<
        CameraSession<
            crate::mode::Async,
            P,
            <R as crate::runtime::RuntimeSerial>::SerialTransport,
            R,
        >,
        Error,
    >
    where
        P: Profile + Default,
        R: crate::runtime::Runtime + crate::runtime::RuntimeSerial,
    {
        CameraConfig::<P>::new()
            .serial(port, baud_rate)
            .open_serial_async(runtime)
            .await
    }
}

#[cfg(not(feature = "mode-async"))]
impl Connect {
    /// Open a blocking camera connection with automatic protocol detection.
    ///
    /// This is the simplest way to connect to a camera in blocking mode. It will:
    /// 1. Try Sony encapsulated protocol on port 52381 (UDP)
    /// 2. Try raw VISCA on port 1259 (UDP)
    /// 3. Try raw VISCA on port 5678 (TCP)
    ///
    /// # Arguments
    ///
    /// * `addr` - The camera address (hostname or IP, port optional)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// let cam = Camera::open_auto_blocking::<PtzOpticsG2>("192.168.0.110")?;
    /// cam.power().on()?;
    /// cam.close()?;
    /// ```
    pub fn open_auto_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        use crate::camera::config::TransportOptions;

        let address = addr.into();
        let session = CameraConfig::<P>::new()
            .transport(TransportOptions::Auto { address })
            .auto_protocol()
            .open_blocking()?;
        Ok(crate::BlockingClient::from_camera(session.into_inner()))
    }

    /// Open a TCP blocking camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
    /// Returns a camera using BlockingTransportHandle for zero-cost operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// let cam = Camera::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    /// ```
    pub fn open_tcp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        let tcp = crate::transport::blocking::tcp::Tcp::connect(&addr.into())?;
        let transport = crate::transport::BlockingTransportHandle::Tcp(tcp);
        let camera = crate::camera::Camera::new_blocking_with_style(transport, P::PROTOCOL_STYLE)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open a UDP blocking camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
    /// Returns a camera using BlockingTransportHandle for zero-cost operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::GenericVisca};
    ///
    /// let cam = Camera::open_udp_blocking::<GenericVisca>("192.168.0.110:1259")?;
    /// ```
    pub fn open_udp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        let udp = crate::transport::blocking::udp::Udp::connect(&addr.into())?;
        let transport = crate::transport::BlockingTransportHandle::Udp(udp);
        let camera = crate::camera::Camera::new_blocking_with_style(transport, P::PROTOCOL_STYLE)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open a serial blocking camera connection.
    ///
    /// Connects to the camera using serial port with the profile's default protocol style.
    ///
    /// # Arguments
    ///
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// let cam = Camera::open_serial_blocking::<PtzOpticsG2>("/dev/ttyUSB0", 9600)?;
    /// cam.power().on()?;
    /// cam.close()?;
    /// ```
    #[cfg(feature = "transport-serial")]
    pub fn open_serial_blocking<P>(
        port: impl Into<String>,
        baud_rate: u32,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        let session = CameraConfig::<P>::new()
            .serial(port, baud_rate)
            .open_serial_blocking()?;
        Ok(crate::BlockingClient::from_camera(session.into_inner()))
    }
}
