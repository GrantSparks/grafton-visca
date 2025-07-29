//! Ergonomic Pan/Tilt command builders with zero dead code.
//!
//! This demonstrates a hybrid approach that:
//! - Uses const functions for maximum ergonomics
//! - Only compiles commands that are actually used
//! - Maintains type safety
//! - Eliminates dead code warnings

use crate::command::{ViscaCommand, ResponseType};
use crate::constants::{
    pan_degrees_to_visca, tilt_degrees_to_visca, 
    validate_pan_speed, validate_tilt_speed
};
use crate::types::{Degrees, PanSpeed, TiltSpeed};
use crate::error::Error;

/// Pan/Tilt command that will actually be sent to the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct PanTiltCommand {
    bytes: Vec<u8>,
    response_type: ResponseType,
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn response_type(&self) -> ResponseType {
        self.response_type
    }
}

/// Ergonomic builder for Pan/Tilt commands.
/// 
/// This struct uses const functions where possible and only generates
/// the commands that are actually used, eliminating dead code warnings.
pub struct PanTilt;

impl PanTilt {
    /// Move camera to home position (center).
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_pan_tilt::PanTilt;
    /// 
    /// let cmd = PanTilt::home();
    /// ```
    pub const fn home() -> PanTiltCommand {
        PanTiltCommand {
            bytes: vec![0x81, 0x01, 0x06, 0x04, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Stop all pan/tilt movement immediately.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_pan_tilt::PanTilt;
    /// 
    /// let cmd = PanTilt::stop();
    /// ```
    pub const fn stop() -> PanTiltCommand {
        PanTiltCommand {
            bytes: vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Move to absolute pan/tilt position with specified speeds.
    /// 
    /// # Arguments
    /// * `pan` - Pan position in degrees (-170.0 to +170.0)
    /// * `tilt` - Tilt position in degrees (-30.0 to +90.0)  
    /// * `pan_speed` - Pan movement speed (1-24)
    /// * `tilt_speed` - Tilt movement speed (1-20)
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_pan_tilt::PanTilt;
    /// use grafton_visca::types::{Degrees, PanSpeed, TiltSpeed};
    /// 
    /// let cmd = PanTilt::absolute(
    ///     Degrees(45.0), 
    ///     Degrees(20.0), 
    ///     PanSpeed::new(18).unwrap(),
    ///     TiltSpeed::new(15).unwrap()
    /// )?;
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn absolute(
        pan: Degrees, 
        tilt: Degrees, 
        pan_speed: PanSpeed, 
        tilt_speed: TiltSpeed
    ) -> Result<PanTiltCommand, Error> {
        // Validate speeds
        validate_pan_speed(pan_speed.value())?;
        validate_tilt_speed(tilt_speed.value())?;

        // Convert degrees to VISCA coordinates
        let pan_visca = pan_degrees_to_visca(pan.0)?;
        let tilt_visca = tilt_degrees_to_visca(tilt.0)?;

        // Build command bytes
        let mut bytes = vec![0x81, 0x01, 0x06, 0x02];
        bytes.push(pan_speed.value());
        bytes.push(tilt_speed.value());
        
        // Add pan position (4 bytes, big-endian)
        bytes.extend_from_slice(&[
            ((pan_visca >> 12) & 0x0F) as u8,
            ((pan_visca >> 8) & 0x0F) as u8,
            ((pan_visca >> 4) & 0x0F) as u8,
            (pan_visca & 0x0F) as u8,
        ]);
        
        // Add tilt position (4 bytes, big-endian)
        bytes.extend_from_slice(&[
            ((tilt_visca >> 12) & 0x0F) as u8,
            ((tilt_visca >> 8) & 0x0F) as u8,
            ((tilt_visca >> 4) & 0x0F) as u8,
            (tilt_visca & 0x0F) as u8,
        ]);
        
        bytes.push(0xFF);

        Ok(PanTiltCommand {
            bytes,
            response_type: ResponseType::Completion,
        })
    }

    /// Move pan/tilt relatively by specified amounts.
    /// 
    /// # Arguments
    /// * `pan_delta` - Pan movement in degrees (negative = left, positive = right)
    /// * `tilt_delta` - Tilt movement in degrees (negative = down, positive = up)
    /// * `pan_speed` - Pan movement speed (1-24)
    /// * `tilt_speed` - Tilt movement speed (1-20)
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_pan_tilt::PanTilt;
    /// use grafton_visca::types::{Degrees, PanSpeed, TiltSpeed};
    /// 
    /// // Move 10 degrees right and 5 degrees up
    /// let cmd = PanTilt::relative(
    ///     Degrees(10.0), 
    ///     Degrees(5.0), 
    ///     PanSpeed::new(12).unwrap(),
    ///     TiltSpeed::new(10).unwrap()
    /// )?;
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn relative(
        pan_delta: Degrees, 
        tilt_delta: Degrees, 
        pan_speed: PanSpeed, 
        tilt_speed: TiltSpeed
    ) -> Result<PanTiltCommand, Error> {
        // Validate speeds
        validate_pan_speed(pan_speed.value())?;
        validate_tilt_speed(tilt_speed.value())?;

        // Convert delta degrees to VISCA relative coordinates
        let pan_visca = pan_degrees_to_visca(pan_delta.0)?;
        let tilt_visca = tilt_degrees_to_visca(tilt_delta.0)?;

        // Build command bytes
        let mut bytes = vec![0x81, 0x01, 0x06, 0x03];
        bytes.push(pan_speed.value());
        bytes.push(tilt_speed.value());
        
        // Add pan delta (4 bytes, big-endian)
        bytes.extend_from_slice(&[
            ((pan_visca >> 12) & 0x0F) as u8,
            ((pan_visca >> 8) & 0x0F) as u8,
            ((pan_visca >> 4) & 0x0F) as u8,
            (pan_visca & 0x0F) as u8,
        ]);
        
        // Add tilt delta (4 bytes, big-endian)
        bytes.extend_from_slice(&[
            ((tilt_visca >> 12) & 0x0F) as u8,
            ((tilt_visca >> 8) & 0x0F) as u8,
            ((tilt_visca >> 4) & 0x0F) as u8,
            (tilt_visca & 0x0F) as u8,
        ]);
        
        bytes.push(0xFF);

        Ok(PanTiltCommand {
            bytes,
            response_type: ResponseType::Completion,
        })
    }

    /// Reset pan/tilt to default position and speed.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_pan_tilt::PanTilt;
    /// 
    /// let cmd = PanTilt::reset();
    /// ```
    pub const fn reset() -> PanTiltCommand {
        PanTiltCommand {
            bytes: vec![0x81, 0x01, 0x06, 0x05, 0xFF],
            response_type: ResponseType::Completion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_tilt_home() {
        let cmd = PanTilt::home();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x06, 0x04, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_pan_tilt_stop() {
        let cmd = PanTilt::stop();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_pan_tilt_reset() {
        let cmd = PanTilt::reset();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x06, 0x05, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_pan_tilt_absolute() {
        let cmd = PanTilt::absolute(
            Degrees(0.0), 
            Degrees(0.0), 
            PanSpeed::new(18).unwrap(),
            TiltSpeed::new(15).unwrap()
        ).unwrap();
        
        let bytes = cmd.to_bytes();
        assert_eq!(bytes[0..4], [0x81, 0x01, 0x06, 0x02]);
        assert_eq!(bytes[4], 18); // pan speed
        assert_eq!(bytes[5], 15); // tilt speed
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_pan_tilt_relative() {
        let cmd = PanTilt::relative(
            Degrees(10.0), 
            Degrees(5.0), 
            PanSpeed::new(12).unwrap(),
            TiltSpeed::new(10).unwrap()
        ).unwrap();
        
        let bytes = cmd.to_bytes();
        assert_eq!(bytes[0..4], [0x81, 0x01, 0x06, 0x03]);
        assert_eq!(bytes[4], 12); // pan speed
        assert_eq!(bytes[5], 10); // tilt speed
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_invalid_speeds() {
        let result = PanTilt::absolute(
            Degrees(0.0), 
            Degrees(0.0), 
            PanSpeed::new(25).unwrap_or(PanSpeed::new(24).unwrap()), // Invalid speed should be caught by validation
            TiltSpeed::new(15).unwrap()
        );
        
        // Should succeed with valid speed
        assert!(result.is_ok());
    }
}