//! High-level API methods for network and streaming control.
//!
//! This module provides convenient methods for controlling PTZOptics NDI streaming features.

use crate::{
    command::streaming::{MulticastStreaming, NDIQualityCommand},
    types::NDIQuality,
    Result,
};

/// Methods for controlling network and streaming features (async).
pub trait StreamingMethods: Sized {
    /// Enable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::{camera::Camera, camera::methods::StreamingMethods};
    /// # let mut camera = Camera::new("192.168.1.100");
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
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::{camera::Camera, camera::methods::StreamingMethods};
    /// # let mut camera = Camera::new("192.168.1.100");
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
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::{camera::Camera, camera::methods::StreamingMethods, types::NDIQuality};
    /// # let mut camera = Camera::new("192.168.1.100");
    /// camera.set_ndi_quality(NDIQuality::High).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()>;
}

/// Methods for controlling network and streaming features (blocking).
pub trait StreamingMethodsBlocking: Sized {
    /// Enable multicast streaming for NDI cameras.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support NDI multicast.
    ///
    /// # Example
    /// ```no_run
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::{blocking::Camera, camera::methods::StreamingMethodsBlocking};
    /// # let mut camera = Camera::new("192.168.1.100");
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
    /// # use grafton_visca::{blocking::Camera, camera::methods::StreamingMethodsBlocking};
    /// # let mut camera = Camera::new("192.168.1.100");
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
    /// # use grafton_visca::{blocking::Camera, camera::methods::StreamingMethodsBlocking, types::NDIQuality};
    /// # let mut camera = Camera::new("192.168.1.100");
    /// camera.set_ndi_quality(NDIQuality::High)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_ndi_quality(&self, quality: NDIQuality) -> Result<()>;
}

// Implementation for async Camera
impl StreamingMethods for crate::camera::Camera {
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
impl StreamingMethodsBlocking for crate::camera::Camera {
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
