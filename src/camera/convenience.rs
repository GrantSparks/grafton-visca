//! One-liner convenience methods for quick camera setup.
//!
//! This module provides simple, one-line methods to quickly connect to cameras
//! with auto-detection and sensible defaults.

use crate::{
    camera::{config::CameraConfig, session::CameraSession},
    capabilities::Profile,
    error::Error,
};

/// Convenience methods for Camera that provide one-liner setup.
#[derive(Debug, Clone, Copy)]
pub struct Camera;

#[cfg(feature = "async")]
impl Camera {
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
    ) -> Result<
        CameraSession<crate::mode::Async, P, crate::runtime_trait::TransportHandle<R>, R>,
        Error,
    >
    where
        P: Profile + Default,
        R: crate::runtime_trait::Runtime,
    {
        CameraConfig::<P>::new()
            .address(addr)
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
    ) -> Result<
        CameraSession<crate::mode::Async, P, crate::runtime_trait::TransportHandle<R>, R>,
        Error,
    >
    where
        P: Profile + Default,
        R: crate::runtime_trait::Runtime,
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
    ) -> Result<
        CameraSession<crate::mode::Async, P, crate::runtime_trait::TransportHandle<R>, R>,
        Error,
    >
    where
        P: Profile + Default,
        R: crate::runtime_trait::Runtime,
    {
        CameraConfig::<P>::new()
            .udp()
            .address(addr)
            .open_async(runtime)
            .await
    }
}

#[cfg(not(feature = "async"))]
impl Camera {
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
    ) -> Result<
        CameraSession<crate::mode::Blocking, P, Box<dyn crate::transport::SyncTransport>, ()>,
        Error,
    >
    where
        P: Profile + Default,
    {
        CameraConfig::<P>::new()
            .address(addr)
            .auto_protocol()
            .open_blocking()
    }

    /// Open a TCP blocking camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
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
    ) -> Result<
        CameraSession<crate::mode::Blocking, P, Box<dyn crate::transport::SyncTransport>, ()>,
        Error,
    >
    where
        P: Profile + Default,
    {
        CameraConfig::<P>::new().tcp().address(addr).open_blocking()
    }

    /// Open a UDP blocking camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
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
    ) -> Result<
        CameraSession<crate::mode::Blocking, P, Box<dyn crate::transport::SyncTransport>, ()>,
        Error,
    >
    where
        P: Profile + Default,
    {
        CameraConfig::<P>::new().udp().address(addr).open_blocking()
    }
}
