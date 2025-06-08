//! Unified extension traits that work with both async and blocking contexts.
//!
//! This module provides a clean API that automatically adapts to the context
//! without code duplication. The key insight is that `ViscaClient` already
//! handles async/sync unification, so we can build on top of that.

use crate::{
    command::{
        InquiryCommand, PowerCommand, power::Power,
        zoom::{ZoomCommand, ZoomSpeed},
        preset::{PresetCommand, PresetAction, PresetNumber},
        pan_tilt::{PanTiltCommand, PanSpeed, TiltSpeed},
    },
    ViscaClient, ViscaError, ViscaResponse, ViscaInquiryResponse,
};
use std::time::Duration;

/// Power control extension for ViscaClient.
///
/// Provides high-level power control methods that work in both
/// async and blocking contexts.
pub trait PowerExt {
    /// Check if the camera is powered on.
    fn is_powered_on(&self) -> Result<bool, ViscaError>;
    
    /// Power on the camera.
    fn power_on(&self) -> Result<(), ViscaError>;
    
    /// Power off the camera (standby mode).
    fn power_off(&self) -> Result<(), ViscaError>;
    
    /// Power cycle the camera with a delay.
    fn power_cycle(&self, delay: Duration) -> Result<(), ViscaError>;
    
    /// Wait for the camera to power on.
    fn wait_for_power_on(&self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError>;
}

/// Async power control extension for ViscaClient.
///
/// Provides async versions of power control methods.
#[cfg(feature = "async-client")]
pub trait AsyncPowerExt {
    /// Check if the camera is powered on.
    async fn is_powered_on_async(&self) -> Result<bool, ViscaError>;
    
    /// Power on the camera.
    async fn power_on_async(&self) -> Result<(), ViscaError>;
    
    /// Power off the camera.
    async fn power_off_async(&self) -> Result<(), ViscaError>;
    
    /// Power cycle the camera with a delay.
    async fn power_cycle_async(&self, delay: Duration) -> Result<(), ViscaError>;
    
    /// Wait for the camera to power on.
    async fn wait_for_power_on_async(&self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError>;
}

// Implement blocking power extension
impl PowerExt for ViscaClient {
    fn is_powered_on(&self) -> Result<bool, ViscaError> {
        let response = self.send(&InquiryCommand::Power)?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
    
    fn power_on(&self) -> Result<(), ViscaError> {
        self.send(&PowerCommand { power: Power::On })?;
        Ok(())
    }
    
    fn power_off(&self) -> Result<(), ViscaError> {
        self.send(&PowerCommand { power: Power::Standby })?;
        Ok(())
    }
    
    fn power_cycle(&self, delay: Duration) -> Result<(), ViscaError> {
        self.power_off()?;
        std::thread::sleep(delay);
        self.power_on()?;
        Ok(())
    }
    
    fn wait_for_power_on(&self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError> {
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
}

// Implement async power extension
#[cfg(feature = "async-client")]
impl AsyncPowerExt for ViscaClient {
    async fn is_powered_on_async(&self) -> Result<bool, ViscaError> {
        let response = self.send_async(&InquiryCommand::Power).await?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
    
    async fn power_on_async(&self) -> Result<(), ViscaError> {
        self.send_async(&PowerCommand { power: Power::On }).await?;
        Ok(())
    }
    
    async fn power_off_async(&self) -> Result<(), ViscaError> {
        self.send_async(&PowerCommand { power: Power::Standby }).await?;
        Ok(())
    }
    
    async fn power_cycle_async(&self, delay: Duration) -> Result<(), ViscaError> {
        self.power_off_async().await?;
        tokio::time::sleep(delay).await;
        self.power_on_async().await?;
        Ok(())
    }
    
    async fn wait_for_power_on_async(&self, timeout: Duration, poll_interval: Duration) -> Result<(), ViscaError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(poll_interval);
        
        loop {
            if self.is_powered_on_async().await? {
                return Ok(());
            }
            
            if tokio::time::Instant::now() >= deadline {
                return Err(ViscaError::Timeout);
            }
            
            interval.tick().await;
        }
    }
}

/// Zoom control extension for ViscaClient.
pub trait ZoomExt {
    /// Zoom to a specific position.
    fn zoom_to(&self, position: u16) -> Result<(), ViscaError>;
    
    /// Zoom in at the specified speed.
    fn zoom_in(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;
    
    /// Zoom out at the specified speed.
    fn zoom_out(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;
    
    /// Stop zooming.
    fn zoom_stop(&self) -> Result<(), ViscaError>;
    
    /// Get the current zoom position.
    fn get_zoom_position(&self) -> Result<u16, ViscaError>;
}

/// Async zoom control extension.
#[cfg(feature = "async-client")]
pub trait AsyncZoomExt {
    /// Zoom to a specific position.
    async fn zoom_to_async(&self, position: u16) -> Result<(), ViscaError>;
    
    /// Zoom in at the specified speed.
    async fn zoom_in_async(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;
    
    /// Zoom out at the specified speed.
    async fn zoom_out_async(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;
    
    /// Stop zooming.
    async fn zoom_stop_async(&self) -> Result<(), ViscaError>;
    
    /// Get the current zoom position.
    async fn get_zoom_position_async(&self) -> Result<u16, ViscaError>;
}

// Implement zoom extensions...
impl ZoomExt for ViscaClient {
    fn zoom_to(&self, position: u16) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::Direct(position))?;
        Ok(())
    }
    
    fn zoom_in(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::TeleVariable(speed))?;
        Ok(())
    }
    
    fn zoom_out(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::WideVariable(speed))?;
        Ok(())
    }
    
    fn zoom_stop(&self) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::Stop)?;
        Ok(())
    }
    
    fn get_zoom_position(&self) -> Result<u16, ViscaError> {
        let response = self.send(&InquiryCommand::ZoomPosition)?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => Ok(position),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
}

#[cfg(feature = "async-client")]
impl AsyncZoomExt for ViscaClient {
    async fn zoom_to_async(&self, position: u16) -> Result<(), ViscaError> {
        self.send_async(&ZoomCommand::Direct(position)).await?;
        Ok(())
    }
    
    async fn zoom_in_async(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send_async(&ZoomCommand::TeleVariable(speed)).await?;
        Ok(())
    }
    
    async fn zoom_out_async(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send_async(&ZoomCommand::WideVariable(speed)).await?;
        Ok(())
    }
    
    async fn zoom_stop_async(&self) -> Result<(), ViscaError> {
        self.send_async(&ZoomCommand::Stop).await?;
        Ok(())
    }
    
    async fn get_zoom_position_async(&self) -> Result<u16, ViscaError> {
        let response = self.send_async(&InquiryCommand::ZoomPosition).await?;
        
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => Ok(position),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
}

/// Preset control extension for ViscaClient.
pub trait PresetExt {
    /// Save current position to a preset.
    fn save_preset(&self, preset_number: u8) -> Result<(), ViscaError>;
    
    /// Recall a preset position.
    fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError>;
    
    /// Reset/clear a preset.
    fn reset_preset(&self, preset_number: u8) -> Result<(), ViscaError>;
}

/// Async preset control extension.
#[cfg(feature = "async-client")]
pub trait AsyncPresetExt {
    /// Save current position to a preset.
    async fn save_preset_async(&self, preset_number: u8) -> Result<(), ViscaError>;
    
    /// Recall a preset position.
    async fn recall_preset_async(&self, preset_number: u8) -> Result<(), ViscaError>;
    
    /// Reset/clear a preset.
    async fn reset_preset_async(&self, preset_number: u8) -> Result<(), ViscaError>;
}

impl PresetExt for ViscaClient {
    fn save_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Set,
            preset_number: preset_num,
        })?;
        Ok(())
    }
    
    fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_num,
        })?;
        Ok(())
    }
    
    fn reset_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset_num,
        })?;
        Ok(())
    }
}

#[cfg(feature = "async-client")]
impl AsyncPresetExt for ViscaClient {
    async fn save_preset_async(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send_async(&PresetCommand {
            action: PresetAction::Set,
            preset_number: preset_num,
        }).await?;
        Ok(())
    }
    
    async fn recall_preset_async(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send_async(&PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_num,
        }).await?;
        Ok(())
    }
    
    async fn reset_preset_async(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send_async(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset_num,
        }).await?;
        Ok(())
    }
}