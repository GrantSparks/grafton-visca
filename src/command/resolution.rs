//! Picture-effect and neutral-density filter value types.

/// Source-backed picture effect modes for built-in PTZ profiles.
///
/// Other model-specific values remain available through [`Self::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PictureEffectMode {
    /// Normal operation (no effect).
    Off,
    /// Black and white effect.
    BlackAndWhite,
    /// Unknown or camera-specific effect.
    Unknown(u8),
}

impl PictureEffectMode {
    /// Convert a raw picture effect byte from the picture-effect inquiry to a
    /// `PictureEffectMode`.
    ///
    /// The inquiry reports Off as `02`, while the command selects Off with
    /// `00` (`docs/visca_reference.md` §7.8 and §7.10), so this is
    /// deliberately not the inverse of [`Self::as_byte`] for `Off`.
    ///
    /// # Arguments
    /// * `effect` - The raw effect byte from the camera
    ///
    /// # Returns
    /// The corresponding PictureEffectMode variant
    pub fn from_byte(effect: u8) -> Self {
        match effect {
            0x02 => PictureEffectMode::Off,
            0x04 => PictureEffectMode::BlackAndWhite,
            _ => PictureEffectMode::Unknown(effect),
        }
    }

    /// Get a human-readable description of the picture effect.
    pub fn description(&self) -> &'static str {
        match self {
            PictureEffectMode::Off => "Off (normal)",
            PictureEffectMode::BlackAndWhite => "Black & White",
            PictureEffectMode::Unknown(_) => "Unknown picture effect",
        }
    }

    /// Get the picture-effect command byte for this mode (Off is `00`; see
    /// [`Self::from_byte`]).
    pub fn as_byte(&self) -> u8 {
        match self {
            PictureEffectMode::Off => 0x00,
            PictureEffectMode::BlackAndWhite => 0x04,
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
    /// Every named position; [`Self::as_byte`] holds their wire codes.
    const KNOWN: [Self; 6] = [
        Self::Clear,
        Self::OneQuarter,
        Self::OneEighth,
        Self::OneSixteenth,
        Self::OneThirtySecond,
        Self::OneSixtyFourth,
    ];

    /// Convert a raw ND filter position byte to an NdFilterPosition enum.
    ///
    /// This is the inverse of [`Self::as_byte`], which owns the code table.
    ///
    /// # Arguments
    /// * `position` - The raw position byte from the camera
    ///
    /// # Returns
    /// The corresponding NdFilterPosition variant
    pub fn from_byte(position: u8) -> Self {
        Self::KNOWN
            .into_iter()
            .find(|known| known.as_byte() == position)
            .unwrap_or(Self::Unknown(position))
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
    fn nd_filter_codes_round_trip_through_one_table() {
        for code in 0..=u8::MAX {
            assert_eq!(NdFilterPosition::from_byte(code).as_byte(), code);
        }
        for known in NdFilterPosition::KNOWN {
            assert_eq!(NdFilterPosition::from_byte(known.as_byte()), known);
        }
    }

    #[test]
    fn test_picture_effect_descriptions() {
        assert_eq!(PictureEffectMode::Off.description(), "Off (normal)");
        assert_eq!(
            PictureEffectMode::BlackAndWhite.description(),
            "Black & White"
        );
        assert_eq!(PictureEffectMode::from_byte(0x02), PictureEffectMode::Off);
        assert_eq!(
            PictureEffectMode::from_byte(0x04),
            PictureEffectMode::BlackAndWhite
        );
        assert_eq!(PictureEffectMode::BlackAndWhite.as_byte(), 0x04);
        assert_eq!(
            PictureEffectMode::from_byte(0x01),
            PictureEffectMode::Unknown(0x01)
        );
        assert_eq!(
            PictureEffectMode::from_byte(0x00),
            PictureEffectMode::Unknown(0x00)
        );
        assert_eq!(
            PictureEffectMode::Unknown(0xFF).description(),
            "Unknown picture effect"
        );
    }
}
