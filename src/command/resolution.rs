//! Resolution mode helpers and utilities.
//!
//! This module provides helper functions for interpreting resolution mode values
//! returned by cameras in response to resolution inquiry commands.

/// Common video resolution modes for PTZ cameras.
///
/// Note: These mappings are based on common PTZOptics camera patterns.
/// Actual mappings may vary by camera model and firmware version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionMode {
    /// 1920x1080 @ 60fps
    FullHD60,
    /// 1920x1080 @ 30fps
    FullHD30,
    /// 1920x1080 @ 25fps
    FullHD25,
    /// 1280x720 @ 60fps
    HD60,
    /// 1280x720 @ 30fps
    HD30,
    /// 1280x720 @ 25fps
    HD25,
    /// Unknown or camera-specific mode
    Unknown(u8),
}

impl ResolutionMode {
    /// Convert a raw resolution mode byte to a ResolutionMode enum.
    ///
    /// # Arguments
    /// * `mode` - The raw mode byte from the camera
    ///
    /// # Returns
    /// The corresponding ResolutionMode variant
    pub fn from_byte(mode: u8) -> Self {
        match mode {
            0x00 => ResolutionMode::FullHD60,
            0x01 => ResolutionMode::FullHD30,
            0x02 => ResolutionMode::FullHD25,
            0x03 => ResolutionMode::HD60,
            0x04 => ResolutionMode::HD30,
            0x05 => ResolutionMode::HD25,
            _ => ResolutionMode::Unknown(mode),
        }
    }

    /// Get a human-readable description of the resolution mode.
    pub fn description(&self) -> &'static str {
        match self {
            ResolutionMode::FullHD60 => "1080p60 (1920x1080 @ 60fps)",
            ResolutionMode::FullHD30 => "1080p30 (1920x1080 @ 30fps)",
            ResolutionMode::FullHD25 => "1080p25 (1920x1080 @ 25fps)",
            ResolutionMode::HD60 => "720p60 (1280x720 @ 60fps)",
            ResolutionMode::HD30 => "720p30 (1280x720 @ 30fps)",
            ResolutionMode::HD25 => "720p25 (1280x720 @ 25fps)",
            ResolutionMode::Unknown(_) => "Unknown resolution mode",
        }
    }

    /// Get the width in pixels for this resolution mode.
    pub fn width(&self) -> Option<u32> {
        match self {
            ResolutionMode::FullHD60 | ResolutionMode::FullHD30 | ResolutionMode::FullHD25 => {
                Some(1920)
            }
            ResolutionMode::HD60 | ResolutionMode::HD30 | ResolutionMode::HD25 => Some(1280),
            ResolutionMode::Unknown(_) => None,
        }
    }

    /// Get the height in pixels for this resolution mode.
    pub fn height(&self) -> Option<u32> {
        match self {
            ResolutionMode::FullHD60 | ResolutionMode::FullHD30 | ResolutionMode::FullHD25 => {
                Some(1080)
            }
            ResolutionMode::HD60 | ResolutionMode::HD30 | ResolutionMode::HD25 => Some(720),
            ResolutionMode::Unknown(_) => None,
        }
    }

    /// Get the frame rate for this resolution mode.
    pub fn frame_rate(&self) -> Option<f32> {
        match self {
            ResolutionMode::FullHD60 | ResolutionMode::HD60 => Some(60.0),
            ResolutionMode::FullHD30 | ResolutionMode::HD30 => Some(30.0),
            ResolutionMode::FullHD25 | ResolutionMode::HD25 => Some(25.0),
            ResolutionMode::Unknown(_) => None,
        }
    }
}

/// Helper function to interpret ND filter position values.
///
/// # Arguments
/// * `position` - The raw ND filter position byte
///
/// # Returns
/// A human-readable description of the ND filter setting
pub fn nd_filter_description(position: u8) -> &'static str {
    match position {
        0x00 => "Clear (no filter)",
        0x01 => "1/4 ND",
        0x02 => "1/8 ND",
        0x03 => "1/16 ND",
        0x04 => "1/32 ND",
        0x05 => "1/64 ND",
        _ => "Unknown ND filter position",
    }
}

/// Helper function to interpret picture effect values.
///
/// # Arguments
/// * `effect` - The raw picture effect byte
///
/// # Returns
/// A human-readable description of the picture effect
pub fn picture_effect_description(effect: u8) -> &'static str {
    match effect {
        0x00 => "Off (normal)",
        0x01 => "Negative",
        0x02 => "Black & White",
        0x03 => "Sepia",
        0x04 => "Sketch",
        0x05 => "Emboss",
        0x06 => "Mosaic",
        _ => "Unknown picture effect",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_mode_conversion() {
        assert_eq!(ResolutionMode::from_byte(0x00), ResolutionMode::FullHD60);
        assert_eq!(ResolutionMode::from_byte(0x01), ResolutionMode::FullHD30);
        assert_eq!(ResolutionMode::from_byte(0x04), ResolutionMode::HD30);
        assert_eq!(
            ResolutionMode::from_byte(0xFF),
            ResolutionMode::Unknown(0xFF)
        );
    }

    #[test]
    fn test_resolution_properties() {
        let mode = ResolutionMode::FullHD60;
        assert_eq!(mode.width(), Some(1920));
        assert_eq!(mode.height(), Some(1080));
        assert_eq!(mode.frame_rate(), Some(60.0));
        assert_eq!(mode.description(), "1080p60 (1920x1080 @ 60fps)");

        let unknown = ResolutionMode::Unknown(0x99);
        assert_eq!(unknown.width(), None);
        assert_eq!(unknown.height(), None);
        assert_eq!(unknown.frame_rate(), None);
    }

    #[test]
    fn test_nd_filter_descriptions() {
        assert_eq!(nd_filter_description(0x00), "Clear (no filter)");
        assert_eq!(nd_filter_description(0x01), "1/4 ND");
        assert_eq!(nd_filter_description(0x05), "1/64 ND");
        assert_eq!(nd_filter_description(0xFF), "Unknown ND filter position");
    }

    #[test]
    fn test_picture_effect_descriptions() {
        assert_eq!(picture_effect_description(0x00), "Off (normal)");
        assert_eq!(picture_effect_description(0x01), "Negative");
        assert_eq!(picture_effect_description(0x02), "Black & White");
        assert_eq!(picture_effect_description(0xFF), "Unknown picture effect");
    }
}
