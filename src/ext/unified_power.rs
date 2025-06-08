//! Unified power control extension trait.
//!
//! This module provides power control methods that work seamlessly
//! with both async and blocking contexts.

use crate::{
    command::{InquiryCommand, PowerCommand, power::Power},
    ViscaDevice, ViscaError, ViscaResponse, ViscaInquiryResponse,
};
use std::time::Duration;

/// Unified power control extension trait.
///
/// This trait provides high-level power control methods that work
/// in both async and blocking contexts.
pub trait UnifiedPowerExt: ViscaDevice {
    /// Check if the camera is powered on.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera is powered on
    /// * `Ok(false)` - Camera is powered off or in standby
    ///
    /// # Errors
    /// Returns `ViscaError` if the inquiry fails or returns unexpected data.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaClient, ViscaError};
    /// # use grafton_visca::ext::UnifiedPowerExt;
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example(mut client: ViscaClient) -> Result<(), ViscaError> {
    /// if client.is_powered_on()? {
    ///     println!("Camera is on");
    /// } else {
    ///     println!("Camera is off");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "blocking-client")]
    fn is_powered_on(&mut self) -> Result<bool, ViscaError> {
        let response = self.execute_command(&InquiryCommand::Power)?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
    
    /// Check if the camera is powered on (async version).
    #[cfg(feature = "async-client")]
    async fn is_powered_on(&mut self) -> Result<bool, ViscaError> {
        let response = self.execute_command(&InquiryCommand::Power)?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
    
    /// Power on the camera.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails.
    #[cfg(feature = "blocking-client")]
    fn power_on(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&PowerCommand { power: Power::On })?;
        Ok(())
    }
    
    /// Power on the camera (async version).
    #[cfg(feature = "async-client")]
    async fn power_on(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&PowerCommand { power: Power::On })?;
        Ok(())
    }
    
    /// Power off the camera (standby mode).
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails.
    #[cfg(feature = "blocking-client")]
    fn power_off(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&PowerCommand { power: Power::Standby })?;
        Ok(())
    }
    
    /// Power off the camera (async version).
    #[cfg(feature = "async-client")]
    async fn power_off(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&PowerCommand { power: Power::Standby })?;
        Ok(())
    }
    
    /// Power cycle the camera (turn off and back on).
    ///
    /// This method powers off the camera, waits for the specified delay,
    /// then powers it back on. Useful for resetting the camera state.
    ///
    /// # Arguments
    /// * `delay` - How long to wait between power off and power on
    ///
    /// # Errors
    /// Returns `ViscaError` if either power command fails.
    #[cfg(feature = "blocking-client")]
    fn power_cycle(&mut self, delay: Duration) -> Result<(), ViscaError> {
        self.power_off()?;
        std::thread::sleep(delay);
        self.power_on()?;
        Ok(())
    }
    
    /// Power cycle the camera (async version).
    #[cfg(feature = "async-client")]
    async fn power_cycle(&mut self, delay: Duration) -> Result<(), ViscaError> {
        self.power_off().await?;
        tokio::time::sleep(delay).await;
        self.power_on().await?;
        Ok(())
    }
    
    /// Wait for the camera to power on.
    ///
    /// Polls the camera's power state until it reports being powered on,
    /// or until the timeout is reached.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait
    /// * `poll_interval` - How often to check the power state
    ///
    /// # Returns
    /// * `Ok(())` - Camera is powered on
    /// * `Err(ViscaError::Timeout)` - Timeout reached before camera powered on
    ///
    /// # Errors
    /// Returns `ViscaError` if communication fails or timeout is reached.
    #[cfg(feature = "blocking-client")]
    fn wait_for_power_on(&mut self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError> {
        let start = std::time::Instant::now();
        
        loop {
            if self.is_powered_on()? {
                return Ok(());
            }
            
            if start.elapsed() >= timeout {
                return Err(ViscaError::Timeout);
            }
            
            std::thread::sleep(poll_interval);
        }
    }
    
    /// Wait for the camera to power on (async version).
    #[cfg(feature = "async-client")]
    async fn wait_for_power_on(&mut self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(poll_interval);
        
        loop {
            if self.is_powered_on().await? {
                return Ok(());
            }
            
            if tokio::time::Instant::now() >= deadline {
                return Err(ViscaError::Timeout);
            }
            
            interval.tick().await;
        }
    }
}

// Implement for all ViscaDevice types
impl<T: ViscaDevice> UnifiedPowerExt for T {}