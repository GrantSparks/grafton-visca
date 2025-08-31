//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PtzOptics Ndi streaming features.

use crate::{types::NdiQuality, Result};

/// Operations for controlling network and streaming features.
pub trait StreamingControl {
    /// Enable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # #[cfg(feature = "async")]
    /// # async fn example_async() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::runtime_adapters::tokio::TcpTransport as Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.enable_multicast().await?;
    /// # Ok(())
    /// # }
    /// #
    /// # #[cfg(not(feature = "async"))]
    /// # fn example_blocking() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let mut camera = Camera::new(inner_camera);
    /// camera.enable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    fn enable_multicast(&self) -> impl std::future::Future<Output = Result<()>> + Send + '_;

    /// Enable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    #[cfg(not(feature = "async"))]
    fn enable_multicast(&mut self) -> Result<()>;

    /// Disable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    ///
    /// # Example
    /// ```ignore
    /// # #[cfg(feature = "async")]
    /// # async fn example_async() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::runtime_adapters::tokio::TcpTransport as Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.disable_multicast().await?;
    /// # Ok(())
    /// # }
    /// #
    /// # #[cfg(not(feature = "async"))]
    /// # fn example_blocking() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let mut camera = Camera::new(inner_camera);
    /// camera.disable_multicast()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    fn disable_multicast(&self) -> impl std::future::Future<Output = Result<()>> + Send + '_;

    /// Disable multicast streaming for Ndi cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi multicast.
    #[cfg(not(feature = "async"))]
    fn disable_multicast(&mut self) -> Result<()>;

    /// Set the Ndi streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi quality control.
    ///
    /// # Example
    /// ```ignore
    /// # #[cfg(feature = "async")]
    /// # async fn example_async() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::r#async::{Camera, StreamingControl};
    /// # use grafton_visca::runtime_adapters::tokio::TcpTransport as Tcp;
    /// # use grafton_visca::types::NdiQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381").await?;
    /// # let camera = Camera::new(transport);
    /// camera.set_ndi_quality(NdiQuality::High).await?;
    /// # Ok(())
    /// # }
    /// #
    /// # #[cfg(not(feature = "async"))]
    /// # fn example_blocking() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::blocking::{Camera, StreamingControl};
    /// # use grafton_visca::transport::blocking::Tcp;
    /// # use grafton_visca::types::NdiQuality;
    /// # let transport = Tcp::connect("192.168.0.110:52381")?;
    /// # let inner_camera = grafton_visca::Camera::<grafton_visca::camera::profiles::GenericVisca, _>::new(transport);
    /// # let mut camera = Camera::new(inner_camera);
    /// camera.set_ndi_quality(NdiQuality::High)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    fn set_ndi_quality(
        &self,
        quality: NdiQuality,
    ) -> impl std::future::Future<Output = Result<()>> + Send + '_;

    /// Set the Ndi streaming quality.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support Ndi quality control.
    #[cfg(not(feature = "async"))]
    fn set_ndi_quality(&mut self, quality: NdiQuality) -> Result<()>;
}

// Unified implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> StreamingControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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
        use crate::command::streaming::NdiQualityCommand;

        let cmd = NdiQualityCommand::new(quality);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Unified implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> StreamingControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn enable_multicast(&mut self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;
        let cmd = MulticastStreaming::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_multicast(&mut self) -> Result<()> {
        use crate::command::streaming::MulticastStreaming;

        let cmd = MulticastStreaming::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_ndi_quality(&mut self, quality: NdiQuality) -> Result<()> {
        use crate::command::streaming::NdiQualityCommand;

        let cmd = NdiQualityCommand::new(quality);
        self.send_command(&cmd)?;
        Ok(())
    }
}
