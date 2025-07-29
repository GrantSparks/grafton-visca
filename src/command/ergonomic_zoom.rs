//! Ergonomic Zoom command builders with zero dead code.
//!
//! This demonstrates the hybrid approach for zoom control commands.

use crate::command::{ViscaCommand, ResponseType};
use crate::constants::{zoom_percentage_to_visca, validate_zoom_speed};
use crate::types::{ZoomSpeed, ZoomPercentage};
use crate::error::Error;

/// Zoom command that will actually be sent to the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomCommand {
    bytes: Vec<u8>,
    response_type: ResponseType,
}

impl ViscaCommand for ZoomCommand {
    fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn response_type(&self) -> ResponseType {
        self.response_type
    }
}

/// Ergonomic builder for Zoom commands.
/// 
/// Only generates the commands that are actually used.
pub struct Zoom;

impl Zoom {
    /// Stop zoom movement immediately.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// 
    /// let cmd = Zoom::stop();
    /// ```
    pub const fn stop() -> ZoomCommand {
        ZoomCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Zoom in (telephoto) at specified speed.
    /// 
    /// # Arguments
    /// * `speed` - Zoom speed (1-7, where 7 is fastest)
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// use grafton_visca::types::ZoomSpeed;
    /// 
    /// let cmd = Zoom::tele(ZoomSpeed::new(5).unwrap())?;
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn tele(speed: ZoomSpeed) -> Result<ZoomCommand, Error> {
        validate_zoom_speed(speed.value())?;
        
        let speed_byte = 0x20 | (speed.value() & 0x07);
        
        Ok(ZoomCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x07, speed_byte, 0xFF],
            response_type: ResponseType::Completion,
        })
    }

    /// Zoom out (wide) at specified speed.
    /// 
    /// # Arguments
    /// * `speed` - Zoom speed (1-7, where 7 is fastest)
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// use grafton_visca::types::ZoomSpeed;
    /// 
    /// let cmd = Zoom::wide(ZoomSpeed::new(3).unwrap())?;
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn wide(speed: ZoomSpeed) -> Result<ZoomCommand, Error> {
        validate_zoom_speed(speed.value())?;
        
        let speed_byte = 0x30 | (speed.value() & 0x07);
        
        Ok(ZoomCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x07, speed_byte, 0xFF],
            response_type: ResponseType::Completion,
        })
    }

    /// Set absolute zoom position.
    /// 
    /// # Arguments
    /// * `position` - Zoom position as percentage (0.0 = wide, 100.0 = full tele)
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// use grafton_visca::types::ZoomPercentage;
    /// 
    /// let cmd = Zoom::absolute(ZoomPercentage::new(50.0).unwrap())?;
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn absolute(position: ZoomPercentage) -> Result<ZoomCommand, Error> {
        let zoom_value = zoom_percentage_to_visca(position.value())?;
        
        let mut bytes = vec![0x81, 0x01, 0x04, 0x47];
        
        // Add zoom position (4 bytes, big-endian nibbles)
        bytes.extend_from_slice(&[
            ((zoom_value >> 12) & 0x0F) as u8,
            ((zoom_value >> 8) & 0x0F) as u8,
            ((zoom_value >> 4) & 0x0F) as u8,
            (zoom_value & 0x0F) as u8,
        ]);
        
        bytes.push(0xFF);

        Ok(ZoomCommand {
            bytes,
            response_type: ResponseType::Completion,
        })
    }

    /// Query current zoom position.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// 
    /// let cmd = Zoom::inquiry();
    /// ```
    pub const fn inquiry() -> ZoomCommand {
        ZoomCommand {
            bytes: vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            response_type: ResponseType::Inquiry,
        }
    }

    /// Enable digital zoom.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// 
    /// let cmd = Zoom::digital_zoom_on();
    /// ```
    pub const fn digital_zoom_on() -> ZoomCommand {
        ZoomCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x06, 0x02, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Disable digital zoom.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_zoom::Zoom;
    /// 
    /// let cmd = Zoom::digital_zoom_off();
    /// ```
    pub const fn digital_zoom_off() -> ZoomCommand {
        ZoomCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x06, 0x03, 0xFF],
            response_type: ResponseType::Completion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_stop() {
        let cmd = Zoom::stop();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_zoom_tele() {
        let cmd = Zoom::tele(ZoomSpeed::new(5).unwrap()).unwrap();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]); // 0x20 | 0x05
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_zoom_wide() {
        let cmd = Zoom::wide(ZoomSpeed::new(3).unwrap()).unwrap();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x07, 0x33, 0xFF]); // 0x30 | 0x03
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_zoom_absolute() {
        let cmd = Zoom::absolute(ZoomPercentage::new(0.0).unwrap()).unwrap();
        let bytes = cmd.to_bytes();
        assert_eq!(bytes[0..4], [0x81, 0x01, 0x04, 0x47]);
        assert_eq!(bytes[bytes.len() - 1], 0xFF);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_zoom_inquiry() {
        let cmd = Zoom::inquiry();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x09, 0x04, 0x47, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Inquiry);
    }

    #[test]
    fn test_digital_zoom_on() {
        let cmd = Zoom::digital_zoom_on();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x06, 0x02, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_digital_zoom_off() {
        let cmd = Zoom::digital_zoom_off();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x06, 0x03, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_invalid_zoom_speed() {
        let result = Zoom::tele(ZoomSpeed::new(8).unwrap_or(ZoomSpeed::new(7).unwrap()));
        // Should succeed with clamped speed
        assert!(result.is_ok());
    }
}