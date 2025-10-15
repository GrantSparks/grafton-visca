//! Resolution mode helpers and utilities.
//!
//! This module provides helper functions for interpreting resolution mode values
//! returned by cameras in response to resolution inquiry commands.

/// Common video resolution modes for Ptz cameras.
///
/// Note: These mappings are based on common PtzOptics camera patterns.
/// Actual mappings may vary by camera model and firmware version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

/// Picture effect modes for Ptz cameras.
///
/// These effects modify the camera's video output for artistic or functional purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum PictureEffectMode {
    /// Normal operation (no effect).
    Off,
    /// Negative image effect.
    Negative,
    /// Black and white effect.
    BlackAndWhite,
    /// Sepia tone effect.
    Sepia,
    /// Sketch effect.
    Sketch,
    /// Emboss effect.
    Emboss,
    /// Mosaic effect.
    Mosaic,
    /// Unknown or camera-specific effect.
    Unknown(u8),
}

impl PictureEffectMode {
    /// Convert a raw picture effect byte to a PictureEffectMode enum.
    ///
    /// # Arguments
    /// * `effect` - The raw effect byte from the camera
    ///
    /// # Returns
    /// The corresponding PictureEffectMode variant
    pub fn from_byte(effect: u8) -> Self {
        match effect {
            0x00 => PictureEffectMode::Off,
            0x01 => PictureEffectMode::Negative,
            0x02 => PictureEffectMode::BlackAndWhite,
            0x03 => PictureEffectMode::Sepia,
            0x04 => PictureEffectMode::Sketch,
            0x05 => PictureEffectMode::Emboss,
            0x06 => PictureEffectMode::Mosaic,
            _ => PictureEffectMode::Unknown(effect),
        }
    }

    /// Get a human-readable description of the picture effect.
    pub fn description(&self) -> &'static str {
        match self {
            PictureEffectMode::Off => "Off (normal)",
            PictureEffectMode::Negative => "Negative",
            PictureEffectMode::BlackAndWhite => "Black & White",
            PictureEffectMode::Sepia => "Sepia",
            PictureEffectMode::Sketch => "Sketch",
            PictureEffectMode::Emboss => "Emboss",
            PictureEffectMode::Mosaic => "Mosaic",
            PictureEffectMode::Unknown(_) => "Unknown picture effect",
        }
    }

    /// Get the raw byte value for this picture effect mode.
    pub fn as_byte(&self) -> u8 {
        match self {
            PictureEffectMode::Off => 0x00,
            PictureEffectMode::Negative => 0x01,
            PictureEffectMode::BlackAndWhite => 0x02,
            PictureEffectMode::Sepia => 0x03,
            PictureEffectMode::Sketch => 0x04,
            PictureEffectMode::Emboss => 0x05,
            PictureEffectMode::Mosaic => 0x06,
            PictureEffectMode::Unknown(value) => *value,
        }
    }
}

/// ND filter positions for cameras with neutral density filters (Sony FR7).
///
/// ND filters reduce light entering the camera without affecting color balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NdFilterPosition {
    /// Clear (no filter applied).
    Clear,
    /// 1/4 ND (2 stops reduction).
    OneQuarter,
    /// 1/8 ND (3 stops reduction).
    OneEighth,
    /// 1/16 ND (4 stops reduction).
    OneSixteenth,
    /// 1/32 ND (5 stops reduction).
    OneThirtySecond,
    /// 1/64 ND (6 stops reduction).
    OneSixtyFourth,
    /// Unknown or camera-specific ND filter setting.
    Unknown(u8),
}

impl NdFilterPosition {
    /// Convert a raw ND filter position byte to an NdFilterPosition enum.
    ///
    /// # Arguments
    /// * `position` - The raw position byte from the camera
    ///
    /// # Returns
    /// The corresponding NdFilterPosition variant
    pub fn from_byte(position: u8) -> Self {
        match position {
            0x00 => NdFilterPosition::Clear,
            0x01 => NdFilterPosition::OneQuarter,
            0x02 => NdFilterPosition::OneEighth,
            0x03 => NdFilterPosition::OneSixteenth,
            0x04 => NdFilterPosition::OneThirtySecond,
            0x05 => NdFilterPosition::OneSixtyFourth,
            _ => NdFilterPosition::Unknown(position),
        }
    }

    /// Get a human-readable description of the ND filter position.
    pub fn description(&self) -> &'static str {
        match self {
            NdFilterPosition::Clear => "Clear (no filter)",
            NdFilterPosition::OneQuarter => "1/4 ND",
            NdFilterPosition::OneEighth => "1/8 ND",
            NdFilterPosition::OneSixteenth => "1/16 ND",
            NdFilterPosition::OneThirtySecond => "1/32 ND",
            NdFilterPosition::OneSixtyFourth => "1/64 ND",
            NdFilterPosition::Unknown(_) => "Unknown ND filter position",
        }
    }

    /// Get the raw byte value for this ND filter position.
    pub fn as_byte(&self) -> u8 {
        match self {
            NdFilterPosition::Clear => 0x00,
            NdFilterPosition::OneQuarter => 0x01,
            NdFilterPosition::OneEighth => 0x02,
            NdFilterPosition::OneSixteenth => 0x03,
            NdFilterPosition::OneThirtySecond => 0x04,
            NdFilterPosition::OneSixtyFourth => 0x05,
            NdFilterPosition::Unknown(value) => *value,
        }
    }

    /// Get the light reduction factor as a rational number (numerator, denominator).
    /// Returns None for Clear or Unknown positions.
    pub fn reduction_factor(&self) -> Option<(u32, u32)> {
        match self {
            NdFilterPosition::Clear => Some((1, 1)),
            NdFilterPosition::OneQuarter => Some((1, 4)),
            NdFilterPosition::OneEighth => Some((1, 8)),
            NdFilterPosition::OneSixteenth => Some((1, 16)),
            NdFilterPosition::OneThirtySecond => Some((1, 32)),
            NdFilterPosition::OneSixtyFourth => Some((1, 64)),
            NdFilterPosition::Unknown(_) => None,
        }
    }

    /// Get the number of f-stops of light reduction.
    /// Returns None for Unknown positions.
    pub fn stops_reduction(&self) -> Option<f32> {
        match self {
            NdFilterPosition::Clear => Some(0.0),
            NdFilterPosition::OneQuarter => Some(2.0),
            NdFilterPosition::OneEighth => Some(3.0),
            NdFilterPosition::OneSixteenth => Some(4.0),
            NdFilterPosition::OneThirtySecond => Some(5.0),
            NdFilterPosition::OneSixtyFourth => Some(6.0),
            NdFilterPosition::Unknown(_) => None,
        }
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
        assert_eq!(NdFilterPosition::Clear.description(), "Clear (no filter)");
        assert_eq!(NdFilterPosition::OneQuarter.description(), "1/4 ND");
        assert_eq!(NdFilterPosition::OneSixtyFourth.description(), "1/64 ND");
        assert_eq!(
            NdFilterPosition::Unknown(0xFF).description(),
            "Unknown ND filter position"
        );
    }

    #[test]
    fn test_picture_effect_descriptions() {
        assert_eq!(PictureEffectMode::Off.description(), "Off (normal)");
        assert_eq!(PictureEffectMode::Negative.description(), "Negative");
        assert_eq!(
            PictureEffectMode::BlackAndWhite.description(),
            "Black & White"
        );
        assert_eq!(
            PictureEffectMode::Unknown(0xFF).description(),
            "Unknown picture effect"
        );
    }
}
