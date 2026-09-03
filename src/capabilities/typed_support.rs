//! Runtime representation of profile-backed typed API support.
//!
//! Metadata traits describe runtime discovery facts. `TypedSupportSet` describes
//! the optional typed control, accessor, and inquiry surfaces a profile is
//! permitted to expose.

use std::fmt;

macro_rules! define_typed_support_marker {
    (
        DirectMenu,
        $marker:ident,
        $marker_doc:literal,
        $diagnostic:literal
    ) => {};
    (
        $surface:ident,
        $marker:ident,
        $marker_doc:literal,
        $diagnostic:literal
    ) => {
        #[doc = $marker_doc]
        #[diagnostic::on_unimplemented(
            message = $diagnostic,
            label = "profile does not implement the required typed-support marker",
            note = "see the generated marker tables in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support"
        )]
        pub trait $marker {}
    };
}

macro_rules! define_typed_support_surfaces {
    (
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    bit: $bit:literal,
                    wire: $wire:literal,
                    area: $area:literal,
                    api: $api:literal,
                    surface_doc: $surface_doc:literal,
                    marker_doc: $marker_doc:literal,
                    diagnostic: $diagnostic:literal,
                },
            )*
        ]
    ) => {
        const TYPED_SUPPORT_SURFACE_COUNT: usize = [$($wire),*].len();

        /// Optional typed API surface backed by a profile support marker.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[non_exhaustive]
        pub enum TypedSupportSurface {
            $(
                #[doc = $surface_doc]
                #[cfg_attr(feature = "serde", serde(rename = $wire))]
                $surface,
            )*
        }

        impl TypedSupportSurface {
            /// All known typed support surfaces in their canonical serde order.
            pub const ALL: [Self; TYPED_SUPPORT_SURFACE_COUNT] = [
                $(Self::$surface,)*
            ];

            const fn bit(self) -> u64 {
                1 << match self {
                    $(Self::$surface => $bit,)*
                }
            }

            /// Returns the marker-trait spelling paired with this surface.
            #[cfg(test)]
            pub(crate) const fn marker_trait_name(self) -> &'static str {
                match self {
                    $(Self::$surface => stringify!($marker),)*
                }
            }

            /// Returns the contributor-guide area for this surface.
            #[cfg(test)]
            pub(crate) const fn documentation_area(self) -> &'static str {
                match self {
                    $(Self::$surface => $area,)*
                }
            }

            /// Returns the contributor-guide API description for this surface.
            #[cfg(test)]
            pub(crate) const fn documentation_api(self) -> &'static str {
                match self {
                    $(Self::$surface => $api,)*
                }
            }

            /// Returns the stable serde spelling for this surface.
            #[cfg(test)]
            pub(crate) const fn wire_name(self) -> &'static str {
                match self {
                    $(Self::$surface => $wire,)*
                }
            }
        }

        $(define_typed_support_marker!($surface, $marker, $marker_doc, $diagnostic);)*
    };
}

super::typed_support_registry::typed_support_registry!(define_typed_support_surfaces);

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
    fn generated_private_bits_are_unique_and_dense() {
        for (index, surface) in TypedSupportSurface::ALL.into_iter().enumerate() {
            assert_eq!(
                TypedSupportSet::from_surface(surface).0,
                1_u64 << index,
                "{surface:?} must use the generated bit matching its canonical position"
            );
        }
    }

    #[test]
    fn generated_wire_names_are_unique_and_nonempty() {
        let mut names = std::collections::BTreeSet::new();
        for surface in TypedSupportSurface::ALL {
            assert!(!surface.wire_name().is_empty());
            assert!(
                names.insert(surface.wire_name()),
                "duplicate typed-support wire name for {surface:?}"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_the_historical_wire_contract_and_appends_new_surfaces(
    ) -> Result<(), serde_json::Error> {
        // This table is intentionally independent of `TypedSupportSurface::ALL`
        // and serde's derive-generated spelling.  It pins each public wire tag
        // to its exact semantic surface, so a swapped decoder, a spurious
        // accepted tag, or a surface added only to `ALL` cannot pass by merely
        // agreeing with the aggregate fixtures below.
        const WIRE_SURFACES: [(&str, TypedSupportSurface); 55] = [
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
            ("image-freeze", TypedSupportSurface::ImageFreeze),
            ("defog-level", TypedSupportSurface::DefogLevel),
            ("tally-brightness", TypedSupportSurface::TallyBrightness),
            ("ptz-optics-tally", TypedSupportSurface::PtzOpticsTally),
        ];

        // These wire fixtures deliberately do not derive from the surface list
        // or serde's rename rules. They preserve the 47 surviving historical
        // tags and their canonical order; the unsupported aggregate
        // noise-reduction was intentionally not retained in the 2.0 surface.
        const LEGACY_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality"]"#;
        const PREVIOUS_CURRENT_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality","exposure-mode"]"#;
        const CURRENT_JSON: &str = r#"["direct-zoom","digital-zoom-toggle","digital-zoom-range","iris-control","one-push-focus","ptz-optics-snap-focus","focus-lock","push-auto-focus","focus-zone","auto-focus-sensitivity","focus-near-limit-inquiry","backlight-compensation","wide-dynamic-range","exposure-compensation","brightness-control","one-push-white-balance","auto-tracking-white-balance","auto-white-balance-sensitivity","color-temperature","rgb-gain","rgb-tuning","image-flip","image-mirror","combined-image-flip","contrast-control","sharpness-control","saturation-control","hue-control","luminance-control","gamma-control","noise-reduction2-d","noise-reduction3-d","picture-effect","tally","direct-menu","nd-filter","variable-speed","motion-sync","focus-zone-inquiry","usb-audio","ptz-optics-anti-flicker","ptz-optics-settings-save","ptz-optics-preset-recall-speed","sony-spotlight","sony-auto-slow-shutter","ptz-optics-multicast-streaming","ptz-optics-ndi-quality","exposure-mode","iris-control-inquiry"]"#;
        const CONTROL_JSON: &str = r#"["noise-reduction2-d-control","noise-reduction3-d-control"]"#;
        const ROW_SCOPED_JSON: &str =
            r#"["image-freeze","defog-level","tally-brightness","ptz-optics-tally"]"#;

        assert_eq!(TypedSupportSurface::ALL.len(), 55);
        assert_eq!(WIRE_SURFACES.len(), 55);
        for (wire_name, surface) in WIRE_SURFACES {
            assert_eq!(surface.wire_name(), wire_name);
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
                &WIRE_SURFACES[..47]
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
                &WIRE_SURFACES[..48]
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
                &WIRE_SURFACES[..49]
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

        let row_scoped: TypedSupportSet = serde_json::from_str(ROW_SCOPED_JSON)?;
        assert!(row_scoped.contains(TypedSupportSurface::ImageFreeze));
        assert!(row_scoped.contains(TypedSupportSurface::DefogLevel));
        assert!(row_scoped.contains(TypedSupportSurface::TallyBrightness));
        assert!(row_scoped.contains(TypedSupportSurface::PtzOpticsTally));
        assert_eq!(serde_json::to_string(&row_scoped)?, ROW_SCOPED_JSON);

        let final_current = current.union(controls).union(row_scoped);
        assert_eq!(final_current.iter().count(), 55);
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
                r#"{},"noise-reduction2-d-control","noise-reduction3-d-control","image-freeze","defog-level","tally-brightness","ptz-optics-tally"]"#,
                &CURRENT_JSON[..CURRENT_JSON.len() - 1]
            )
        );

        assert!(serde_json::from_str::<TypedSupportSet>(r#"["noise-reduction"]"#).is_err());
        Ok(())
    }
}
