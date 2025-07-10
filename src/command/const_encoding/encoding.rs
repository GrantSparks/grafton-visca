//! Const and runtime encoding functions for VISCA commands.

use crate::command::const_encoding::{CommandBuilder, DEFAULT_ADDRESS};

/// Encoding error types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncodingError {
    /// Buffer is too small for the command.
    BufferTooSmall,
    /// Invalid parameter value.
    InvalidParameter(&'static str),
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingError::BufferTooSmall => write!(f, "Buffer too small for command"),
            EncodingError::InvalidParameter(param) => write!(f, "Invalid parameter: {}", param),
        }
    }
}

impl std::error::Error for EncodingError {}

// Pan/Tilt encoding functions

/// Const function for pan/tilt absolute position.
///
/// Creates a 15-byte command for absolute pan/tilt positioning.
pub const fn encode_pan_tilt_absolute(
    pan: i16, 
    tilt: i16, 
    pan_speed: u8, 
    tilt_speed: u8
) -> [u8; 15] {
    let pan_speed = if pan_speed > 0x18 { 0x18 } else { pan_speed };
    let tilt_speed = if tilt_speed > 0x14 { 0x14 } else { tilt_speed };
    
    CommandBuilder::<15>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x06, 0x02])
        .byte(pan_speed)
        .byte(tilt_speed)
        .visca_i16(pan)
        .visca_i16(tilt)
        .build()
}

/// Const function for pan/tilt relative position.
pub const fn encode_pan_tilt_relative(
    pan: i16,
    tilt: i16,
    pan_speed: u8,
    tilt_speed: u8
) -> [u8; 15] {
    let pan_speed = if pan_speed > 0x18 { 0x18 } else { pan_speed };
    let tilt_speed = if tilt_speed > 0x14 { 0x14 } else { tilt_speed };
    
    CommandBuilder::<15>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x06, 0x03])
        .byte(pan_speed)
        .byte(tilt_speed)
        .visca_i16(pan)
        .visca_i16(tilt)
        .build()
}

/// Const function for pan/tilt directional movement.
pub const fn encode_pan_tilt_move(
    direction: u8,
    pan_speed: u8,
    tilt_speed: u8
) -> [u8; 9] {
    let pan_speed = if pan_speed > 0x18 { 0x18 } else { pan_speed };
    let tilt_speed = if tilt_speed > 0x14 { 0x14 } else { tilt_speed };
    
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x06, 0x01])
        .byte(pan_speed)
        .byte(tilt_speed)
        .byte(direction)
        .build()
}

// Zoom encoding functions

/// Const function for zoom direct position.
pub const fn encode_zoom_direct(position: u16) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x47])
        .visca_u16(position)
        .build()
}

/// Const function for variable speed zoom.
pub const fn encode_zoom_variable(direction: u8, speed: u8) -> [u8; 6] {
    let speed = if speed > 7 { 7 } else { speed };
    let cmd = 0x20 | (direction & 0x10) | (speed & 0x0F);
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x07])
        .byte(cmd)
        .build()
}

// Focus encoding functions

/// Const function for focus direct position.
pub const fn encode_focus_direct(position: u16) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x48])
        .visca_u16(position)
        .build()
}

/// Const function for variable speed focus.
pub const fn encode_focus_variable(direction: u8, speed: u8) -> [u8; 6] {
    let speed = if speed > 7 { 7 } else { speed };
    let cmd = 0x20 | (direction & 0x10) | (speed & 0x0F);
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x08])
        .byte(cmd)
        .build()
}

// Preset encoding functions

/// Const function for preset operations.
pub const fn encode_preset_recall(preset_number: u8) -> [u8; 7] {
    CommandBuilder::<7>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x3F, 0x02])
        .byte(preset_number)
        .build()
}

/// Const function for preset set.
pub const fn encode_preset_set(preset_number: u8) -> [u8; 7] {
    CommandBuilder::<7>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x3F, 0x01])
        .byte(preset_number)
        .build()
}

/// Const function for preset reset.
pub const fn encode_preset_reset(preset_number: u8) -> [u8; 7] {
    CommandBuilder::<7>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x3F, 0x00])
        .byte(preset_number)
        .build()
}

// Exposure encoding functions

/// Runtime helper for complex exposure encoding.
pub fn encode_exposure_manual(
    iris: u16,
    shutter_speed: u8,
    gain: u8
) -> Result<[u8; 15], EncodingError> {
    // Validate parameters
    if iris > 0x1C {
        return Err(EncodingError::InvalidParameter("iris out of range"));
    }
    if shutter_speed > 0x15 {
        return Err(EncodingError::InvalidParameter("shutter speed out of range"));
    }
    if gain > 0x0F {
        return Err(EncodingError::InvalidParameter("gain out of range"));
    }
    
    Ok(CommandBuilder::<15>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x39])
        .byte(0x03) // Manual mode
        .byte(0x00) // Reserved
        .byte(0x00) // Reserved  
        .byte(iris as u8)
        .byte(0x00) // Reserved
        .byte(0x00) // Reserved
        .byte(shutter_speed)
        .byte(0x00) // Reserved
        .byte(0x00) // Reserved
        .byte(gain)
        .build())
}

/// Const function for iris direct.
pub const fn encode_iris_direct(value: u16) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x4B])
        .visca_u16(value)
        .build()
}

/// Const function for shutter direct.
pub const fn encode_shutter_direct(value: u16) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x4A])
        .visca_u16(value)
        .build()
}

/// Const function for gain direct.
pub const fn encode_gain_direct(value: u8) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x4C])
        .visca_u16(value as u16)
        .build()
}

// ND Filter encoding (for cameras that support it)

/// Const function for ND filter control.
pub const fn encode_nd_filter_fixed(enabled: bool) -> [u8; 6] {
    let value = if enabled { 0x02 } else { 0x03 };
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x37])
        .byte(value)
        .build()
}

/// Const function for stepped ND filter.
pub const fn encode_nd_filter_stepped(step: u8) -> [u8; 6] {
    let step = if step > 0x03 { 0x03 } else { step };
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x37])
        .byte(step)
        .build()
}

/// Const function for variable ND filter.
pub const fn encode_nd_filter_variable(value: u8) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x47])
        .visca_u16(value as u16)
        .build()
}

// Helper functions for encoding

/// Encode a speed value for zoom/focus commands.
pub const fn encode_speed(speed: u8) -> u8 {
    let speed = if speed > 7 { 7 } else { speed };
    0x20 | (speed & 0x0F)
}

/// Encode a 16-bit value in VISCA format.
pub fn encode_u16_visca(value: u16, builder: &mut CommandBuilder<10>) {
    builder.push((value >> 12) as u8 & 0x0F);
    builder.push((value >> 8) as u8 & 0x0F);
    builder.push((value >> 4) as u8 & 0x0F);
    builder.push(value as u8 & 0x0F);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pan_tilt_absolute() {
        let cmd = encode_pan_tilt_absolute(1000, -500, 10, 10);
        assert_eq!(cmd[0..4], [0x81, 0x01, 0x06, 0x02]);
        assert_eq!(cmd[4], 10); // pan speed
        assert_eq!(cmd[5], 10); // tilt speed
        assert_eq!(cmd[14], 0xFF); // terminator
    }
    
    #[test]
    fn test_zoom_direct() {
        let cmd = encode_zoom_direct(0x4000);
        assert_eq!(
            cmd,
            [0x81, 0x01, 0x04, 0x47, 0x04, 0x00, 0x00, 0x00, 0xFF]
        );
    }
    
    #[test]
    fn test_preset_recall() {
        let cmd = encode_preset_recall(5);
        assert_eq!(
            cmd,
            [0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF]
        );
    }
    
    #[test]
    fn test_const_at_compile_time() {
        // This demonstrates that these functions can be used in const context
        const HOME_POS: [u8; 15] = encode_pan_tilt_absolute(0, 0, 24, 24);
        const ZOOM_MID: [u8; 9] = encode_zoom_direct(0x2000);
        
        assert_eq!(HOME_POS[4], 24);
        assert_eq!(ZOOM_MID[4], 0x02);
    }
}