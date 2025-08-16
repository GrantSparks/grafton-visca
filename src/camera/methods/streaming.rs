//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PTZOptics NDI streaming features.

use crate::{types::NDIQuality, Result};

/// Operations for controlling network and streaming features (async).
#[cfg(feature = "async")]
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
pub trait StreamingOpsBlocking: Sized {
    /// Enable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```ignore
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
    /// ```ignore
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
    /// ```ignore
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

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> StreamingOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn enable_multicast(&self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;
        let cmd = MulticastStreaming::On;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_multicast(&self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;
        let cmd = MulticastStreaming::Off;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()> {
        use crate::command::streaming::NDIQualityCommand;
        let cmd = NDIQualityCommand::new(quality);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> StreamingOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn enable_multicast(&self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;
        let cmd = MulticastStreaming::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_multicast(&self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;
        let cmd = MulticastStreaming::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()> {
        use crate::command::streaming::NDIQualityCommand;
        let cmd = NDIQualityCommand::new(quality);
        self.send_command(&cmd)?;
        Ok(())
    }
}
