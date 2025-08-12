//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PTZOptics NDI streaming features.

use crate::{types::NDIQuality, Result};

/// Operations for controlling network and streaming features (async).
pub trait StreamingOps: Sized {
    /// Enable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingOps};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.enable_multicast().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn enable_multicast(&self) -> Result<()>;

    /// Disable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingOps};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.disable_multicast().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn disable_multicast(&self) -> Result<()>;

    /// Set the NDI streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI quality control.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingOps};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # use grafton_visca::types::NDIQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.set_ndi_quality(NDIQuality::High).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()>;
}

/// Operations for controlling network and streaming features (blocking).
#[cfg(not(feature = "async"))]
pub trait StreamingOpsBlocking: Sized {
    /// Enable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```no_run
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingOps};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.enable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    fn enable_multicast(&self) -> Result<()>;

    /// Disable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```no_run
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingOps};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.disable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    fn disable_multicast(&self) -> Result<()>;

    /// Set the NDI streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI quality control.
    ///
    /// # Example
    /// ```no_run
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingOps};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # use grafton_visca::types::NDIQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.set_ndi_quality(NDIQuality::High)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()>;
}

// Implementation for async Camera
