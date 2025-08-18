//! Supporting types used across capability traits.

// Re-export types that are used by multiple capability traits
pub use crate::capabilities::exposure::ShutterSpeed;

/// Coordinate system used by a camera for pan/tilt positions.
///
/// Different camera models use different coordinate representations:
/// - Modern cameras use signed coordinates centered at (0,0)
/// - Legacy cameras (e.g., BRC-300) use unsigned coordinates with (0x8000,0x8000) as center
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    /// Signed coordinates with (0,0) as the center position.
    /// Used by most modern cameras.
    SignedCentered,
    /// Unsigned coordinates with (0x8000,0x8000) as the center position.
    /// Used by legacy cameras like Sony BRC-300.
    UnsignedCentered,
}

impl CoordinateSystem {
    /// Convert logical coordinates (signed, centered at 0) to camera coordinates.
    #[must_use]
    pub fn to_camera_coords(self, pan: i16, tilt: i16) -> (u16, u16) {
        match self {
            Self::SignedCentered => {
                // Direct conversion, interpreting as unsigned
                (pan as u16, tilt as u16)
            }
            Self::UnsignedCentered => {
                // Add offset to center at 0x8000
                let pan_u16 = ((pan as i32) + 0x8000) as u16;
                let tilt_u16 = ((tilt as i32) + 0x8000) as u16;
                (pan_u16, tilt_u16)
            }
        }
    }

    /// Convert camera coordinates to logical coordinates (signed, centered at 0).
    #[must_use]
    pub fn convert_from_camera_coords(self, pan: u16, tilt: u16) -> (i16, i16) {
        match self {
            Self::SignedCentered => {
                // Direct conversion, interpreting as signed
                (pan as i16, tilt as i16)
            }
            Self::UnsignedCentered => {
                // Subtract offset to center at 0
                let pan_i16 = ((pan as i32) - 0x8000) as i16;
                let tilt_i16 = ((tilt as i32) - 0x8000) as i16;
                (pan_i16, tilt_i16)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_centered_coordinates() {
        let coord_system = CoordinateSystem::SignedCentered;

        // Test conversion to camera coords
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(1000, -500);
        assert_eq!(cam_pan, 1000_u16);
        assert_eq!(cam_tilt, (-500_i16) as u16);

        // Test conversion from camera coords
        let (log_pan, log_tilt) = coord_system.convert_from_camera_coords(1000, (-500_i16) as u16);
        assert_eq!(log_pan, 1000);
        assert_eq!(log_tilt, -500);
    }

    #[test]
    fn test_unsigned_centered_coordinates() {
        let coord_system = CoordinateSystem::UnsignedCentered;

        // Test conversion to camera coords (add 0x8000)
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(1000, -500);
        assert_eq!(cam_pan, 0x8000 + 1000);
        assert_eq!(cam_tilt, 0x8000 - 500);

        // Test conversion from camera coords (subtract 0x8000)
        let (log_pan, log_tilt) =
            coord_system.convert_from_camera_coords(0x8000 + 1000, 0x8000 - 500);
        assert_eq!(log_pan, 1000);
        assert_eq!(log_tilt, -500);

        // Test center position
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(0, 0);
        assert_eq!(cam_pan, 0x8000);
        assert_eq!(cam_tilt, 0x8000);

        let (log_pan, log_tilt) = coord_system.convert_from_camera_coords(0x8000, 0x8000);
        assert_eq!(log_pan, 0);
        assert_eq!(log_tilt, 0);
    }
}
