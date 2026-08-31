//! Supporting types used across capability traits.

use std::ops::RangeInclusive;

// Re-export types that are used by multiple capability traits
pub use crate::capabilities::exposure::ShutterSpeed;

/// Inclusive numeric bounds for profile capability metadata.
///
/// VISCA profile facts are closed ranges: both endpoints are supported values.
/// This type keeps those protocol facts explicit in profile traits and registry
/// literals while still exposing standard [`RangeInclusive`] values for runtime
/// discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CapabilityRange<T> {
    min: T,
    max: T,
}

macro_rules! impl_capability_range {
    ($ty:ty) => {
        impl CapabilityRange<$ty> {
            /// Creates an inclusive capability range.
            ///
            /// # Panics
            /// Panics when `min > max`.
            #[must_use]
            pub const fn new(min: $ty, max: $ty) -> Self {
                assert!(min <= max, "capability range minimum exceeds maximum");
                Self { min, max }
            }

            /// Returns the inclusive minimum value.
            #[must_use]
            pub const fn min(self) -> $ty {
                self.min
            }

            /// Returns the inclusive maximum value.
            #[must_use]
            pub const fn max(self) -> $ty {
                self.max
            }

            /// Returns true when `value` is within the closed bounds.
            #[must_use]
            pub const fn contains(self, value: $ty) -> bool {
                value >= self.min && value <= self.max
            }

            /// Clamps `value` to the closed bounds.
            #[must_use]
            pub const fn clamp(self, value: $ty) -> $ty {
                if value < self.min {
                    self.min
                } else if value > self.max {
                    self.max
                } else {
                    value
                }
            }

            /// Converts the closed bounds to a standard inclusive range.
            #[must_use]
            pub fn as_inclusive(self) -> RangeInclusive<$ty> {
                self.min..=self.max
            }
        }
    };
}

impl_capability_range!(u8);
impl_capability_range!(u16);
impl_capability_range!(i8);
impl_capability_range!(i16);
impl_capability_range!(i32);

/// Coordinate system used by a camera for pan/tilt positions.
///
/// Different camera models use different coordinate representations:
/// - Modern cameras use signed coordinates centered at (0,0)
/// - Some cameras use unsigned coordinates with (0x8000,0x8000) as center
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum CoordinateSystem {
    /// Signed coordinates with (0,0) as the center position.
    /// Used by most modern cameras.
    SignedCentered,
    /// Unsigned coordinates with (0x8000,0x8000) as the center position.
    /// Used by cameras whose standard 16-bit fields are offset from center.
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
    fn capability_range_constructs_valid_signed_and_unsigned_ranges() {
        assert_eq!(CapabilityRange::<u8>::new(1_u8, 3).as_inclusive(), 1..=3);
        assert_eq!(
            CapabilityRange::<u16>::new(0_u16, 10).as_inclusive(),
            0..=10
        );
        assert_eq!(CapabilityRange::<i8>::new(-7_i8, 7).as_inclusive(), -7..=7);
        assert_eq!(
            CapabilityRange::<i16>::new(-170_i16, 170).as_inclusive(),
            -170..=170
        );
    }

    #[test]
    fn capability_range_rejects_inverted_ranges() {
        assert!(std::panic::catch_unwind(|| CapabilityRange::<u8>::new(2_u8, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<u16>::new(2_u16, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<i8>::new(2_i8, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<i16>::new(2_i16, 1)).is_err());
    }

    #[test]
    fn capability_range_allows_single_value_ranges() {
        let range = CapabilityRange::<u8>::new(5_u8, 5);

        assert_eq!(range.min(), 5);
        assert_eq!(range.max(), 5);
        assert!(range.contains(5));
        assert!(!range.contains(4));
        assert_eq!(range.clamp(4), 5);
        assert_eq!(range.clamp(6), 5);
        assert_eq!(range.as_inclusive(), 5..=5);
    }

    #[test]
    fn capability_range_represents_full_width_u8_ranges() {
        let range = CapabilityRange::<u8>::new(0_u8, u8::MAX);

        assert!(range.contains(0));
        assert!(range.contains(u8::MAX));
        assert_eq!(range.as_inclusive(), 0..=u8::MAX);
    }

    #[test]
    fn capability_range_methods_work_for_supported_primitives() {
        let u8_range = CapabilityRange::<u8>::new(1_u8, 3);
        assert_eq!(u8_range.min(), 1);
        assert_eq!(u8_range.max(), 3);
        assert!(u8_range.contains(2));
        assert_eq!(u8_range.clamp(0), 1);
        assert_eq!(u8_range.clamp(4), 3);
        assert_eq!(u8_range.as_inclusive(), 1..=3);

        let u16_range = CapabilityRange::<u16>::new(10_u16, 12);
        assert_eq!(u16_range.min(), 10);
        assert_eq!(u16_range.max(), 12);
        assert!(u16_range.contains(11));
        assert_eq!(u16_range.clamp(9), 10);
        assert_eq!(u16_range.clamp(13), 12);
        assert_eq!(u16_range.as_inclusive(), 10..=12);

        let i8_range = CapabilityRange::<i8>::new(-2_i8, 2);
        assert_eq!(i8_range.min(), -2);
        assert_eq!(i8_range.max(), 2);
        assert!(i8_range.contains(0));
        assert_eq!(i8_range.clamp(-3), -2);
        assert_eq!(i8_range.clamp(3), 2);
        assert_eq!(i8_range.as_inclusive(), -2..=2);

        let i16_range = CapabilityRange::<i16>::new(-20_i16, 20);
        assert_eq!(i16_range.min(), -20);
        assert_eq!(i16_range.max(), 20);
        assert!(i16_range.contains(0));
        assert_eq!(i16_range.clamp(-21), -20);
        assert_eq!(i16_range.clamp(21), 20);
        assert_eq!(i16_range.as_inclusive(), -20..=20);
    }

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
