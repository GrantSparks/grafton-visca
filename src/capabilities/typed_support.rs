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
    /// Standard iris reset/up/down/direct controls and the `09 04 4B`
    /// iris-position inquiry.
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
    /// 2D noise-reduction mode and level inquiries.
    NoiseReduction2D,
    /// 3D noise-reduction level inquiry.
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
    /// PTZOptics anti-flicker control and inquiry.
    PtzOpticsAntiFlicker,
    /// PTZOptics persistent-settings save command.
    PtzOpticsSettingsSave,
    /// PTZOptics preset-recall speed control.
    PtzOpticsPresetRecallSpeed,
    /// Sony VISCA spotlight controls.
    SonySpotlight,
    /// Sony VISCA automatic slow-shutter controls.
    SonyAutoSlowShutter,
    /// PTZOptics multicast-streaming controls.
    PtzOpticsMulticastStreaming,
    /// PTZOptics NDI-quality control.
    PtzOpticsNdiQuality,
    /// Shared VISCA exposure-mode control and inquiry.
    ExposureMode,
    /// Standard `09 04 2B` iris auto/manual status inquiry.
    ///
    /// This remains distinct from [`Self::IrisControl`] because position and
    /// status inquiries are independently documented profile surfaces.
    IrisControlInquiry,
    /// 2D noise-reduction mode and level controls.
    ///
    /// This is deliberately independent from [`Self::NoiseReduction2D`],
    /// whose source-backed permission covers only inquiries.
    NoiseReduction2DControl,
    /// 3D noise-reduction level controls.
    ///
    /// This is deliberately independent from [`Self::NoiseReduction3D`],
    /// whose source-backed permission covers only inquiries.
    NoiseReduction3DControl,
}

impl TypedSupportSurface {
    /// All known typed support surfaces.
    pub const ALL: [Self; 51] = [
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
        Self::PtzOpticsAntiFlicker,
        Self::PtzOpticsSettingsSave,
        Self::PtzOpticsPresetRecallSpeed,
        Self::SonySpotlight,
        Self::SonyAutoSlowShutter,
        Self::PtzOpticsMulticastStreaming,
        Self::PtzOpticsNdiQuality,
        Self::ExposureMode,
        Self::IrisControlInquiry,
        Self::NoiseReduction2DControl,
        Self::NoiseReduction3DControl,
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
            // Bit 30 remains reserved for the removed aggregate NR surface,
            // preserving persisted support-set encodings for every survivor.
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
            Self::PtzOpticsAntiFlicker => 41,
            Self::PtzOpticsSettingsSave => 42,
            Self::PtzOpticsPresetRecallSpeed => 43,
            Self::SonySpotlight => 44,
            Self::SonyAutoSlowShutter => 45,
            Self::PtzOpticsMulticastStreaming => 46,
            Self::PtzOpticsNdiQuality => 47,
            // Appended after all existing persisted support-set bits.
            Self::ExposureMode => 48,
            // Appended after all existing persisted support-set bits.
            Self::IrisControlInquiry => 49,
            // Appended after all existing persisted support-set bits.
            Self::NoiseReduction2DControl => 50,
            // Appended after all existing persisted support-set bits.
            Self::NoiseReduction3DControl => 51,
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

    #[test]
    fn appended_noise_reduction_control_bits_preserve_existing_encodings() {
        assert_eq!(
            TypedSupportSet::from_surface(TypedSupportSurface::PtzOpticsNdiQuality).0,
            1 << 47
        );
        assert_eq!(
            TypedSupportSet::from_surface(TypedSupportSurface::ExposureMode).0,
            1 << 48
        );
        assert_eq!(
            TypedSupportSet::from_surface(TypedSupportSurface::IrisControlInquiry).0,
            1 << 49
        );
        assert_eq!(
            TypedSupportSet::from_surface(TypedSupportSurface::NoiseReduction2DControl).0,
            1 << 50
        );
        assert_eq!(
            TypedSupportSet::from_surface(TypedSupportSurface::NoiseReduction3DControl).0,
            1 << 51
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_the_historical_wire_contract_and_appends_noise_reduction_controls(
    ) -> Result<(), serde_json::Error> {
        // This table is intentionally independent of `TypedSupportSurface::ALL`
        // and serde's derive-generated spelling.  It pins each public wire tag
        // to its exact semantic surface, so a swapped decoder, a spurious
        // accepted tag, or a surface added only to `ALL` cannot pass by merely
        // agreeing with the aggregate fixtures below.
        const WIRE_SURFACES: [(&str, TypedSupportSurface); 51] = [
            ("direct-zoom", TypedSupportSurface::DirectZoom),
            (
                "digital-zoom-toggle",
                TypedSupportSurface::DigitalZoomToggle,
            ),
            ("digital-zoom-range", TypedSupportSurface::DigitalZoomRange),
            ("iris-control", TypedSupportSurface::IrisControl),
            ("one-push-focus", TypedSupportSurface::OnePushFocus),
            (
                "ptz-optics-snap-focus",
                TypedSupportSurface::PtzOpticsSnapFocus,
            ),
            ("focus-lock", TypedSupportSurface::FocusLock),
            ("push-auto-focus", TypedSupportSurface::PushAutoFocus),
            ("focus-zone", TypedSupportSurface::FocusZone),
            (
                "auto-focus-sensitivity",
                TypedSupportSurface::AutoFocusSensitivity,
            ),
            (
                "focus-near-limit-inquiry",
                TypedSupportSurface::FocusNearLimitInquiry,
            ),
            (
                "backlight-compensation",
                TypedSupportSurface::BacklightCompensation,
            ),
            ("wide-dynamic-range", TypedSupportSurface::WideDynamicRange),
            (
                "exposure-compensation",
                TypedSupportSurface::ExposureCompensation,
            ),
            ("brightness-control", TypedSupportSurface::BrightnessControl),
            (
                "one-push-white-balance",
                TypedSupportSurface::OnePushWhiteBalance,
            ),
            (
                "auto-tracking-white-balance",
                TypedSupportSurface::AutoTrackingWhiteBalance,
            ),
            (
                "auto-white-balance-sensitivity",
                TypedSupportSurface::AutoWhiteBalanceSensitivity,
            ),
            ("color-temperature", TypedSupportSurface::ColorTemperature),
            ("rgb-gain", TypedSupportSurface::RgbGain),
            ("rgb-tuning", TypedSupportSurface::RgbTuning),
            ("image-flip", TypedSupportSurface::ImageFlip),
            ("image-mirror", TypedSupportSurface::ImageMirror),
            (
                "combined-image-flip",
                TypedSupportSurface::CombinedImageFlip,
            ),
            ("contrast-control", TypedSupportSurface::ContrastControl),
            ("sharpness-control", TypedSupportSurface::SharpnessControl),
            ("saturation-control", TypedSupportSurface::SaturationControl),
            ("hue-control", TypedSupportSurface::HueControl),
            ("luminance-control", TypedSupportSurface::LuminanceControl),
            ("gamma-control", TypedSupportSurface::GammaControl),
            ("noise-reduction2-d", TypedSupportSurface::NoiseReduction2D),
            ("noise-reduction3-d", TypedSupportSurface::NoiseReduction3D),
            ("picture-effect", TypedSupportSurface::PictureEffect),
            ("tally", TypedSupportSurface::Tally),
            ("direct-menu", TypedSupportSurface::DirectMenu),
            ("nd-filter", TypedSupportSurface::NdFilter),
            ("variable-speed", TypedSupportSurface::VariableSpeed),
            ("motion-sync", TypedSupportSurface::MotionSync),
            ("focus-zone-inquiry", TypedSupportSurface::FocusZoneInquiry),
            ("usb-audio", TypedSupportSurface::UsbAudio),
            (
                "ptz-optics-anti-flicker",
                TypedSupportSurface::PtzOpticsAntiFlicker,
            ),
            (
                "ptz-optics-settings-save",
                TypedSupportSurface::PtzOpticsSettingsSave,
            ),
            (
                "ptz-optics-preset-recall-speed",
                TypedSupportSurface::PtzOpticsPresetRecallSpeed,
            ),
            ("sony-spotlight", TypedSupportSurface::SonySpotlight),
            (
                "sony-auto-slow-shutter",
                TypedSupportSurface::SonyAutoSlowShutter,
            ),
            (
                "ptz-optics-multicast-streaming",
                TypedSupportSurface::PtzOpticsMulticastStreaming,
            ),
            (
                "ptz-optics-ndi-quality",
                TypedSupportSurface::PtzOpticsNdiQuality,
            ),
            ("exposure-mode", TypedSupportSurface::ExposureMode),
            (
                "iris-control-inquiry",
                TypedSupportSurface::IrisControlInquiry,
            ),
            (
                "noise-reduction2-d-control",
                TypedSupportSurface::NoiseReduction2DControl,
            ),
            (
                "noise-reduction3-d-control",
                TypedSupportSurface::NoiseReduction3DControl,
            ),
        ];

        // These wire fixtures deliberately do not derive from the surface list
        // or serde's rename rules. They preserve the 47 surviving historical
        // tags and their canonical order; the unsupported aggregate
        // noise-reduction was intentionally not retained in the 2.0 surface.
        const LEGACY_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality"]"#;
        const PREVIOUS_CURRENT_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality","exposure-mode"]"#;
        const CURRENT_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality","exposure-mode","iris-control-inquiry"]"#;
        const CONTROL_JSON: &str = r#"["noise-reduction2-d-control","noise-reduction3-d-control"]"#;

        assert_eq!(TypedSupportSurface::ALL.len(), 51);
        assert_eq!(WIRE_SURFACES.len(), 51);
        for (wire_name, surface) in WIRE_SURFACES {
            let singleton_json = serde_json::to_string(&[wire_name])?;
            let decoded: TypedSupportSet = serde_json::from_str(&singleton_json)?;
            let expected = TypedSupportSet::from_surface(surface);

            assert_eq!(decoded, expected, "{wire_name} must decode to {surface:?}");
            assert_eq!(decoded.iter().count(), 1, "{wire_name} must be a singleton");
            assert_eq!(
                serde_json::to_string(&decoded)?,
                singleton_json,
                "{wire_name} must serialize with its exact pinned spelling"
            );
        }

        assert_eq!(
            PREVIOUS_CURRENT_JSON,
            format!(
                r#"{},"exposure-mode"]"#,
                &LEGACY_JSON[..LEGACY_JSON.len() - 1]
            )
        );
        assert_eq!(
            CURRENT_JSON,
            format!(
                r#"{},"iris-control-inquiry"]"#,
                &PREVIOUS_CURRENT_JSON[..PREVIOUS_CURRENT_JSON.len() - 1]
            )
        );

        let legacy: TypedSupportSet = serde_json::from_str(LEGACY_JSON)?;
        assert_eq!(legacy.iter().count(), 47);
        assert!(!legacy.contains(TypedSupportSurface::ExposureMode));
        assert!(!legacy.contains(TypedSupportSurface::IrisControlInquiry));
        assert!(!legacy.contains(TypedSupportSurface::NoiseReduction2DControl));
        assert!(!legacy.contains(TypedSupportSurface::NoiseReduction3DControl));
        assert_eq!(
            legacy,
            TypedSupportSet::from_surfaces(
                &WIRE_SURFACES[..WIRE_SURFACES.len() - 4]
                    .iter()
                    .map(|(_, surface)| *surface)
                    .collect::<Vec<_>>(),
            )
        );
        assert_eq!(serde_json::to_string(&legacy)?, LEGACY_JSON);

        let previous_current: TypedSupportSet = serde_json::from_str(PREVIOUS_CURRENT_JSON)?;
        assert_eq!(previous_current.iter().count(), 48);
        assert!(previous_current.contains(TypedSupportSurface::ExposureMode));
        assert!(!previous_current.contains(TypedSupportSurface::IrisControlInquiry));
        assert!(!previous_current.contains(TypedSupportSurface::NoiseReduction2DControl));
        assert!(!previous_current.contains(TypedSupportSurface::NoiseReduction3DControl));
        assert_eq!(
            previous_current,
            TypedSupportSet::from_surfaces(
                &WIRE_SURFACES[..WIRE_SURFACES.len() - 3]
                    .iter()
                    .map(|(_, surface)| *surface)
                    .collect::<Vec<_>>(),
            )
        );
        assert_eq!(
            serde_json::to_string(&previous_current)?,
            PREVIOUS_CURRENT_JSON
        );

        let current: TypedSupportSet = serde_json::from_str(CURRENT_JSON)?;
        assert_eq!(current.iter().count(), 49);
        assert!(current.contains(TypedSupportSurface::ExposureMode));
        assert!(current.contains(TypedSupportSurface::IrisControlInquiry));
        assert!(!current.contains(TypedSupportSurface::NoiseReduction2DControl));
        assert!(!current.contains(TypedSupportSurface::NoiseReduction3DControl));
        assert_eq!(
            current,
            TypedSupportSet::from_surfaces(
                &WIRE_SURFACES[..WIRE_SURFACES.len() - 2]
                    .iter()
                    .map(|(_, surface)| *surface)
                    .collect::<Vec<_>>(),
            )
        );
        assert_eq!(serde_json::to_string(&current)?, CURRENT_JSON);
        assert_eq!(
            current,
            previous_current.union(TypedSupportSet::from_surface(
                TypedSupportSurface::IrisControlInquiry
            ))
        );

        let controls: TypedSupportSet = serde_json::from_str(CONTROL_JSON)?;
        assert!(controls.contains(TypedSupportSurface::NoiseReduction2DControl));
        assert!(controls.contains(TypedSupportSurface::NoiseReduction3DControl));
        assert_eq!(serde_json::to_string(&controls)?, CONTROL_JSON);

        let final_current = current.union(controls);
        assert_eq!(final_current.iter().count(), 51);
        assert_eq!(
            final_current,
            TypedSupportSet::from_surfaces(
                &WIRE_SURFACES
                    .iter()
                    .map(|(_, surface)| *surface)
                    .collect::<Vec<_>>(),
            )
        );
        assert_eq!(
            serde_json::to_string(&final_current)?,
            format!(
                r#"{},"noise-reduction2-d-control","noise-reduction3-d-control"]"#,
                &CURRENT_JSON[..CURRENT_JSON.len() - 1]
            )
        );

        assert!(serde_json::from_str::<TypedSupportSet>(r#"["noise-reduction"]"#).is_err());
        Ok(())
    }
}
