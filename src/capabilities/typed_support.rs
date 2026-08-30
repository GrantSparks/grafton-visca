//! Runtime representation of profile-backed typed API support.
//!
//! Metadata traits describe runtime discovery facts. `TypedSupportSet` describes
//! the optional typed control, accessor, and inquiry surfaces a profile is
//! permitted to expose.

use std::fmt;

/// Optional typed API surface backed by a profile support marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum TypedSupportSurface {
    /// Direct absolute zoom positioning.
    DirectZoom,
    /// VISCA digital zoom enable/disable command.
    DigitalZoomToggle,
    /// Absolute zoom positions in the optical-plus-digital range.
    DigitalZoomRange,
    /// Direct iris controls and iris inquiries.
    IrisControl,
    /// Standard one-push auto focus trigger.
    OnePushFocus,
    /// PTZOptics snap focus command.
    PtzOpticsSnapFocus,
    /// Focus lock command.
    FocusLock,
    /// Sony push auto focus command.
    PushAutoFocus,
    /// Focus-zone selection command.
    FocusZone,
    /// Auto-focus sensitivity control and inquiry.
    AutoFocusSensitivity,
    /// Focus near-limit inquiry.
    FocusNearLimitInquiry,
    /// Backlight compensation control and inquiry.
    BacklightCompensation,
    /// Wide dynamic range control and inquiry.
    WideDynamicRange,
    /// Exposure compensation controls and inquiries.
    ExposureCompensation,
    /// Exposure brightness controls and inquiry.
    BrightnessControl,
    /// One-push white balance mode and trigger.
    OnePushWhiteBalance,
    /// Auto-tracking white balance mode.
    AutoTrackingWhiteBalance,
    /// Auto white-balance sensitivity control.
    AutoWhiteBalanceSensitivity,
    /// Color temperature controls and inquiry.
    ColorTemperature,
    /// Red/blue gain controls and inquiries.
    RgbGain,
    /// Red/blue tuning controls and inquiries.
    RgbTuning,
    /// Vertical image flip control and inquiry.
    ImageFlip,
    /// Horizontal image mirror control.
    ImageMirror,
    /// Combined image flip mode command.
    CombinedImageFlip,
    /// Contrast control and inquiry.
    ContrastControl,
    /// Sharpness control and inquiry.
    SharpnessControl,
    /// Saturation control and inquiry.
    SaturationControl,
    /// Hue control and inquiry.
    HueControl,
    /// Luminance control and inquiry.
    LuminanceControl,
    /// Gamma control and inquiry.
    GammaControl,
    /// Aggregate noise-reduction inquiries.
    NoiseReduction,
    /// 2D noise-reduction control and inquiry.
    NoiseReduction2D,
    /// 3D noise-reduction control and inquiry.
    NoiseReduction3D,
    /// Picture-effect control and inquiry.
    PictureEffect,
    /// Tally light controls and inquiries.
    Tally,
    /// Direct menu controls.
    DirectMenu,
    /// ND filter controls and inquiries.
    NdFilter,
    /// Variable speed mode controls.
    VariableSpeed,
    /// Motion Sync controls and inquiries.
    MotionSync,
    /// Focus-zone inquiry.
    ///
    /// This is distinct from [`Self::FocusZone`], whose support covers the
    /// selection command. Some profiles document the command but not the
    /// matching status response.
    FocusZoneInquiry,
    /// USB audio control and inquiry.
    UsbAudio,
}

impl TypedSupportSurface {
    /// All known typed support surfaces.
    pub const ALL: [Self; 41] = [
        Self::DirectZoom,
        Self::DigitalZoomToggle,
        Self::DigitalZoomRange,
        Self::IrisControl,
        Self::OnePushFocus,
        Self::PtzOpticsSnapFocus,
        Self::FocusLock,
        Self::PushAutoFocus,
        Self::FocusZone,
        Self::AutoFocusSensitivity,
        Self::FocusNearLimitInquiry,
        Self::BacklightCompensation,
        Self::WideDynamicRange,
        Self::ExposureCompensation,
        Self::BrightnessControl,
        Self::OnePushWhiteBalance,
        Self::AutoTrackingWhiteBalance,
        Self::AutoWhiteBalanceSensitivity,
        Self::ColorTemperature,
        Self::RgbGain,
        Self::RgbTuning,
        Self::ImageFlip,
        Self::ImageMirror,
        Self::CombinedImageFlip,
        Self::ContrastControl,
        Self::SharpnessControl,
        Self::SaturationControl,
        Self::HueControl,
        Self::LuminanceControl,
        Self::GammaControl,
        Self::NoiseReduction,
        Self::NoiseReduction2D,
        Self::NoiseReduction3D,
        Self::PictureEffect,
        Self::Tally,
        Self::DirectMenu,
        Self::NdFilter,
        Self::VariableSpeed,
        Self::MotionSync,
        Self::FocusZoneInquiry,
        Self::UsbAudio,
    ];

    const fn bit(self) -> u64 {
        1 << match self {
            Self::DirectZoom => 0,
            Self::DigitalZoomToggle => 1,
            Self::DigitalZoomRange => 2,
            Self::IrisControl => 3,
            Self::OnePushFocus => 4,
            Self::PtzOpticsSnapFocus => 5,
            Self::FocusLock => 6,
            Self::PushAutoFocus => 7,
            Self::FocusZone => 8,
            Self::AutoFocusSensitivity => 9,
            Self::FocusNearLimitInquiry => 10,
            Self::BacklightCompensation => 11,
            Self::WideDynamicRange => 12,
            Self::ExposureCompensation => 13,
            Self::BrightnessControl => 14,
            Self::OnePushWhiteBalance => 15,
            Self::AutoTrackingWhiteBalance => 16,
            Self::AutoWhiteBalanceSensitivity => 17,
            Self::ColorTemperature => 18,
            Self::RgbGain => 19,
            Self::RgbTuning => 20,
            Self::ImageFlip => 21,
            Self::ImageMirror => 22,
            Self::CombinedImageFlip => 23,
            Self::ContrastControl => 24,
            Self::SharpnessControl => 25,
            Self::SaturationControl => 26,
            Self::HueControl => 27,
            Self::LuminanceControl => 28,
            Self::GammaControl => 29,
            Self::NoiseReduction => 30,
            Self::NoiseReduction2D => 31,
            Self::NoiseReduction3D => 32,
            Self::PictureEffect => 33,
            Self::Tally => 34,
            Self::DirectMenu => 35,
            Self::NdFilter => 36,
            Self::VariableSpeed => 37,
            Self::MotionSync => 38,
            Self::FocusZoneInquiry => 39,
            Self::UsbAudio => 40,
        }
    }
}

/// Compact set of typed API surfaces supported by a profile.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TypedSupportSet(u64);

impl TypedSupportSet {
    /// Empty typed support set.
    pub const EMPTY: Self = Self(0);

    /// Returns an empty typed support set.
    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    /// Creates a set containing one typed support surface.
    #[must_use]
    pub const fn from_surface(surface: TypedSupportSurface) -> Self {
        Self(surface.bit())
    }

    /// Creates a set containing all listed typed support surfaces.
    ///
    /// Duplicate surfaces are ignored because the representation is a bitset.
    #[must_use]
    pub const fn from_surfaces(surfaces: &[TypedSupportSurface]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < surfaces.len() {
            bits |= surfaces[index].bit();
            index += 1;
        }
        Self(bits)
    }

    /// Returns true when the set contains no typed support surfaces.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when this set contains `surface`.
    #[must_use]
    pub const fn contains(self, surface: TypedSupportSurface) -> bool {
        self.0 & surface.bit() != 0
    }

    /// Returns this set without `surface`.
    #[must_use]
    pub const fn without(self, surface: TypedSupportSurface) -> Self {
        Self(self.0 & !surface.bit())
    }

    /// Returns true when this set contains every surface in `required`.
    #[must_use]
    pub const fn contains_all(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Returns the union of two typed support sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Iterates over surfaces present in this set.
    pub fn iter(self) -> impl Iterator<Item = TypedSupportSurface> {
        TypedSupportSurface::ALL
            .into_iter()
            .filter(move |surface| self.contains(*surface))
    }
}

impl fmt::Debug for TypedSupportSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// Profile contract for runtime typed API permission discovery.
///
/// Custom profiles must keep this set in sync with optional marker trait impls.
/// Built-in profiles derive both marker impls and this set from the same
/// registry entry.
pub trait ProfileTypedSupport {
    /// Typed support surfaces intentionally exposed for this profile.
    const TYPED_SUPPORT: TypedSupportSet;
}

#[cfg(feature = "serde")]
impl serde::Serialize for TypedSupportSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TypedSupportSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let surfaces = Vec::<TypedSupportSurface>::deserialize(deserializer)?;
        Ok(Self::from_surfaces(&surfaces))
    }
}

#[cfg(test)]
mod tests {
    use super::{TypedSupportSet, TypedSupportSurface};

    #[test]
    fn empty_set_contains_no_surfaces() {
        let set = TypedSupportSet::empty();

        assert!(set.is_empty());
        assert!(!set.contains(TypedSupportSurface::DirectZoom));
    }

    #[test]
    fn single_surface_set_reports_membership() {
        let set = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);

        assert!(set.contains(TypedSupportSurface::DirectZoom));
        assert!(!set.contains(TypedSupportSurface::DigitalZoomToggle));
    }

    #[test]
    fn combined_set_reports_all_members() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomToggle,
        ]);

        assert!(set.contains(TypedSupportSurface::DirectZoom));
        assert!(set.contains(TypedSupportSurface::DigitalZoomToggle));
        assert!(!set.contains(TypedSupportSurface::FocusZone));
    }

    #[test]
    fn duplicate_surfaces_are_idempotent() {
        let duplicated = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DirectZoom,
        ]);
        let single = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);

        assert_eq!(duplicated, single);
    }

    #[test]
    fn contains_all_requires_every_surface() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomRange,
        ]);

        assert!(set.contains_all(TypedSupportSet::from_surface(
            TypedSupportSurface::DirectZoom
        )));
        assert!(!set.contains_all(TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomToggle,
        ])));
    }

    #[test]
    fn appended_focus_zone_inquiry_and_usb_audio_bits_round_trip() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::FocusZoneInquiry,
            TypedSupportSurface::UsbAudio,
        ]);

        assert!(set.contains(TypedSupportSurface::FocusZoneInquiry));
        assert!(set.contains(TypedSupportSurface::UsbAudio));
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![
                TypedSupportSurface::FocusZoneInquiry,
                TypedSupportSurface::UsbAudio,
            ]
        );
    }
}
