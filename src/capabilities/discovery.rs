//! Structured capabilities response for runtime feature discovery.
//!
//! This module provides a unified `Capabilities` struct that exposes
//! camera capabilities at runtime, complementing the compile-time
//! trait-based capability system.

use std::borrow::Cow;

use super::TypedSupportSurface as S;
use super::{profile_metadata::InquirySupport, TypedSupportSet, TypedSupportSurface};
use crate::WhiteBalanceMode;

/// Keep the explicit-rule set and its executable rules in one declaration.
/// The const gate below uses the same patterns as the runtime match.
macro_rules! surface_metadata_rules {
    ($this:ident, $surface:ident; locals { $($locals:tt)* }
        rules { $($pattern:pat => $value:expr),+ $(,)? }) => {
        /// Relates a typed permission to the facts required to prepare it.
        pub(crate) fn surface_metadata(&$this, $surface: TypedSupportSurface) -> SurfaceMetadata {
            if $this.inquiry_support == InquirySupport::None
                && crate::command::surface::is_inquiry_only_surface($surface)
            {
                return SurfaceMetadata::ParentDomain(false);
            }
            $($locals)*
            match $surface {
                $($pattern => $value,)+
                _ => SurfaceMetadata::ParentDomain(
                    crate::command::surface::surface_domain($surface).permits($this),
                ),
            }
        }

        const fn has_explicit_surface_metadata($surface: TypedSupportSurface) -> bool {
            matches!($surface, $($pattern)|+)
        }
    };
}

// A mixed or missing domain must acquire an explicit metadata rule. The rule
// patterns above generate this predicate, so adding an arm cannot be forgotten
// in a second manually maintained exception inventory.
const _: () = {
    let mut index = 0;
    while index < TypedSupportSurface::ALL.len() {
        let surface = TypedSupportSurface::ALL[index];
        if !Capabilities::has_explicit_surface_metadata(surface) {
            assert!(
                !matches!(
                    crate::command::surface::surface_domain(surface),
                    crate::command::surface::SurfaceDomain::Mixed
                ),
                "a mixed or missing parent domain needs an explicit metadata rule",
            );
        }
        index += 1;
    }
};

/// Runtime discovery facts grouped by physical or control domain.
///
/// An absent domain has no dependent facts. Typed permission remains explicit in
/// [`Self::typed_support`]; inspect the selected domain for its numeric inventory.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Capabilities {
    /// Built-in identity, absent for a custom runtime profile.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub profile_id: Option<crate::camera::profiles::ProfileId>,
    /// Camera model name.
    pub model_name: String,
    /// Validated individual default target.
    pub default_camera_id: crate::IndividualCameraId,
    /// Pan/tilt facts, present exactly when the domain is supported.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub pan_tilt: Option<super::PanTiltCaps>,
    /// Optical and digital lens facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub zoom: Option<super::ZoomCaps>,
    /// Focus bounds, orientation and refinements.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub focus: Option<super::FocusCaps>,
    /// Exposure inventories and shared numeric domains.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub exposure: Option<super::ExposureCaps>,
    /// White-balance modes and numeric domains.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub white_balance: Option<super::WhiteBalanceCaps>,
    /// Image-processing adjustment facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub image: Option<super::ImageCaps>,
    /// Preset inventory and exact recall axes.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub presets: Option<super::PresetCaps>,
    /// Power sequencing facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub power: Option<super::PowerCaps>,
    /// Menu domain, present only for profiles with source-backed menu support.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub menu: Option<super::MenuCaps>,
    /// Profile-wide inquiry availability.
    pub inquiry_support: InquirySupport,
    /// Optional typed API permissions; metadata is not fallback permission.
    #[cfg_attr(feature = "schemars", schemars(with = "Vec<TypedSupportSurface>"))]
    pub typed_support: TypedSupportSet,
}

impl Capabilities {
    /// Creates a conservative runtime-only capability inventory.
    ///
    /// No transport, control domain, inquiry, completion, cancellation, or
    /// optional typed surface is granted. Callers explicitly populate the
    /// documented facts, then pass the value to [`crate::profiles::ProfileSpec::builder`](crate::profiles::ProfileSpec::builder).
    ///
    /// # Errors
    ///
    /// Returns an error for an empty model name or invalid camera ID.
    pub fn runtime_baseline(
        model_name: impl Into<String>,
        default_camera_id: u8,
    ) -> Result<Self, crate::Error> {
        let model_name = model_name.into();
        if model_name.trim().is_empty() {
            return Err(crate::Error::InvalidRequest {
                reason: "runtime capability model name must not be empty".into(),
            });
        }
        Ok(Self {
            profile_id: None,
            model_name,
            default_camera_id: crate::IndividualCameraId::try_from(default_camera_id)?,
            pan_tilt: None,
            zoom: None,
            focus: None,
            exposure: None,
            white_balance: None,
            image: None,
            presets: None,
            power: None,
            menu: None,
            inquiry_support: InquirySupport::None,
            typed_support: TypedSupportSet::empty(),
        })
    }

    /// Returns whether the profile grants the requested typed surface.
    #[must_use]
    pub const fn supports(&self, surface: TypedSupportSurface) -> bool {
        self.typed_support.contains(surface)
    }

    surface_metadata_rules! {
        self, surface;
        locals {
            use SurfaceMetadata::{Dedicated, ParentDomain};
            let exposure = self.exposure.as_ref();
            let focus = self.focus.as_ref();
            let white_balance = self.white_balance.as_ref();
            let image = self.image.as_ref();
        }
        rules {
            S::DirectZoom => Dedicated(self.zoom.as_ref().is_some_and(|domain| domain.direct)),
            S::DigitalZoomToggle | S::DigitalZoomRange => Dedicated(
                self.zoom
                    .as_ref()
                    .is_some_and(|domain| domain.digital_max.is_some()),
            ),
            S::ExposureMode => Dedicated(exposure.is_some_and(|domain| !domain.modes.is_empty())),
            S::Iris => Dedicated(exposure.is_some_and(|domain| domain.iris_range.is_some())),
            S::Gain => Dedicated(exposure.is_some_and(|domain| domain.gain_range.is_some())),
            S::BacklightCompensation => {
                Dedicated(exposure.is_some_and(|domain| domain.backlight_compensation))
            },
            S::WideDynamicRange => {
                Dedicated(exposure.is_some_and(|domain| domain.wide_dynamic_range))
            },
            S::ExposureCompensation => {
                Dedicated(exposure.is_some_and(|domain| domain.compensation_range.is_some()))
            },
            S::Brightness => {
                Dedicated(exposure.is_some_and(|domain| domain.brightness_range.is_some()))
            },
            S::OnePushFocus => Dedicated(focus.is_some_and(|domain| domain.one_push)),
            S::FocusZone => Dedicated(focus.is_some_and(|domain| !domain.zones.is_empty())),
            S::FocusZoneInquiry => Dedicated(focus.is_some_and(|domain| domain.zone_inquiry)),
            S::AutoFocusSensitivity => {
                Dedicated(focus.is_some_and(|domain| domain.auto_focus && domain.af_sensitivity))
            },
            S::FocusNearLimit => Dedicated(focus.is_some_and(|domain| domain.near_limit_inquiry)),
            // A one-shot action does not require the standard OnePush mode:
            // the documented FR7 action captures white in Memory A or ATW.
            S::OnePushWhiteBalance => {
                Dedicated(white_balance.is_some_and(|domain| domain.one_push))
            },
            S::ColorTemperature => Dedicated(white_balance.is_some_and(|domain| {
                domain.color_temp_range.is_some()
                    && domain.modes.contains(&WhiteBalanceMode::ColorTemperature)
            })),
            S::RgbGain => Dedicated(white_balance.is_some_and(|domain| domain.rgb_gain.is_some())),
            S::RgbTuning => {
                Dedicated(white_balance.is_some_and(|domain| domain.rgb_tuning.is_some()))
            },
            S::ImageFlip => Dedicated(image.is_some_and(|domain| domain.flip)),
            S::ImageMirror => Dedicated(image.is_some_and(|domain| domain.mirror)),
            S::CombinedImageFlip => {
                ParentDomain(image.is_some_and(|domain| domain.flip && domain.mirror))
            },
            S::Contrast => Dedicated(image.is_some_and(|domain| domain.contrast_range.is_some())),
            S::Sharpness => Dedicated(image.is_some_and(|domain| domain.sharpness_range.is_some())),
            S::Saturation => {
                Dedicated(image.is_some_and(|domain| domain.saturation_range.is_some()))
            },
            S::Hue => Dedicated(image.is_some_and(|domain| domain.hue_range.is_some())),
            S::Luminance => Dedicated(image.is_some_and(|domain| domain.luminance_range.is_some())),
            S::Gamma => Dedicated(image.is_some_and(|domain| domain.gamma_range.is_some())),
            S::PictureEffect => Dedicated(image.is_some_and(|domain| domain.picture_effect)),
            S::PanTiltSpeedMode => Dedicated(
                self.pan_tilt
                    .as_ref()
                    .is_some_and(|domain| domain.variable_speed),
            ),
            S::MotionSync => Dedicated(
                self.pan_tilt
                    .as_ref()
                    .is_some_and(|domain| domain.motion_sync_speed_range.is_some()),
            ),
            S::PresetRecallSpeed => ParentDomain(
                self.presets
                    .as_ref()
                    .is_some_and(|domain| domain.recall_speed_range.is_some()),
            ),
            S::PresetRecallSpeedInquiry => {
                ParentDomain(self.presets.is_some() && self.supports(S::PresetRecallSpeed))
            },
            S::PowerStandby => Dedicated(
                self.power
                    .as_ref()
                    .is_some_and(|domain| domain.standby.is_some()),
            ),
            S::DirectShutter => {
                ParentDomain(exposure.is_some_and(|domain| !domain.shutter_speeds.is_empty()))
            },
            S::NoiseReduction2DMode => {
                ParentDomain(image.is_some() && self.supports(S::NoiseReduction2D))
            },
            S::ColorTemperatureInquiry => {
                ParentDomain(self.surface_metadata(S::ColorTemperature).permits())
            },
            S::TallyBrightness | S::TallyMode => ParentDomain(self.supports(S::Tally)),
        }
    }

    /// Creates the fixed lens's optical magnification scale.
    pub fn zoom_scale(&self) -> Result<super::ZoomScale, crate::Error> {
        let zoom = self
            .zoom
            .as_ref()
            .ok_or(crate::Error::FeatureNotSupported { feature: "zoom" })?;
        let ratio = zoom.optical_ratio.ok_or_else(|| crate::Error::InvalidRequest { reason: format!("{} does not fix its lens's optical zoom ratio; declare the installed lens with `Capabilities::zoom_scale_for_lens(ratio)` or `ProfileSpecBuilder::zoom`", self.model_name).into() })?;
        super::ZoomScale::new(zoom.optical_max, ratio)
    }

    /// Creates the optical magnification scale for a declared installed lens.
    pub fn zoom_scale_for_lens(
        &self,
        optical_ratio: f32,
    ) -> Result<super::ZoomScale, crate::Error> {
        let zoom = self
            .zoom
            .as_ref()
            .ok_or(crate::Error::FeatureNotSupported { feature: "zoom" })?;
        if zoom
            .optical_ratio
            .is_some_and(|fixed| fixed != optical_ratio)
        {
            return Err(crate::Error::InvalidParameter {
                parameter: "optical zoom ratio",
                value: Cow::Owned(optical_ratio.to_string()),
                reason: Cow::Owned(format!("{} fixes a different lens ratio", self.model_name)),
            });
        }
        super::ZoomScale::new(zoom.optical_max, optical_ratio)
    }

    /// Returns the shared shutter code for an exact source exposure time.
    pub fn shutter_speed_for(
        &self,
        exposure: crate::units::ExposureTime,
    ) -> Result<crate::types::ShutterSpeed, crate::Error> {
        let entry = self
            .exposure
            .as_ref()
            .and_then(|domain| {
                domain
                    .shutter_speeds
                    .iter()
                    .find(|speed| speed.exposure == exposure)
            })
            .ok_or_else(|| crate::Error::InvalidParameter {
                parameter: "shutter speed",
                value: Cow::Owned(exposure.to_string()),
                reason: Cow::Owned(format!(
                    "{} has no shutter-table entry for this exposure time",
                    self.model_name
                )),
            })?;
        crate::types::ShutterSpeed::new(entry.value)
    }

    /// Formats the present domains and their main numeric limits.
    pub fn summary(&self) -> String {
        let mut lines = vec![format!("Model: {}", self.model_name)];
        if let Some(domain) = &self.pan_tilt {
            lines.push(format!(
                "Pan Range: {:?}° ({:?} units)",
                domain.pan_range_degrees(),
                domain.pan_range
            ));
            lines.push(format!(
                "Tilt Range: {:?}° ({:?} units)",
                domain.tilt_range_degrees(None),
                domain.tilt_ranges.range(None)
            ));
        }
        if let Some(domain) = &self.zoom {
            let lens = domain
                .optical_ratio
                .map_or_else(|| "lens-dependent".to_owned(), |ratio| format!("{ratio}x"));
            lines.push(format!(
                "Optical Zoom: {lens} (0x{:04X} units)",
                domain.optical_max
            ));
            if let Some(maximum) = domain.digital_max {
                lines.push(format!("Digital Zoom: to 0x{maximum:04X} units"));
            }
        }
        if let Some(domain) = &self.presets {
            lines.push(format!("Presets: 0..={}", domain.highest));
        }
        if self.supports(TypedSupportSurface::NdFilter) {
            lines.push("ND Filter: supported".to_owned());
        }
        lines.join("\n")
    }
}

/// What discovery metadata says about one typed surface
/// ([`Capabilities::surface_metadata`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceMetadata {
    /// The surface has its own discovery fact; `true` when the metadata
    /// advertises it. For a built-in profile the typed-support marker must
    /// agree exactly, unless the row's evidence records why it differs.
    Dedicated(bool),
    /// The surface has no discovery fact of its own and needs only its parent
    /// domain (`true` when present). Its typed-support marker is the only
    /// source-backed fact.
    ParentDomain(bool),
}

impl SurfaceMetadata {
    /// Whether the metadata permits a typed-support marker for the surface.
    pub(crate) const fn permits(self) -> bool {
        match self {
            Self::Dedicated(permits) | Self::ParentDomain(permits) => permits,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {

    use crate::camera::profiles::{
        GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300,
        SonyBrcH900, SonyEviH100, SonyFr7,
    };
    use crate::capabilities::CapabilityDomain;
    use crate::command::exposure_wire::ExposureMode;
    use crate::WhiteBalanceMode;

    use super::*;

    /// Characterized on 9eb89b62 before replacing the handwritten domains.
    /// Hex bitsets follow TypedSupportSurface::ALL: permission first, then
    /// whether the metadata has a dedicated fact. Domain-removal cases are
    /// intentionally unvalidated capability records, as used by validation.
    #[test]
    fn surface_metadata_characterization() {
        use crate::profiles::ProfileId;
        use std::fmt::Write;
        let mut actual = String::new();
        let mut record = |label: &str, caps: &Capabilities| {
            let mut permits = 0_u128;
            let mut dedicated = 0_u128;
            for surface in TypedSupportSurface::ALL {
                let metadata = caps.surface_metadata(surface);
                if metadata.permits() {
                    permits |= 1_u128 << (surface as u8);
                }
                if matches!(metadata, SurfaceMetadata::Dedicated(_)) {
                    dedicated |= 1_u128 << (surface as u8);
                }
            }
            writeln!(actual, "{label} {permits:021x} {dedicated:021x}").expect("write snapshot");
        };
        for id in ProfileId::all() {
            record(
                &format!("{id:?}"),
                id.spec().expect("profile").capabilities(),
            );
        }
        let mut baseline = ProfileId::SonyFr7
            .spec()
            .expect("full domain fixture")
            .capabilities()
            .clone();
        for id in ProfileId::all() {
            let profile = id.spec().expect("profile");
            let caps = profile.capabilities();
            if baseline.pan_tilt.is_none() {
                baseline.pan_tilt = caps.pan_tilt;
            }
            if baseline.zoom.is_none() {
                baseline.zoom = caps.zoom;
            }
            if baseline.focus.is_none() {
                baseline.focus = caps.focus.clone();
            }
            if baseline.exposure.is_none() {
                baseline.exposure = caps.exposure.clone();
            }
            if baseline.white_balance.is_none() {
                baseline.white_balance = caps.white_balance.clone();
            }
            if baseline.image.is_none() {
                baseline.image = caps.image;
            }
            if baseline.presets.is_none() {
                baseline.presets = caps.presets;
            }
            if baseline.power.is_none() {
                baseline.power = caps.power;
            }
            if baseline.menu.is_none() {
                baseline.menu = caps.menu;
            }
        }
        record("all_domains", &baseline);
        for absent in [
            "pan_tilt",
            "zoom",
            "focus",
            "exposure",
            "white_balance",
            "image",
            "presets",
            "power",
            "menu",
        ] {
            let mut caps = baseline.clone();
            match absent {
                "pan_tilt" => caps.pan_tilt = None,
                "zoom" => caps.zoom = None,
                "focus" => caps.focus = None,
                "exposure" => caps.exposure = None,
                "white_balance" => caps.white_balance = None,
                "image" => caps.image = None,
                "presets" => caps.presets = None,
                "power" => caps.power = None,
                "menu" => caps.menu = None,
                _ => {}
            }
            record(&format!("without_{absent}"), &caps);
        }
        assert_surface_metadata_rows(
            &actual,
            include_str!("../../tests/fixtures/issue_1000_surface_metadata.txt"),
        );
    }

    fn assert_surface_metadata_rows(actual: &str, expected: &str) {
        assert_eq!(
            actual.lines().collect::<Vec<_>>(),
            expected.lines().collect::<Vec<_>>()
        );
    }

    #[test]
    fn surface_metadata_rows_accept_crlf_and_reject_changed_bitmasks() {
        let fixture = include_str!("../../tests/fixtures/issue_1000_surface_metadata.txt");
        let lf = fixture.replace("\r\n", "\n");
        let crlf = lf.replace('\n', "\r\n");
        assert_surface_metadata_rows(&lf, &crlf);
        assert_surface_metadata_rows(&crlf, &lf);

        let mut changed = crlf;
        let first_bitmask = changed.find(' ').expect("profile and bitmask separator") + 1;
        let replacement = if changed.as_bytes()[first_bitmask] == b'0' {
            "1"
        } else {
            "0"
        };
        changed.replace_range(first_bitmask..first_bitmask + 1, replacement);
        assert!(std::panic::catch_unwind(|| assert_surface_metadata_rows(&lf, &changed)).is_err());
    }

    fn profile_capabilities<P: crate::CompileTimeProfile>() -> Capabilities {
        P::spec()
            .expect("valid built-in profile")
            .capabilities()
            .clone()
    }

    #[cfg(feature = "serde")]
    #[test]
    fn power_query_fact_has_pinned_names_and_is_required_on_load() {
        use crate::capabilities::PowerStatusQuery;
        for (id, expected) in [
            (crate::profiles::ProfileId::PtzOpticsG2, "standard"),
            (crate::profiles::ProfileId::SonyBrcH900, "extended"),
        ] {
            let profile = id.spec().expect("profile");
            let mut document = serde_json::to_value(&profile).expect("export profile");
            assert_eq!(document["capabilities"]["power"]["status_query"], expected);
            document["capabilities"]["profile_id"] = serde_json::Value::Null;
            let loaded: crate::ProfileSpec =
                serde_json::from_value(document.clone()).expect("custom clone");
            assert_eq!(loaded.capabilities().power, profile.capabilities().power);
            let expected_body: &[u8] = if expected == "extended" {
                &[0x09, 0x7E, 0x04, 0x00]
            } else {
                &[0x09, 0x04, 0x00]
            };
            assert_eq!(
                crate::command::inquiry_structs::body_for_profile(
                    crate::command::InquiryKind::Power,
                    &crate::command::bytes::constants::power::STATE_INQUIRY,
                    &loaded,
                ),
                expected_body
            );
            document["capabilities"]["power"]
                .as_object_mut()
                .expect("power object")
                .remove("status_query");
            let error = serde_json::from_value::<crate::ProfileSpec>(document)
                .expect_err("dialect is explicit");
            assert!(error.to_string().contains("status_query"), "{error}");
        }
        assert_eq!(
            serde_json::to_value(PowerStatusQuery::Standard).expect("standard"),
            "standard"
        );
        assert_eq!(
            serde_json::to_value(PowerStatusQuery::Extended).expect("extended"),
            "extended"
        );
        assert!(serde_json::from_str::<PowerStatusQuery>("\"model_specific\"").is_err());
    }

    #[test]
    fn shared_gain_metadata_requires_its_value_range() {
        let mut caps = profile_capabilities::<PtzOpticsG2>();
        assert_eq!(
            caps.surface_metadata(TypedSupportSurface::Gain),
            SurfaceMetadata::Dedicated(true)
        );
        caps.exposure.as_mut().expect("exposure").gain_range = None;
        assert_eq!(
            caps.surface_metadata(TypedSupportSurface::Gain),
            SurfaceMetadata::Dedicated(false)
        );
        assert!(caps
            .surface_metadata(TypedSupportSurface::GainLimit)
            .permits());
    }

    #[test]
    fn test_capabilities_from_ptzoptics_g2() {
        let caps = profile_capabilities::<PtzOpticsG2>();

        assert_eq!(
            caps.profile_id,
            Some(crate::profiles::ProfileId::PtzOpticsG2)
        );
        assert_eq!(caps.model_name, "PtzOptics G2");
        assert_eq!(caps.default_camera_id.value(), 1);
        let transports = <PtzOpticsG2 as crate::CompileTimeProfile>::spec()
            .expect("profile")
            .transports();
        assert_eq!(transports.tcp_port(), Some(5678));
        assert_eq!(transports.udp_port(), Some(1259));

        assert!(caps.pan_tilt.is_some());
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .pan_speed
                .as_inclusive(),
            1..=24
        );
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .tilt_speed
                .as_inclusive(),
            1..=20
        );

        assert!(caps.zoom.is_some());
        assert!(caps
            .zoom
            .as_ref()
            .and_then(|domain| domain.digital_range())
            .map(|range| range.as_inclusive())
            .is_none());
        assert_eq!(
            caps.zoom
                .as_ref()
                .expect("zoom")
                .optical_range()
                .as_inclusive(),
            0x0000..=0x4000
        );
        assert_eq!(
            caps.zoom
                .as_ref()
                .and_then(|domain| domain.digital_range())
                .map(|range| range.as_inclusive()),
            None
        );

        assert!(caps.focus.as_ref().is_some_and(|domain| domain.auto_focus));
        assert!(!caps.focus.as_ref().is_some_and(|domain| domain.one_push));
        assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.zone_inquiry));
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.af_sensitivity));
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.near_limit_inquiry));
        assert!(caps
            .exposure
            .as_ref()
            .and_then(|domain| domain.iris_range.clone())
            .is_some());
        assert!(caps
            .exposure
            .as_ref()
            .is_some_and(|domain| domain.modes.contains(&ExposureMode::Iris)));
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.iris_range.clone()),
            Some(CapabilityDomain::<u16>::new(0, 12))
        );
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.compensation_range)
                .map(|range| range.as_inclusive()),
            Some(-7..=7)
        );
        assert_eq!(
            caps.white_balance
                .as_ref()
                .and_then(|domain| domain.rgb_gain)
                .map(|range| range.red.as_inclusive()),
            Some(0..=255)
        );
        assert_eq!(
            caps.white_balance
                .as_ref()
                .and_then(|domain| domain.rgb_gain)
                .map(|range| range.blue.as_inclusive()),
            Some(0..=255)
        );

        assert_eq!(caps.presets.as_ref().expect("presets").highest, 127);
        assert!(!caps.presets.as_ref().is_some_and(|domain| domain.tour));
        assert!(!caps.supports(TypedSupportSurface::NdFilter));
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(!caps
            .pan_tilt
            .as_ref()
            .is_some_and(|domain| domain.variable_speed));
        assert!(caps.supports(TypedSupportSurface::UsbAudio));
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            Some(CapabilityDomain::<u8>::new(0, 17))
        );
        assert!(caps.image.is_some());
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.contrast_range)
                .map(|range| range.as_inclusive()),
            Some(0..=14)
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.sharpness_range)
                .map(|range| range.as_inclusive()),
            Some(0..=15)
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.luminance_range)
                .map(|range| range.as_inclusive()),
            Some(0..=14)
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.gamma_range)
                .map(|range| range.as_inclusive()),
            Some(0..=4)
        );
        assert!(caps
            .image
            .as_ref()
            .is_some_and(|domain| domain.picture_effect));

        assert!(
            caps.pan_tilt.is_some()
                && caps.zoom.is_some()
                && caps.focus.is_some()
                && caps.exposure.is_some()
                && caps.white_balance.is_some()
                && caps.image.is_some()
                && caps.presets.is_some()
                && caps.power.is_some()
                && caps.menu.is_some()
        );
    }

    #[test]
    fn test_capabilities_from_sony_fr7() {
        let caps = profile_capabilities::<SonyFr7>();

        assert_eq!(caps.model_name, "Sony FR7");
        let transports = <SonyFr7 as crate::CompileTimeProfile>::spec()
            .expect("profile")
            .transports();
        assert_eq!(transports.tcp_port(), None);
        assert_eq!(transports.udp_port(), Some(52381));

        assert_eq!(caps.presets.as_ref().expect("presets").highest, 99);
        assert!(!caps.presets.as_ref().is_some_and(|domain| domain.tour));
        assert!(!caps.focus.as_ref().is_some_and(|domain| domain.one_push));
        assert!(caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.zone_inquiry));
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.af_sensitivity));
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.near_limit_inquiry));
        assert_eq!(
            caps.white_balance
                .as_ref()
                .and_then(|domain| domain.rgb_gain)
                .map(|range| range.red.as_inclusive()),
            None
        );
        assert_eq!(
            caps.white_balance
                .as_ref()
                .and_then(|domain| domain.rgb_gain)
                .map(|range| range.blue.as_inclusive()),
            None
        );
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.compensation_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.white_balance
                .as_ref()
                .and_then(|domain| domain.color_temp_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(caps.supports(TypedSupportSurface::NdFilter));
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(caps
            .pan_tilt
            .as_ref()
            .is_some_and(|domain| domain.variable_speed));
        assert!(!caps.supports(TypedSupportSurface::UsbAudio));
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            None
        );
        assert!(caps.image.is_some());
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.contrast_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.sharpness_range)
                .map(|range| range.as_inclusive()),
            Some(0..=14)
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.gamma_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(!caps
            .image
            .as_ref()
            .is_some_and(|domain| domain.picture_effect));

        // FR7 shares the one-shot capture concept even though it has no
        // standard OnePush mode variant or shared Kelvin-control family.
        assert!(caps.supports(TypedSupportSurface::OnePushWhiteBalance));
        assert!(caps
            .white_balance
            .as_ref()
            .is_some_and(|domain| domain.one_push));
    }

    #[test]
    fn test_typed_support_discovery_for_built_in_profiles() {
        let g2 = profile_capabilities::<PtzOpticsG2>();
        assert_eq!(
            g2.typed_support,
            <PtzOpticsG2 as crate::CompileTimeProfile>::TYPED_SUPPORT
        );
        assert!(g2.supports(TypedSupportSurface::DirectZoom));
        assert!(g2.supports(TypedSupportSurface::FocusZone));
        assert!(g2.supports(TypedSupportSurface::FocusZoneInquiry));
        assert!(g2.supports(TypedSupportSurface::UsbAudio));
        assert!(g2.supports(TypedSupportSurface::AntiFlicker));
        assert!(g2.supports(TypedSupportSurface::SettingsSave));
        assert!(g2.supports(TypedSupportSurface::PresetRecallSpeed));
        assert!(g2.supports(TypedSupportSurface::MulticastStreaming));
        assert!(g2.supports(TypedSupportSurface::NdiQuality));
        assert!(g2.supports(TypedSupportSurface::ExposureMode));
        assert!(!g2.supports(TypedSupportSurface::DigitalZoomToggle));
        assert!(!g2.supports(TypedSupportSurface::DigitalZoomRange));

        let fr7 = profile_capabilities::<SonyFr7>();
        assert_eq!(
            fr7.typed_support,
            <SonyFr7 as crate::CompileTimeProfile>::TYPED_SUPPORT
        );
        assert!(fr7.supports(TypedSupportSurface::DirectZoom));
        assert!(!fr7.supports(TypedSupportSurface::DigitalZoomToggle));
        assert!(!fr7.supports(TypedSupportSurface::DigitalZoomRange));
        assert!(!fr7.supports(TypedSupportSurface::FocusZone));
        assert!(!fr7.supports(TypedSupportSurface::FocusZoneInquiry));
        assert!(!fr7.supports(TypedSupportSurface::UsbAudio));
        assert!(!fr7.supports(TypedSupportSurface::Brightness));
        assert!(!fr7.supports(TypedSupportSurface::PictureEffect));
        assert!(!fr7.supports(TypedSupportSurface::AutoFocusSensitivity));
        assert!(fr7.supports(TypedSupportSurface::Spotlight));
        assert!(!fr7.supports(TypedSupportSurface::AutoSlowShutter));
        assert!(!fr7.supports(TypedSupportSurface::ExposureMode));
    }

    #[test]
    fn iris_control_status_inquiry_is_not_inferred_from_other_iris_facts() {
        let g3 = profile_capabilities::<PtzOpticsG3>();
        assert!(g3
            .exposure
            .as_ref()
            .and_then(|domain| domain.iris_range.clone())
            .is_some());
        assert!(g3.supports(TypedSupportSurface::Iris));
    }

    #[test]
    fn shared_exposure_mode_support_exactly_matches_discovery_inventory() {
        for caps in [
            profile_capabilities::<PtzOpticsG2>(),
            profile_capabilities::<PtzOpticsG3>(),
            profile_capabilities::<PtzOptics30x>(),
            profile_capabilities::<SonyBrcH900>(),
            profile_capabilities::<SonyEviH100>(),
            profile_capabilities::<SonyBrc300>(),
            profile_capabilities::<NearusBrc300>(),
            profile_capabilities::<GenericVisca>(),
        ] {
            assert!(!caps.exposure.as_ref().expect("exposure").modes.is_empty());
            assert!(caps.supports(TypedSupportSurface::ExposureMode));
        }

        let fr7 = profile_capabilities::<SonyFr7>();
        assert!(fr7.exposure.as_ref().expect("exposure").modes.is_empty());
        assert!(!fr7.supports(TypedSupportSurface::ExposureMode));
    }

    #[test]
    fn sony_vendor_exposure_typed_support_matches_the_model_command_lists() {
        let expected = [
            ("Sony FR7", profile_capabilities::<SonyFr7>(), true, false),
            (
                "Sony BRC-H900",
                profile_capabilities::<SonyBrcH900>(),
                false,
                false,
            ),
            (
                "Sony EVI-H100",
                profile_capabilities::<SonyEviH100>(),
                false,
                true,
            ),
            (
                "Sony BRC-300",
                profile_capabilities::<SonyBrc300>(),
                false,
                true,
            ),
            (
                "Nearus BRC-300",
                profile_capabilities::<NearusBrc300>(),
                false,
                true,
            ),
        ];

        for (model, caps, spotlight, auto_slow_shutter) in expected {
            assert_eq!(
                caps.supports(TypedSupportSurface::Spotlight),
                spotlight,
                "{model} spotlight support"
            );
            assert_eq!(
                caps.supports(TypedSupportSurface::AutoSlowShutter),
                auto_slow_shutter,
                "{model} auto slow-shutter support"
            );
        }
    }

    #[test]
    fn test_metadata_enabled_profile_reports_typed_support_independently() {
        let mut caps = profile_capabilities::<GenericVisca>();
        caps.typed_support = TypedSupportSet::EMPTY;
        caps.zoom.as_mut().expect("zoom").direct = true;
        caps.zoom.as_mut().expect("zoom").optical_max = 0x4000;
        caps.focus.as_mut().expect("focus").one_push = true;
        caps.zoom.as_mut().expect("zoom").digital_max = Some(0x7000);
        caps.focus.as_mut().expect("focus").zone_inquiry = false;
        caps.focus.as_mut().expect("focus").af_sensitivity = true;
        caps.focus.as_mut().expect("focus").zones = vec![crate::command::FocusZone::Center];

        assert!(caps.zoom.as_ref().is_some_and(|domain| domain.direct));
        assert!(caps
            .zoom
            .as_ref()
            .and_then(|domain| domain.digital_range())
            .map(|range| range.as_inclusive())
            .is_some());
        assert!(caps.focus.as_ref().is_some_and(|domain| domain.one_push));
        assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(!caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.zone_inquiry));
        assert!(caps
            .focus
            .as_ref()
            .is_some_and(|domain| domain.af_sensitivity));
        assert!(!caps.supports(TypedSupportSurface::UsbAudio));

        assert!(caps.typed_support.is_empty());
        assert!(!caps.supports(TypedSupportSurface::DirectZoom));
        assert!(!caps.supports(TypedSupportSurface::DigitalZoomToggle));
        assert!(!caps.supports(TypedSupportSurface::DigitalZoomRange));
        assert!(!caps.supports(TypedSupportSurface::OnePushFocus));
        assert!(!caps.supports(TypedSupportSurface::FocusZone));
        assert!(!caps.supports(TypedSupportSurface::FocusZoneInquiry));
        assert!(!caps.supports(TypedSupportSurface::UsbAudio));
        assert!(!caps.supports(TypedSupportSurface::AutoFocusSensitivity));
        assert!(!caps.supports(TypedSupportSurface::AntiFlicker));
        assert!(!caps.supports(TypedSupportSurface::SettingsSave));
        assert!(!caps.supports(TypedSupportSurface::PresetRecallSpeed));
        assert!(!caps.supports(TypedSupportSurface::Spotlight));
        assert!(!caps.supports(TypedSupportSurface::AutoSlowShutter));
        assert!(!caps.supports(TypedSupportSurface::MulticastStreaming));
        assert!(!caps.supports(TypedSupportSurface::NdiQuality));
    }

    #[test]
    fn source_backed_focus_zone_inquiry_and_usb_audio_boundaries_match_profiles() {
        let g2 = profile_capabilities::<PtzOpticsG2>();
        let g3 = profile_capabilities::<PtzOpticsG3>();
        let thirty_x = profile_capabilities::<PtzOptics30x>();

        for caps in [&g2, &g3, &thirty_x] {
            assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
            assert!(caps.supports(TypedSupportSurface::FocusZone));
            assert!(caps
                .image
                .as_ref()
                .is_some_and(|domain| domain.picture_effect));
            assert!(caps.supports(TypedSupportSurface::PictureEffect));
        }

        for caps in [&g2, &thirty_x] {
            assert!(caps
                .focus
                .as_ref()
                .is_some_and(|domain| domain.zone_inquiry));
            assert!(caps.supports(TypedSupportSurface::FocusZoneInquiry));
            assert!(caps.supports(TypedSupportSurface::UsbAudio));
            assert!(caps.supports(TypedSupportSurface::UsbAudio));
        }

        assert!(!g3.focus.as_ref().is_some_and(|domain| domain.zone_inquiry));
        assert!(!g3.supports(TypedSupportSurface::FocusZoneInquiry));
        assert!(!g3.supports(TypedSupportSurface::UsbAudio));
        assert!(!g3.supports(TypedSupportSurface::UsbAudio));
    }

    #[test]
    fn test_color_temperature_profile_metadata_matches_sources() {
        for caps in [
            profile_capabilities::<PtzOpticsG2>(),
            profile_capabilities::<PtzOpticsG3>(),
            profile_capabilities::<PtzOptics30x>(),
        ] {
            let white = caps.white_balance.as_ref().expect("white balance");
            assert_eq!(
                white.color_temp_range.map(|range| range.as_inclusive()),
                Some(2500..=8000)
            );
            assert_eq!(white.modes.len(), 6);
            assert!(white.modes.contains(&WhiteBalanceMode::ColorTemperature));
        }

        // These Sony sources do not grant the shared color-temperature mode
        // or register. FR7's own Kelvin families are separate typed surfaces.
        for caps in [
            profile_capabilities::<SonyEviH100>(),
            profile_capabilities::<SonyBrcH900>(),
            profile_capabilities::<SonyFr7>(),
        ] {
            let white = caps.white_balance.as_ref().expect("white balance");
            assert_eq!(white.color_temp_range, None, "{}", caps.model_name);
            assert!(
                !white.modes.contains(&WhiteBalanceMode::ColorTemperature),
                "{}",
                caps.model_name
            );
            assert!(
                !caps.supports(TypedSupportSurface::ColorTemperature),
                "{}",
                caps.model_name
            );
        }
    }

    #[test]
    fn test_capabilities_from_generic() {
        let caps = profile_capabilities::<GenericVisca>();

        assert_eq!(caps.model_name, "Generic VISCA Camera");
        assert!(caps
            .zoom
            .as_ref()
            .and_then(|domain| domain.digital_range())
            .map(|range| range.as_inclusive())
            .is_none());
        assert!(!caps.zoom.as_ref().is_some_and(|domain| domain.direct));
        assert_eq!(caps.presets.as_ref().expect("presets").highest, 5);
        assert_eq!(
            caps.presets
                .as_ref()
                .and_then(|domain| domain.recall_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(caps
            .exposure
            .as_ref()
            .and_then(|domain| domain.iris_range.clone())
            .is_some());
        assert!(caps
            .exposure
            .as_ref()
            .is_some_and(|domain| domain.modes.contains(&ExposureMode::Iris)));

        assert!(!caps.image.is_some());
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            None
        );
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.compensation_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.contrast_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.sharpness_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.luminance_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            caps.image
                .as_ref()
                .and_then(|domain| domain.gamma_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(caps.image.is_none());
        // GenericVisca has one-push white balance, which counts as an advanced feature
        assert!(caps
            .white_balance
            .as_ref()
            .is_some_and(|domain| domain.one_push));
        assert!(!caps.supports(TypedSupportSurface::NdFilter));
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(!caps
            .pan_tilt
            .as_ref()
            .is_some_and(|domain| domain.variable_speed));
        assert!(caps.supports(TypedSupportSurface::OnePushWhiteBalance));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn brc300_static_profile_preserves_documented_preset_metadata() {
        let profile =
            <SonyBrc300 as crate::CompileTimeProfile>::spec().expect("Sony BRC-300 static profile");
        let caps = profile.capabilities();

        assert_eq!(
            caps.presets.as_ref().expect("presets").highest,
            5,
            "CAM_Memory p is 0 through 5"
        );
        assert_eq!(
            caps.presets
                .as_ref()
                .and_then(|domain| domain.recall_speed_range)
                .map(|range| range.as_inclusive()),
            Some(1..=24),
            "Cmd_PT_M_Speed q is 1 through 24"
        );
    }

    #[test]
    fn brc300_discovery_keeps_preset_facts_separate_from_ptzoptics_typed_support() {
        let caps = profile_capabilities::<SonyBrc300>();

        assert_eq!(caps.presets.as_ref().expect("presets").highest, 5);
        assert_eq!(
            caps.presets
                .as_ref()
                .and_then(|domain| domain.recall_speed_range)
                .map(|range| range.as_inclusive()),
            Some(1..=24)
        );
        assert_eq!(
            caps.typed_support,
            <SonyBrc300 as crate::CompileTimeProfile>::TYPED_SUPPORT,
            "discovery must mirror the static typed-support registry"
        );
        assert!(
            !caps.supports(TypedSupportSurface::PresetRecallSpeed),
            "BRC-300 metadata must not grant the distinct PTZOptics typed command"
        );
    }

    #[test]
    fn brc300_capabilities_keep_reverse_axis_degree_ranges_ordered() {
        let caps = profile_capabilities::<SonyBrc300>();

        assert!(
            caps.pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .pan_range_degrees()
                .start()
                <= caps
                    .pan_tilt
                    .as_ref()
                    .expect("pan/tilt")
                    .pan_range_degrees()
                    .end()
        );
        assert!(
            caps.pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .tilt_range_degrees(None)
                .start()
                <= caps
                    .pan_tilt
                    .as_ref()
                    .expect("pan/tilt")
                    .tilt_range_degrees(None)
                    .end()
        );
        assert_eq!(
            *caps
                .pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .pan_range_degrees()
                .start(),
            0x08A58 as f32 / -208.0,
            "positive raw pan is the left/negative-degree endpoint"
        );
        assert_eq!(
            *caps
                .pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .pan_range_degrees()
                .end(),
            -0x08A58 as f32 / -208.0,
            "negative raw pan is the right/positive-degree endpoint"
        );
        assert_eq!(
            *caps
                .pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .tilt_range_degrees(None)
                .start(),
            -0x186A as f32 / 208.0,
            "negative raw tilt is the down/negative-degree endpoint"
        );
        assert_eq!(
            *caps
                .pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .tilt_range_degrees(None)
                .end(),
            0x493D as f32 / 208.0,
            "positive raw tilt is the up/positive-degree endpoint"
        );
    }

    #[test]
    fn test_unsupported_quality_ranges_are_none() {
        for caps in [
            profile_capabilities::<SonyBrc300>(),
            profile_capabilities::<NearusBrc300>(),
            profile_capabilities::<GenericVisca>(),
        ] {
            assert_eq!(
                caps.image
                    .as_ref()
                    .and_then(|domain| domain.contrast_range)
                    .map(|range| range.as_inclusive()),
                None
            );
            assert_eq!(
                caps.image
                    .as_ref()
                    .and_then(|domain| domain.sharpness_range)
                    .map(|range| range.as_inclusive()),
                None
            );
        }
        // R8 documents Aperture Level `00`..`0F` but no contrast control.
        let evi = profile_capabilities::<SonyEviH100>();
        assert_eq!(
            evi.image
                .as_ref()
                .and_then(|domain| domain.contrast_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert_eq!(
            evi.image
                .as_ref()
                .and_then(|domain| domain.sharpness_range)
                .map(|range| range.as_inclusive()),
            Some(0x00..=0x0F)
        );
        // R8, R12 and R21 document `04 4D` Bright Direct with their own
        // Bright tables; Generic VISCA does not assume it.
        assert_eq!(
            profile_capabilities::<SonyEviH100>()
                .exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            Some(CapabilityDomain::<u8>::with_gaps(
                0x00,
                0x1F,
                &[0x01, 0x02, 0x03, 0x04]
            ))
        );
        for caps in [
            profile_capabilities::<SonyBrc300>(),
            profile_capabilities::<NearusBrc300>(),
        ] {
            assert_eq!(
                caps.exposure
                    .as_ref()
                    .and_then(|domain| domain.brightness_range.clone()),
                Some(CapabilityDomain::<u8>::new(0x00, 0x17))
            );
        }
        assert_eq!(
            profile_capabilities::<GenericVisca>()
                .exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            None
        );

        assert!(profile_capabilities::<SonyEviH100>().image.is_some());
        assert!(profile_capabilities::<SonyBrc300>().image.is_some());
    }

    #[test]
    fn test_optional_vendor_feature_metadata_matrix() {
        for caps in [
            profile_capabilities::<PtzOpticsG2>(),
            profile_capabilities::<PtzOpticsG3>(),
            profile_capabilities::<PtzOptics30x>(),
        ] {
            assert!(!caps.focus.as_ref().is_some_and(|domain| domain.one_push));
            assert!(!caps.supports(TypedSupportSurface::NdFilter));

            assert_eq!(
                caps.pan_tilt
                    .as_ref()
                    .and_then(|domain| domain.motion_sync_speed_range)
                    .map(|range| range.as_inclusive()),
                None
            );
            assert!(!caps
                .pan_tilt
                .as_ref()
                .is_some_and(|domain| domain.variable_speed));
        }

        let fr7 = profile_capabilities::<SonyFr7>();
        assert!(fr7.supports(TypedSupportSurface::NdFilter));
        assert!(!fr7.focus.as_ref().is_some_and(|domain| domain.one_push));
        assert_eq!(
            fr7.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(fr7
            .pan_tilt
            .as_ref()
            .is_some_and(|domain| domain.variable_speed));

        for caps in [
            profile_capabilities::<GenericVisca>(),
            profile_capabilities::<SonyBrcH900>(),
            profile_capabilities::<SonyEviH100>(),
            profile_capabilities::<SonyBrc300>(),
            profile_capabilities::<NearusBrc300>(),
        ] {
            // R8, R12 and R21 document the One Push AF trigger; the BRC-H900
            // source does not.
            assert_eq!(
                caps.focus.as_ref().is_some_and(|domain| domain.one_push),
                caps.model_name != "Sony BRC-H900",
                "{}",
                caps.model_name
            );
            assert!(!caps.supports(TypedSupportSurface::NdFilter));

            assert_eq!(
                caps.pan_tilt
                    .as_ref()
                    .and_then(|domain| domain.motion_sync_speed_range)
                    .map(|range| range.as_inclusive()),
                None
            );
            assert!(!caps
                .pan_tilt
                .as_ref()
                .is_some_and(|domain| domain.variable_speed));
        }
    }

    #[test]
    fn test_capabilities_summary() {
        let caps = profile_capabilities::<PtzOpticsG2>();
        let summary = caps.summary();

        assert!(summary.contains("Model: PtzOptics G2"));
        assert!(summary.contains("Optical Zoom: lens-dependent"));
        assert!(summary.contains("0x4000"));
        assert!(!summary.contains("0x7000"));
        assert!(summary.contains("Presets: 0..=127"));
    }

    /// #828 M2: the G2 family's lens is declared per camera; the legacy
    /// PT30X profile's 30x lens is a profile fact (R3, docs/visca_reference.md
    /// §4.1).
    #[test]
    #[allow(clippy::expect_used)]
    fn zoom_scale_comes_from_the_profile_or_the_declared_lens() -> Result<(), crate::Error> {
        let g2 = profile_capabilities::<PtzOpticsG2>();
        let undeclared = g2.zoom_scale().expect_err("G2 lens is not a profile fact");
        assert!(
            matches!(&undeclared, crate::Error::InvalidRequest { reason: message }
                if message.contains("PtzOptics G2")
                    && message.contains("Capabilities::zoom_scale_for_lens(ratio)")),
            "{undeclared:?}"
        );
        for ratio in [12.0, 20.0, 30.0] {
            let lens = g2.zoom_scale_for_lens(ratio)?;
            assert_eq!(lens.units(ratio)?, 0x4000);
            assert_eq!(lens.units(1.0)?, 0);
        }
        for invalid in [1.0, 0.5, f32::NAN, f32::INFINITY] {
            assert!(matches!(
                g2.zoom_scale_for_lens(invalid),
                Err(crate::Error::InvalidParameter {
                    parameter: "optical zoom ratio",
                    ..
                })
            ));
        }

        let pt30x = profile_capabilities::<PtzOptics30x>();
        // 29 × 565.0 = 16385 units: the former scale could not reach 30x.
        assert_eq!(pt30x.zoom_scale()?.units(30.0)?, 0x4000);
        assert_eq!(pt30x.zoom_scale_for_lens(30.0)?, pt30x.zoom_scale()?);
        assert!(pt30x.zoom_scale_for_lens(20.0).is_err());

        let mut no_zoom = pt30x;
        no_zoom.zoom = None;
        assert!(matches!(
            no_zoom.zoom_scale(),
            Err(crate::Error::FeatureNotSupported { feature: "zoom" })
        ));
        assert!(no_zoom.zoom_scale_for_lens(20.0).is_err());
        Ok(())
    }
}

#[cfg(all(test, feature = "serde", feature = "schemars", feature = "ts-rs"))]
#[allow(clippy::expect_used)]
mod ts_contract_tests {
    use super::*;

    #[test]
    fn persisted_profile_dependencies_all_implement_the_export_contract() {
        fn persisted<
            T: serde::Serialize + serde::de::DeserializeOwned + schemars::JsonSchema + ts_rs::TS,
        >() {
        }
        persisted::<Capabilities>();
        persisted::<TypedSupportSet>();
        persisted::<crate::capabilities::PanTiltCaps>();
        persisted::<crate::capabilities::ZoomCaps>();
        persisted::<crate::capabilities::FocusCaps>();
        persisted::<crate::capabilities::ExposureCaps>();
        persisted::<crate::capabilities::WhiteBalanceCaps>();
        persisted::<crate::capabilities::ImageCaps>();
        persisted::<crate::capabilities::PresetCaps>();
        persisted::<crate::capabilities::PowerCaps>();
        persisted::<crate::capabilities::PowerStatusQuery>();
        persisted::<crate::capabilities::MenuCaps>();
        persisted::<crate::capabilities::CapabilityRange<i32>>();
        persisted::<crate::capabilities::CapabilityDomain<u16>>();
        persisted::<crate::capabilities::PanTiltWireCodec>();
        persisted::<crate::capabilities::CoordinateSystem>();
        persisted::<crate::capabilities::ShutterSpeedEntry>();
        persisted::<crate::capabilities::ProfileEncodings>();
        persisted::<crate::profiles::ProfileSpec>();
    }

    #[test]
    fn nullable_capability_keys_are_required_in_json_schema_and_typescript() {
        let caps = Capabilities::runtime_baseline("Schema fixture", 1).expect("baseline");
        let value = serde_json::to_value(caps).expect("serialize capabilities");
        let schema = schemars::schema_for!(Capabilities);
        let required = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .expect("required keys");
        let declaration = <Capabilities as ts_rs::TS>::decl(&ts_rs::Config::default());
        for key in [
            "profile_id",
            "pan_tilt",
            "zoom",
            "focus",
            "exposure",
            "white_balance",
            "image",
            "presets",
            "power",
            "menu",
        ] {
            assert_eq!(value[key], serde_json::Value::Null, "{key}");
            assert!(
                required.iter().any(|entry| entry == key),
                "schema omits required {key}"
            );
            assert!(
                declaration.contains(&format!("{key}:")),
                "TypeScript makes {key} optional: {declaration}"
            );
        }
        assert!(value["typed_support"].is_array());
        let support = <TypedSupportSet as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(support.contains("Array<TypedSupportSurface>"), "{support}");
        assert!(!support.contains("bigint"), "{support}");
    }

    #[test]
    fn power_query_schema_and_bindings_pin_both_dialects() {
        use crate::capabilities::{PowerCaps, PowerStatusQuery};
        let schema = schemars::schema_for!(PowerStatusQuery);
        let variants = schema.get("enum").cloned().or_else(|| {
            schema
                .get("oneOf")
                .and_then(serde_json::Value::as_array)
                .map(|variants| {
                    serde_json::Value::Array(
                        variants
                            .iter()
                            .filter_map(|variant| variant.get("const").cloned())
                            .collect(),
                    )
                })
        });
        assert_eq!(variants, Some(serde_json::json!(["standard", "extended"])));
        let declaration = <PowerStatusQuery as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(declaration.contains("\"standard\""), "{declaration}");
        assert!(declaration.contains("\"extended\""), "{declaration}");
        let schema = schemars::schema_for!(PowerCaps);
        assert!(schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .expect("required fields")
            .iter()
            .any(|field| field == "status_query"));
        let declaration = <PowerCaps as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(
            declaration.contains("status_query: PowerStatusQuery"),
            "{declaration}"
        );
    }

    #[test]
    fn duration_and_shared_table_fields_follow_their_serialized_shapes() {
        let config = ts_rs::Config::default();
        for declaration in [
            <crate::capabilities::PanTiltCaps as ts_rs::TS>::decl(&config),
            <crate::capabilities::PresetCaps as ts_rs::TS>::decl(&config),
            <crate::capabilities::PowerCaps as ts_rs::TS>::decl(&config),
        ] {
            assert!(declaration.contains("secs: number"), "{declaration}");
            assert!(declaration.contains("nanos: number"), "{declaration}");
            assert!(!declaration.contains("Duration"), "{declaration}");
        }
        let white_balance = <crate::capabilities::WhiteBalanceCaps as ts_rs::TS>::decl(&config);
        assert!(
            white_balance.contains("color_temperature_steps: Array<number>"),
            "{white_balance}"
        );
    }

    #[test]
    fn profile_export_closes_its_dependencies_without_private_fact_bindings() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let export_dir = std::env::temp_dir().join(format!(
            "grafton-visca-profile-ts-{}-{nonce}",
            std::process::id()
        ));
        let config = ts_rs::Config::default().with_out_dir(&export_dir);
        <crate::profiles::ProfileSpec as ts_rs::TS>::export_all(&config)
            .expect("complete profile dependencies export");
        let profile = std::fs::read_to_string(export_dir.join("ProfileSpec.ts"))
            .expect("canonical profile binding");
        assert!(profile.contains("type ProfileSpec = {"), "{profile}");
        assert!(!profile.contains("ProfileFacts"), "{profile}");
        for private in [
            "ProfileFacts.ts",
            "ProfileTimingSerde.ts",
            "PanTiltCoordinateConversionSerde.ts",
            "Duration.ts",
        ] {
            assert!(
                !export_dir.join(private).exists(),
                "private or fictional binding {private}"
            );
        }
        assert!(export_dir.join("Capabilities.ts").is_file());
        assert!(export_dir.join("TypedSupportSet.ts").is_file());
        std::fs::remove_dir_all(export_dir).expect("remove profile binding fixture");
    }
}
