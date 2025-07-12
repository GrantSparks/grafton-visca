//! Pan/Tilt control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera pan (horizontal) and tilt (vertical)
//! movement, including directional movement, absolute positioning, and relative positioning.
//!
//! # Speed Limits
//! - Pan speed: 0x00 to 0x18 (0-24 decimal)
//! - Tilt speed: 0x00 to 0x14 (0-20 decimal)
//!
//! # Example
//! ```ignore
//! # #[cfg(not(feature = "async"))]
//! # {
//! # use grafton_visca::command::pan_tilt::{PanTilt, PanTiltDirection, PanSpeed, TiltSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.1.100:5678").unwrap();
//! // Move camera diagonally up-right
//! let command = PanTilt::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10).unwrap(),
//!     tilt_speed: TiltSpeed::new(0x10).unwrap(),
//! };
//! client.send(&command).unwrap();
//! # }
//! ```

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
    // units::Normalized, // Used in commented out code
};

/// Direction for pan/tilt movement commands.
///
/// Represents the 8 directional movements plus stop.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PanTiltDirection {
    /// Move camera upward (tilt up) while maintaining pan position.
    Up,
    /// Move camera downward (tilt down) while maintaining pan position.
    Down,
    /// Move camera leftward (pan left) while maintaining tilt position.
    Left,
    /// Move camera rightward (pan right) while maintaining tilt position.
    Right,
    /// Move camera diagonally up and to the left.
    UpLeft,
    /// Move camera diagonally up and to the right.
    UpRight,
    /// Move camera diagonally down and to the left.
    DownLeft,
    /// Move camera diagonally down and to the right.
    DownRight,
    /// Stop all pan/tilt movement.
    Stop}

impl PanTiltDirection {
    /// Converts the direction to its VISCA byte representation.
    ///
    /// Returns a tuple of (`pan_direction`, `tilt_direction`) bytes.
    #[must_use]
    pub const fn to_bytes(self) -> (u8, u8) {
        match self {
            Self::Up => (0x03, 0x01),
            Self::Down => (0x03, 0x02),
            Self::Left => (0x01, 0x03),
            Self::Right => (0x02, 0x03),
            Self::UpLeft => (0x01, 0x01),
            Self::UpRight => (0x02, 0x01),
            Self::DownLeft => (0x01, 0x02),
            Self::DownRight => (0x02, 0x02),
            Self::Stop => (0x03, 0x03)}
    }
}

/// Pan/Tilt movement commands.
///
/// Provides various ways to control camera pan and tilt:
/// - `Home` - Return to home position
/// - `Reset` - Reset pan/tilt mechanism
/// - `Move` - Directional movement with speed control
/// - `AbsolutePosition` - Move to exact coordinates
/// - `RelativePosition` - Move relative to current position
#[derive(Debug, Copy, Clone)]
pub enum PanTilt {
    /// Return camera to home position.
    Home,
    /// Reset pan/tilt mechanism.
    Reset,
    /// Move camera in specified direction with given speeds.
    Move {
        /// Direction of movement (8 directions + stop).
        direction: PanTiltDirection,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed},
    /// Move camera to an absolute pan/tilt position.
    ///
    /// The pan and tilt values specify exact coordinates to move to.
    AbsolutePosition {
        /// Absolute pan position to move to.
        pan: PanPosition,
        /// Absolute tilt position to move to.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed},
    /// Move camera relative to its current position.
    ///
    /// The pan and tilt values specify the offset from the current position.
    RelativePosition {
        /// Relative pan movement amount.
        pan: PanPosition,
        /// Relative tilt movement amount.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed}}

impl PanTilt {
    // Note: The absolute_position_degrees method was removed because it referenced
    // an out-of-scope generic parameter. Use the camera facade methods instead:
    //
    // ```compile_fail
    // use grafton_visca::command::pan_tilt::PanTilt;
    // use grafton_visca::units::Degrees;
    //
    // // This would not compile - P is not in scope
    // let cmd = PanTilt::absolute_position_degrees(
    //     Degrees(45.0),
    //     Degrees(30.0)
    // );
    // ```

    /// Create a stop command.
    pub fn stop() -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = crate::validate_all! {
            pan_speed: PanSpeed::new(0),
            tilt_speed: TiltSpeed::new(0)}?;

        Ok(Self::Move {
            direction: PanTiltDirection::Stop,
            pan_speed,
            tilt_speed})
    }
}

impl EncodeVisca for PanTilt {
    type Response = ();
    const MAX_SIZE: usize = 5;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x06;
        buffer[3] = 0x04;
        buffer[4] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

/// Converts a position value to 4 bytes in VISCA nibble format.
fn position_to_bytes(position: i16) -> [u8; 4] {
    let pos_u16 = position as u16;
    [
        ((pos_u16 >> 12) & 0x0F) as u8,
        ((pos_u16 >> 8) & 0x0F) as u8,
        ((pos_u16 >> 4) & 0x0F) as u8,
        (pos_u16 & 0x0F) as u8,
    ]
}
