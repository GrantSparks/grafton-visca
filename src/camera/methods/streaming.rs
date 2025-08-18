//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PtzOptics Ndi streaming features.

use crate::{types::NdiQuality, Result};

/// Operations for controlling network and streaming features (async).
#[cfg(feature = "async")]
pub trait StreamingControl: Sized {
    /// Enable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.enable_multicast().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn enable_multicast(&self) -> Result<()>;

    /// Disable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.disable_multicast().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn disable_multicast(&self) -> Result<()>;

    /// Set the Ndi streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi quality control.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// # use grafton_visca::types::NdiQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.set_ndi_quality(NdiQuality::High).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set_ndi_quality(&self, quality: NdiQuality) -> Result<()>;
}

/// Operations for controlling network and streaming features (blocking).
pub trait StreamingControlBlocking: Sized {
    /// Enable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.enable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    fn enable_multicast(&self) -> Result<()>;

    /// Disable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.disable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    fn disable_multicast(&self) -> Result<()>;

    /// Set the Ndi streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi quality control.
    ///
    /// # Example
    /// ```ignore
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # use grafton_visca::types::NdiQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let camera = Camera::new(inner_camera);
    /// camera.set_ndi_quality(NdiQuality::High)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_ndi_quality(&self, quality: NdiQuality) -> Result<()>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> StreamingControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
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

    async fn set_ndi_quality(&self, quality: NdiQuality) -> Result<()> {
        use crate::command::streaming::NdiQualityCmd;

        let cmd = NdiQualityCmd::new(quality);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> StreamingControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
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

    fn set_ndi_quality(&self, quality: NdiQuality) -> Result<()> {
        use crate::command::streaming::NdiQualityCmd;

        let cmd = NdiQualityCmd::new(quality);
        self.send_command(&cmd)?;
        Ok(())
    }
}
