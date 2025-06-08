//! Unified power control extension trait using async-trait.
//!
//! This module provides power control methods that work seamlessly
//! with both async and blocking contexts without code duplication.

use crate::{
    command::{InquiryCommand, PowerCommand, power::Power},
    ViscaDevice, ViscaError, ViscaResponse, ViscaInquiryResponse,
};
use std::time::Duration;

/// Unified power control extension trait.
///
/// This trait provides high-level power control methods that automatically
/// adapt to async or blocking contexts based on the feature flags.
pub trait UnifiedPowerExt: ViscaDevice {
    /// Check if the camera is powered on.
    fn is_powered_on(&mut self) -> impl std::future::Future<Output = Result<bool, ViscaError>> + Send {
        async move {
            let response = self.execute_command(&InquiryCommand::Power)?;
            
            match response {
                ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
                _ => Err(ViscaError::UnexpectedResponseType),
            }
        }
    }
    
    /// Power on the camera.
    fn power_on(&mut self) -> impl std::future::Future<Output = Result<(), ViscaError>> + Send {
        async move {
            self.execute_command(&PowerCommand { power: Power::On })?;
            Ok(())
        }
    }
    
    /// Power off the camera (standby mode).
    fn power_off(&mut self) -> impl std::future::Future<Output = Result<(), ViscaError>> + Send {
        async move {
            self.execute_command(&PowerCommand { power: Power::Standby })?;
            Ok(())
        }
    }
    
    /// Power cycle the camera (turn off and back on).
    fn power_cycle(&mut self, delay: Duration) -> impl std::future::Future<Output = Result<(), ViscaError>> + Send {
        async move {
            self.power_off().await?;
            
            #[cfg(feature = "async-client")]
            tokio::time::sleep(delay).await;
            
            #[cfg(not(feature = "async-client"))]
            std::thread::sleep(delay);
            
            self.power_on().await?;
            Ok(())
        }
    }
    
    /// Wait for the camera to power on.
    fn wait_for_power_on(
        &mut self, 
        timeout: Duration, 
        poll_interval: Duration
    ) -> impl std::future::Future<Output = Result<(), ViscaError>> + Send {
        async move {
            #[cfg(feature = "async-client")]
            {
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
            
            #[cfg(not(feature = "async-client"))]
            {
                let start = std::time::Instant::now();
                
                loop {
                    if self.is_powered_on().await? {
                        return Ok(());
                    }
                    
                    if start.elapsed() >= timeout {
                        return Err(ViscaError::Timeout);
                    }
                    
                    std::thread::sleep(poll_interval);
                }
            }
        }
    }
}

// Implement for all ViscaDevice types
impl<T: ViscaDevice> UnifiedPowerExt for T {}

/// Blocking adapter for unified power extension.
///
/// This adapter allows blocking code to use the async trait methods
/// by immediately polling the futures to completion.
#[cfg(feature = "blocking-client")]
pub trait UnifiedPowerExtBlocking: UnifiedPowerExt {
    /// Check if the camera is powered on (blocking).
    fn is_powered_on_blocking(&mut self) -> Result<bool, ViscaError> {
        futures_lite::future::block_on(self.is_powered_on())
    }
    
    /// Power on the camera (blocking).
    fn power_on_blocking(&mut self) -> Result<(), ViscaError> {
        futures_lite::future::block_on(self.power_on())
    }
    
    /// Power off the camera (blocking).
    fn power_off_blocking(&mut self) -> Result<(), ViscaError> {
        futures_lite::future::block_on(self.power_off())
    }
    
    /// Power cycle the camera (blocking).
    fn power_cycle_blocking(&mut self, delay: Duration) -> Result<(), ViscaError> {
        futures_lite::future::block_on(self.power_cycle(delay))
    }
    
    /// Wait for the camera to power on (blocking).
    fn wait_for_power_on_blocking(&mut self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError> {
        futures_lite::future::block_on(self.wait_for_power_on(timeout, poll_interval))
    }
}

#[cfg(feature = "blocking-client")]
impl<T: UnifiedPowerExt> UnifiedPowerExtBlocking for T {}