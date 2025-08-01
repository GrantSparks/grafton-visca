//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PTZOptics NDI streaming features.

use crate::{
    command::streaming::{MulticastStreaming, NDIQualityCommand},
    types::NDIQuality,
    Result,
};

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
    /// # let transport = Tcp::connect("192.168.1.100:52381").await?;
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
    /// # let transport = Tcp::connect("192.168.1.100:52381").await?;
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
    /// # let transport = Tcp::connect("192.168.1.100:52381").await?;
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
    /// ```no_run
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::camera::methods::StreamingOpsBlocking;
    /// # use grafton_visca::transport::TcpTransportBlocking;
    /// # use grafton_visca::prelude::blocking::GenericViscaCam;
    /// # let transport = TcpTransportBlocking::connect("192.168.1.100:52381")?;
    /// # let camera = GenericViscaCam::new_blocking(transport);
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
    /// # use grafton_visca::camera::methods::StreamingOpsBlocking;
    /// # use grafton_visca::transport::TcpTransportBlocking;
    /// # use grafton_visca::prelude::blocking::GenericViscaCam;
    /// # let transport = TcpTransportBlocking::connect("192.168.1.100:52381")?;
    /// # let camera = GenericViscaCam::new_blocking(transport);
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
    /// # use grafton_visca::camera::methods::StreamingOpsBlocking;
    /// # use grafton_visca::transport::TcpTransportBlocking;
    /// # use grafton_visca::prelude::blocking::GenericViscaCam;
    /// # use grafton_visca::types::NDIQuality;
    /// # let transport = TcpTransportBlocking::connect("192.168.1.100:52381")?;
    /// # let camera = GenericViscaCam::new_blocking(transport);
    /// camera.set_ndi_quality(NDIQuality::High)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()>;
}

// Implementation for async Camera
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> StreamingOps
    for crate::camera::generic::Camera<P, T>
{
    async fn enable_multicast(&self) -> Result<()> {
        let command = MulticastStreaming::On;
        let response = self.send_command(&command).await?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }

    async fn disable_multicast(&self) -> Result<()> {
        let command = MulticastStreaming::Off;
        let response = self.send_command(&command).await?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }

    async fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()> {
        let command = NDIQualityCommand::new(quality);
        let response = self.send_command(&command).await?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }
}

// Implementation for blocking Camera
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> StreamingOpsBlocking
    for crate::camera::generic::Camera<P, T>
{
    fn enable_multicast(&self) -> Result<()> {
        let command = MulticastStreaming::On;
        let response = self.send_command_blocking(&command)?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }

    fn disable_multicast(&self) -> Result<()> {
        let command = MulticastStreaming::Off;
        let response = self.send_command_blocking(&command)?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }

    fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()> {
        let command = NDIQualityCommand::new(quality);
        let response = self.send_command_blocking(&command)?;
        match response {
            crate::command::Response::Completion => Ok(()),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::error::Error::UnexpectedResponseType),
        }
    }
}
