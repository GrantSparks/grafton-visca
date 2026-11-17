//! Source-backed picture-effect values and checked extension codes.

use crate::wire::WireValue;

/// Picture effects supported by the documented command register.
#[crate::value_enum]
pub enum PictureEffectMode {
    /// Normal image, without an effect.
    #[wire(0)]
    Off,
    /// A black-and-white image.
    #[wire(4)]
    BlackAndWhite,
    /// A checked camera-specific command code.
    Unknown(UnknownPictureEffect),
}

impl PictureEffectMode {
    /// Returns a human-readable description of the effect.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Off => "Off (normal)",
            Self::BlackAndWhite => "Black & White",
            Self::Unknown(_) => "Unknown picture effect",
        }
    }
}

/// A model-specific picture-effect code that cannot alias a named mode.
///
/// `00` and `04` name documented commands; `02` is the inquiry-only Off
/// spelling and cannot be sent as an unknown command code. `FF` is the frame
/// terminator and is never a selector. The selected profile must list any
/// extension value in its picture-effect inventory before it can be sent.
#[crate::value(valid_values=[1, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254], display="Picture Effect {}")]
pub struct UnknownPictureEffect(u8);

/// Decodes the inquiry-only Off alias before applying the command codec.
pub(crate) fn decode_picture_effect_code(value: u8) -> crate::Result<PictureEffectMode> {
    if value == 2 {
        Ok(PictureEffectMode::Off)
    } else {
        PictureEffectMode::from_wire(value)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn picture_effect_command_codes_are_canonical_and_total() {
        for code in 0..=u8::MAX {
            let effect = PictureEffectMode::from_wire(code);
            if matches!(code, 2 | 255) {
                assert!(effect.is_err());
            } else {
                let effect = effect.unwrap();
                assert_eq!(effect.to_wire(), code);
                assert_eq!(
                    PictureEffectMode::from_wire(effect.to_wire()).unwrap(),
                    effect
                );
            }
            assert_eq!(
                UnknownPictureEffect::new(code).is_err(),
                matches!(code, 0 | 2 | 4 | 255)
            );
        }
        assert_eq!(
            decode_picture_effect_code(2).unwrap(),
            PictureEffectMode::Off
        );
        assert_eq!(PictureEffectMode::Off.to_wire(), 0);
        assert_eq!(PictureEffectMode::BlackAndWhite.to_wire(), 4);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn picture_effect_persistence_cannot_create_aliases() {
        for code in 0..=u8::MAX {
            let decoded = serde_json::from_str::<UnknownPictureEffect>(&code.to_string());
            assert_eq!(decoded.is_err(), matches!(code, 0 | 2 | 4 | 255));
        }
        for code in [0, 4, 7] {
            let effect = PictureEffectMode::from_wire(code).unwrap();
            let json = serde_json::to_string(&effect).unwrap();
            assert_eq!(
                serde_json::from_str::<PictureEffectMode>(&json).unwrap(),
                effect
            );
        }
    }
}
