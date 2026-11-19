//! Cohesive runtime capability domains. An absent domain carries no leaf facts.

use super::{CapabilityDomain, CapabilityRange};
use crate::{command::ExposureMode, WhiteBalanceMode};
use std::ops::RangeInclusive;

/// Which endpoint of an ordered focus range is nearest to the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum FocusNearEnd {
    /// The source establishes raw positions but not their physical orientation.
    Unknown,
    /// The numerical minimum is the near endpoint.
    Minimum,
    /// The numerical maximum is the near endpoint.
    Maximum,
}

/// How a source relates nominal degree bounds to quantized wire endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum EndpointQuantization {
    /// Degree bounds are the exact wire endpoints divided by the scale.
    Exact,
    /// The source states whole-degree bounds whose wire endpoints are rounded.
    /// A nominal endpoint must lie within one wire unit of the stated endpoint.
    WholeDegrees,
}

impl EndpointQuantization {
    /// Derives the nominal degree interval from ordered exact degree bounds.
    #[must_use]
    pub fn degree_range(self, exact: RangeInclusive<f32>) -> RangeInclusive<f32> {
        match self {
            Self::Exact => exact,
            Self::WholeDegrees => exact.start().round()..=exact.end().round(),
        }
    }
}

/// Source-backed tilt limits, with installation selection only where documented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum TiltRanges {
    /// One unconditional tilt range.
    Single(CapabilityRange<i32>),
    /// The ranges selected by the camera's installation menu.
    MountDependent {
        /// Upright desktop range.
        desktop: CapabilityRange<i32>,
        /// Inverted ceiling range.
        ceiling: CapabilityRange<i32>,
    },
}

impl TiltRanges {
    /// Returns whether the source documents installation-dependent limits.
    #[must_use]
    pub const fn supports_mount_selection(self) -> bool {
        matches!(self, Self::MountDependent { .. })
    }

    /// Returns the selected source range; no selection uses the desktop default.
    /// Target registration refuses mount selection on a single-range profile.
    #[must_use]
    pub const fn range(self, mount: Option<crate::MountPosition>) -> CapabilityRange<i32> {
        match (self, mount) {
            (Self::Single(range), _) => range,
            (Self::MountDependent { ceiling, .. }, Some(crate::MountPosition::Ceiling)) => ceiling,
            (Self::MountDependent { desktop, .. }, _) => desktop,
        }
    }
}

/// Pan/tilt source facts, present only for a camera with this domain.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PanTiltCaps {
    /// Sourced degree positioning and position-reply facts, when available.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub geometry: Option<PanTiltGeometry>,
    /// Supported nonzero pan speeds.
    pub pan_speed: CapabilityRange<u8>,
    /// Supported nonzero tilt speeds.
    pub tilt_speed: CapabilityRange<u8>,
    /// Whether the variable-speed mode family is supported.
    pub variable_speed: bool,
    /// Supported motion-sync speeds, when available.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub motion_sync_speed_range: Option<CapabilityRange<u8>>,
}

/// Position limits, coordinate scales and wire framing for degree positioning.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PanTiltGeometry {
    /// Pan position bounds in wire units.
    pub pan_range: CapabilityRange<i32>,
    /// Fixed or mount-selected tilt bounds in wire units.
    pub tilt_ranges: TiltRanges,
    /// Validated coordinate and wire conversion.
    pub coordinates: crate::PanTiltCoordinateConversion,
    /// Source policy for the nominal degree endpoint cells.
    pub endpoint_quantization: EndpointQuantization,
}

/// Optical lens and optional digital zoom facts.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct ZoomCaps {
    /// Maximum optical position; the optical range starts at zero.
    pub optical_max: u16,
    /// Maximum digital position; its range starts at the optical maximum.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub digital_max: Option<u16>,
    /// Supported variable zoom speeds.
    pub speed: CapabilityRange<u8>,
    /// Whether direct absolute positioning is documented.
    pub direct: bool,
    /// Whether variable-speed zoom drive is documented.
    pub variable: bool,
    /// Fixed lens magnification, or no fixed lens ratio.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub optical_ratio: Option<f32>,
}

impl ZoomCaps {
    /// Converts a normalized position through a documented zoom domain.
    ///
    /// Uses the selected range's nearest-integer rounding, with halves away
    /// from zero.
    pub fn position_at(
        &self,
        position: crate::units::UnitInterval,
        domain: crate::ZoomDomain,
    ) -> crate::Result<crate::types::ZoomPosition> {
        crate::types::ZoomPosition::new(self.normalization_range(domain)?.denormalize(position))
    }

    /// Normalizes a raw position within the selected documented domain.
    pub fn normalize(
        &self,
        position: crate::types::ZoomPosition,
        domain: crate::ZoomDomain,
    ) -> crate::Result<crate::units::UnitInterval> {
        self.normalization_range(domain)?
            .normalize(position.value())
    }

    pub(crate) fn normalization_range(
        &self,
        domain: crate::ZoomDomain,
    ) -> crate::Result<CapabilityRange<u16>> {
        match domain {
            crate::ZoomDomain::Optical => Ok(self.optical_range()),
            crate::ZoomDomain::OpticalPlusDigital => {
                let maximum = self.digital_max.ok_or(crate::Error::FeatureNotSupported {
                    feature: "optical-plus-digital zoom range",
                })?;
                CapabilityRange::<u16>::new(0, maximum)
            }
        }
    }
}

/// Direction-free focus limits and source-backed refinements.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct FocusCaps {
    /// Ordered focus bounds in wire units.
    pub range: CapabilityRange<u16>,
    /// Which bound is nearest to the camera.
    pub near_end: FocusNearEnd,
    /// Supported manual focus speeds.
    pub speed: CapabilityRange<u8>,
    /// Whether automatic focus is supported.
    pub auto_focus: bool,
    /// Whether one-push focus is documented.
    pub one_push: bool,
    /// Whether the focus-zone reply is documented.
    pub zone_inquiry: bool,
    /// Permitted focus-zone values; empty when selection is unsupported.
    pub zones: Vec<crate::command::FocusZone>,
    /// Whether AF sensitivity adjustment is documented.
    pub af_sensitivity: bool,
    /// Whether focus near-limit readback is documented.
    pub near_limit_inquiry: bool,
}

/// Exposure inventories and optional shared command domains.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct ExposureCaps {
    /// Supported shared exposure modes.
    pub modes: Vec<ExposureMode>,
    /// Whether shared backlight compensation is supported.
    pub backlight_compensation: bool,
    /// Whether the shared dynamic-range family is supported.
    pub wide_dynamic_range: bool,
    /// Supported shared exposure compensation levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub compensation_range: Option<CapabilityRange<i8>>,
    /// Sourced shared iris positions, including gaps.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub iris_range: Option<CapabilityDomain<u16>>,
    /// Shared gain levels, absent for a different vendor gain family.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub gain_range: Option<CapabilityRange<u8>>,
    /// Admissible raw shutter codes, when direct shutter control is supported.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub shutter_codes: Option<CapabilityDomain<u8>>,
    /// Exact exposure-time to shutter-code inventory. Empty if the mapping is unknown.
    pub shutter_speeds: Vec<super::ShutterSpeedEntry>,
    /// Supported exposure brightness positions.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub brightness_range: Option<CapabilityDomain<u8>>,
}

/// Both channels of the manual RGB-gain domain.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct RgbGainRanges {
    /// Supported red gain levels.
    pub red: CapabilityRange<u8>,
    /// Supported blue gain levels.
    pub blue: CapabilityRange<u8>,
}

/// Both channels of the RGB-tuning domain.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct RgbTuningRanges {
    /// Supported red tuning levels.
    pub red: CapabilityRange<i8>,
    /// Supported blue tuning levels.
    pub blue: CapabilityRange<i8>,
}

/// White-balance mode inventory and optional numeric domains.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct WhiteBalanceCaps {
    /// Supported white-balance modes; a present domain has at least one mode.
    pub modes: Vec<WhiteBalanceMode>,
    /// Whether one-push white balance is supported.
    pub one_push: bool,
    /// Physical Kelvin value selected by each color-temperature wire code.
    ///
    /// An empty table declares no shared direct color-temperature encoding.
    pub color_temperature_steps: std::sync::Arc<[u16]>,
    /// Manual gain exists only with both channels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub rgb_gain: Option<RgbGainRanges>,
    /// RGB tuning exists only with both channels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub rgb_tuning: Option<RgbTuningRanges>,
}

impl WhiteBalanceCaps {
    /// Returns the smallest and largest Kelvin values in the step table.
    ///
    /// An empty table has no range. The table lists the accepted temperatures,
    /// including any gaps between the endpoints.
    #[must_use]
    pub fn color_temperature_range(&self) -> Option<CapabilityRange<u16>> {
        let min = self.color_temperature_steps.iter().copied().min()?;
        let max = self.color_temperature_steps.iter().copied().max()?;
        CapabilityRange::<u16>::new(min, max).ok()
    }

    /// Lowers a supported physical temperature through this profile's step table.
    pub fn color_temperature_wire(&self, temperature: crate::types::Kelvin) -> crate::Result<u16> {
        Self::temperature_step(&self.color_temperature_steps, temperature)
    }

    pub(crate) fn temperature_step(
        steps: &[u16],
        temperature: crate::types::Kelvin,
    ) -> crate::Result<u16> {
        steps
            .iter()
            .position(|value| *value == temperature.value())
            .and_then(|index| u16::try_from(index).ok())
            .ok_or_else(|| {
                crate::Error::invalid_parameter(
                    "color_temperature",
                    temperature.value().to_string(),
                    "temperature must be a documented profile step",
                )
            })
    }

    /// Decodes a documented wire step to its physical temperature.
    pub fn color_temperature_from_wire(&self, code: u16) -> crate::Result<crate::types::Kelvin> {
        Self::temperature_from_step(&self.color_temperature_steps, code)
    }

    pub(crate) fn temperature_from_step(
        steps: &[u16],
        code: u16,
    ) -> crate::Result<crate::types::Kelvin> {
        let temperature = steps.get(usize::from(code)).copied().ok_or_else(|| {
            crate::Error::invalid_parameter(
                "color_temperature_wire",
                code.to_string(),
                "unknown profile temperature step",
            )
        })?;
        crate::types::Kelvin::new(temperature)
    }
}

/// Image-processing source facts and optional adjustment domains.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct ImageCaps {
    /// Supported contrast levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub contrast_range: Option<CapabilityRange<u8>>,
    /// Supported sharpness levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub sharpness_range: Option<CapabilityRange<u8>>,
    /// Supported saturation levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub saturation_range: Option<CapabilityRange<u8>>,
    /// Supported hue levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub hue_range: Option<CapabilityRange<u8>>,
    /// Supported luminance levels.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub luminance_range: Option<CapabilityRange<u8>>,
    /// Supported gamma curves.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub gamma_range: Option<CapabilityRange<u8>>,
    /// Whether separate vertical image flip is documented.
    pub flip: bool,
    /// Whether separate horizontal mirroring is documented.
    pub mirror: bool,
    /// Picture-effect values documented for this model; empty means unsupported.
    pub picture_effects: Vec<crate::types::PictureEffectMode>,
}

/// Preset memory facts; presence supplies an exact recall-axis set.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PresetCaps {
    /// Highest memory number in the zero-based preset inventory.
    pub highest: u8,
    /// Exact physical axes changed by recall.
    pub recall_axes: crate::AffectedAxes,
    /// Supported shared recall speeds.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profiles::nullable_schema))]
    pub recall_speed_range: Option<CapabilityRange<u8>>,
}

/// Packet category used by a camera's power-status inquiry.
///
/// This is a profile fact: an exported custom profile keeps the same query
/// packet after its built-in identity is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PowerStatusQuery {
    /// Standard VISCA `09 04 00` power-status query.
    #[cfg_attr(feature = "serde", serde(rename = "standard"))]
    Standard,
    /// Extended-category `09 7E 04 00` power-status query.
    #[cfg_attr(feature = "serde", serde(rename = "extended"))]
    Extended,
}

impl PowerStatusQuery {
    pub(crate) const fn body(self) -> &'static [u8] {
        use crate::command::bytes::constants::power;
        match self {
            Self::Standard => &power::STATE_INQUIRY,
            Self::Extended => &power::BRC_H900_STATE_INQUIRY,
        }
    }

    /// Allocation bound derived from this family's declared packet bodies.
    pub(crate) const fn maximum_body() -> &'static [u8] {
        let standard = Self::Standard.body();
        let extended = Self::Extended.body();
        if standard.len() > extended.len() {
            standard
        } else {
            extended
        }
    }
}

/// Power-query dialect and standby support for a camera with power control.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PowerCaps {
    /// Source-backed packet category for the power-status inquiry.
    pub status_query: PowerStatusQuery,
    /// Whether the camera supports the standby command.
    pub standby: bool,
}

impl PanTiltGeometry {
    /// Derives the pan bounds in nominal logical degrees.
    #[must_use]
    pub fn pan_range_degrees(&self) -> RangeInclusive<f32> {
        self.endpoint_quantization.degree_range(
            self.coordinates
                .pan_degree_range(&self.pan_range.as_inclusive()),
        )
    }

    /// Derives the selected tilt bounds in nominal logical degrees.
    #[must_use]
    pub fn tilt_range_degrees(&self, mount: Option<crate::MountPosition>) -> RangeInclusive<f32> {
        self.endpoint_quantization.degree_range(
            self.coordinates
                .tilt_degree_range(&self.tilt_ranges.range(mount).as_inclusive()),
        )
    }
}

impl ZoomCaps {
    /// Derives the zero-based optical position interval.
    #[must_use]
    pub const fn optical_range(&self) -> CapabilityRange<u16> {
        CapabilityRange::<u16>::from_registry(0, self.optical_max)
    }

    /// Derives the digital position interval sharing the optical boundary.
    /// Returns `None` when no digital maximum exists or it is below the optical maximum.
    #[must_use]
    pub fn digital_range(&self) -> Option<CapabilityRange<u16>> {
        self.digital_max
            .and_then(|maximum| CapabilityRange::<u16>::new(self.optical_max, maximum).ok())
    }
}

impl FocusCaps {
    /// Returns the sourced near endpoint, or `None` when orientation is unknown.
    #[must_use]
    pub const fn near(&self) -> Option<u16> {
        match self.near_end {
            FocusNearEnd::Unknown => None,
            FocusNearEnd::Minimum => Some(self.range.min()),
            FocusNearEnd::Maximum => Some(self.range.max()),
        }
    }

    /// Returns the sourced far endpoint, or `None` when orientation is unknown.
    #[must_use]
    pub const fn far(&self) -> Option<u16> {
        match self.near_end {
            FocusNearEnd::Unknown => None,
            FocusNearEnd::Minimum => Some(self.range.max()),
            FocusNearEnd::Maximum => Some(self.range.min()),
        }
    }
}

impl super::Capabilities {
    /// Checks only constraints not already carried by the domain representation.
    pub(crate) fn validate_domains(&self) -> crate::Result<()> {
        use crate::profiles::invalid_profile_fields as invalid;
        fn unique<T: PartialEq>(values: &[T]) -> bool {
            values
                .iter()
                .enumerate()
                .all(|(index, value)| !values[..index].contains(value))
        }
        fn domain<T: Copy>(
            field: &'static str,
            bounds: [T; 2],
            permits: impl Fn(T) -> bool,
        ) -> crate::Result<()> {
            if bounds.into_iter().all(permits) {
                Ok(())
            } else {
                Err(crate::profiles::invalid_profile_fields(
                    &[field],
                    "advertised capability ranges must fit their typed value domains",
                ))
            }
        }
        macro_rules! optional {
            ($field:literal, $range:expr, $valid:expr) => {
                if let Some(range) = &$range {
                    domain($field, [range.min(), range.max()], $valid)?;
                }
            };
        }
        if let Some(pan_tilt) = &self.pan_tilt {
            domain(
                "capabilities.pan_tilt.pan_speed",
                [pan_tilt.pan_speed.min(), pan_tilt.pan_speed.max()],
                |value| value != 0 && crate::types::PanSpeed::new(value).is_ok(),
            )?;
            domain(
                "capabilities.pan_tilt.tilt_speed",
                [pan_tilt.tilt_speed.min(), pan_tilt.tilt_speed.max()],
                |value| value != 0 && crate::types::TiltSpeed::new(value).is_ok(),
            )?;
            for (field, maximum) in [
                ("capabilities.pan_tilt.pan_speed", pan_tilt.pan_speed.max()),
                (
                    "capabilities.pan_tilt.tilt_speed",
                    pan_tilt.tilt_speed.max(),
                ),
            ] {
                if maximum > crate::command::PanTiltSpeedMode::STANDARD_MAXIMUM {
                    if !pan_tilt.variable_speed {
                        return Err(invalid(
                            &[field, "capabilities.pan_tilt.variable_speed"],
                            "speeds above the standard maximum require variable speed modes",
                        ));
                    }
                    if !self
                        .typed_support
                        .contains(super::TypedSupportSurface::PanTiltSpeedMode)
                    {
                        return Err(invalid(
                            &[field, "capabilities.typed_support"],
                            "speeds above the standard maximum require the pan/tilt speed-mode permission",
                        ));
                    }
                }
            }
            optional!(
                "capabilities.pan_tilt.motion_sync_speed_range",
                pan_tilt.motion_sync_speed_range,
                |value| crate::types::MotionSyncSpeed::new(value).is_ok()
            );
            if let Some(geometry) = pan_tilt.geometry {
                let framing = crate::command::pan_tilt_wire::PanTiltFraming::for_conversion(
                    geometry.coordinates,
                );
                let (pan_word, tilt_word) = (framing.pan_maximum(), framing.tilt_maximum());
                domain(
                    "capabilities.pan_tilt.geometry.pan_range",
                    [geometry.pan_range.min(), geometry.pan_range.max()],
                    |value| (-pan_word - 1..=pan_word).contains(&value),
                )?;
                let tilt_ranges = match geometry.tilt_ranges {
                    TiltRanges::Single(range) => [Some(range), None],
                    TiltRanges::MountDependent { desktop, ceiling } => {
                        [Some(desktop), Some(ceiling)]
                    }
                };
                for range in tilt_ranges.into_iter().flatten() {
                    domain(
                        "capabilities.pan_tilt.geometry.tilt_ranges",
                        [range.min(), range.max()],
                        |value| (-tilt_word - 1..=tilt_word).contains(&value),
                    )?;
                    if geometry.endpoint_quantization == EndpointQuantization::WholeDegrees {
                        for (raw, scale) in [
                            (
                                geometry.pan_range,
                                geometry.coordinates.pan_degrees_to_units(),
                            ),
                            (range, geometry.coordinates.tilt_degrees_to_units()),
                        ] {
                            if [raw.min(), raw.max()].into_iter().any(|endpoint| {
                                let nominal = (endpoint as f32 / scale).round();
                                (nominal * scale - endpoint as f32).abs() > 1.0
                            }) {
                                return Err(invalid(&["capabilities.pan_tilt.geometry.endpoint_quantization"], "nominal whole-degree endpoints must be within one wire unit of their sourced endpoints"));
                            }
                        }
                    }
                }
            }
        }
        if let Some(zoom) = &self.zoom {
            domain(
                "capabilities.zoom.optical_max",
                [zoom.optical_max; 2],
                |value| crate::types::ZoomPosition::new(value).is_ok(),
            )?;
            if let Some(maximum) = zoom.digital_max {
                domain("capabilities.zoom.digital_max", [maximum; 2], |value| {
                    crate::types::ZoomPosition::new(value).is_ok()
                })?;
            }
            if zoom.optical_max == 0
                || zoom
                    .digital_max
                    .is_some_and(|maximum| maximum <= zoom.optical_max)
            {
                return Err(invalid(&["capabilities.zoom"], "optical range must be nonempty and digital maximum must exceed optical maximum"));
            }
            domain(
                "capabilities.zoom.speed",
                [zoom.speed.min(), zoom.speed.max()],
                |value| crate::types::ZoomSpeed::new(value).is_ok(),
            )?;
            if let Some(ratio) = zoom.optical_ratio {
                super::ZoomScale::new(zoom.optical_max, ratio).map_err(|_| {
                    invalid(
                        &["capabilities.zoom.optical_ratio"],
                        "lens magnification must be finite and greater than one",
                    )
                })?;
            }
        }
        if let Some(focus) = &self.focus {
            if focus.range.min() == focus.range.max() {
                return Err(invalid(
                    &["capabilities.focus.range"],
                    "focus must have distinct near and far endpoints",
                ));
            }
            domain(
                "capabilities.focus.speed",
                [focus.speed.min(), focus.speed.max()],
                |value| crate::types::FocusSpeed::new(value).is_ok(),
            )?;
            if (!focus.auto_focus && focus.af_sensitivity) || !unique(&focus.zones) {
                return Err(invalid(
                    &["capabilities.focus"],
                    "AF sensitivity requires automatic focus and focus zones must be unique",
                ));
            }
        }
        if let Some(exposure) = &self.exposure {
            optional!(
                "capabilities.exposure.compensation_range",
                exposure.compensation_range,
                |value| crate::types::ExposureCompensationLevel::new(value).is_ok()
            );
            optional!(
                "capabilities.exposure.iris_range",
                exposure.iris_range,
                |value| u8::try_from(value).is_ok()
            );
            if exposure.shutter_speeds.iter().any(|entry| {
                !exposure
                    .shutter_codes
                    .as_ref()
                    .is_some_and(|codes| codes.contains(entry.value))
            }) {
                return Err(invalid(
                    &[
                        "capabilities.exposure.shutter_speeds",
                        "capabilities.exposure.shutter_codes",
                    ],
                    "shutter time entries require codes in the shutter domain",
                ));
            }
            if !unique(&exposure.modes)
                || exposure
                    .shutter_speeds
                    .iter()
                    .enumerate()
                    .any(|(index, entry)| {
                        exposure.shutter_speeds[..index].iter().any(|earlier| {
                            earlier.value == entry.value || earlier.exposure == entry.exposure
                        })
                    })
            {
                return Err(invalid(
                    &["capabilities.exposure"],
                    "mode, shutter-code and shutter-time inventories must be unique",
                ));
            }
        }
        if let Some(white_balance) = &self.white_balance {
            if white_balance.modes.is_empty() || !unique(&white_balance.modes) {
                return Err(invalid(
                    &["capabilities.white_balance.modes"],
                    "white-balance inventory must be nonempty and unique",
                ));
            }
            let steps = &white_balance.color_temperature_steps;
            if steps.len() > 256
                || !unique(steps)
                || steps
                    .iter()
                    .any(|&value| crate::types::Kelvin::new(value).is_err())
            {
                return Err(invalid(
                    &["capabilities.white_balance.color_temperature_steps"],
                    "temperature steps must map at most 256 byte codes to distinct valid Kelvin values",
                ));
            }
            if let Some(gain) = &white_balance.rgb_gain {
                domain(
                    "capabilities.white_balance.rgb_gain.red",
                    [gain.red.min(), gain.red.max()],
                    |value| crate::types::RgbGain::new(value).is_ok(),
                )?;
                domain(
                    "capabilities.white_balance.rgb_gain.blue",
                    [gain.blue.min(), gain.blue.max()],
                    |value| crate::types::RgbGain::new(value).is_ok(),
                )?;
            }
            if let Some(tuning) = &white_balance.rgb_tuning {
                domain(
                    "capabilities.white_balance.rgb_tuning.red",
                    [tuning.red.min(), tuning.red.max()],
                    |value| crate::types::RgbTuning::new(value).is_ok(),
                )?;
                domain(
                    "capabilities.white_balance.rgb_tuning.blue",
                    [tuning.blue.min(), tuning.blue.max()],
                    |value| crate::types::RgbTuning::new(value).is_ok(),
                )?;
            }
        }
        if let Some(image) = &self.image {
            if !unique(&image.picture_effects) {
                return Err(invalid(
                    &["capabilities.image.picture_effects"],
                    "picture-effect inventory must be unique",
                ));
            }
            optional!(
                "capabilities.image.contrast_range",
                image.contrast_range,
                |value| crate::types::ContrastLevel::new(value).is_ok()
            );
            optional!(
                "capabilities.image.sharpness_range",
                image.sharpness_range,
                |value| crate::types::SharpnessLevel::new(value).is_ok()
            );
            optional!(
                "capabilities.image.saturation_range",
                image.saturation_range,
                |value| crate::types::SaturationLevel::new(value).is_ok()
            );
            optional!("capabilities.image.hue_range", image.hue_range, |value| {
                crate::types::HueLevel::new(value).is_ok()
            });
            optional!(
                "capabilities.image.luminance_range",
                image.luminance_range,
                |value| crate::types::LuminanceLevel::new(value).is_ok()
            );
            optional!(
                "capabilities.image.gamma_range",
                image.gamma_range,
                |value| crate::types::GammaLevel::new(value).is_ok()
            );
        }
        if let Some(presets) = &self.presets {
            optional!(
                "capabilities.presets.recall_speed_range",
                presets.recall_speed_range,
                |value| crate::command::PresetSpeed::new(value).is_ok()
            );
        }
        Ok(())
    }
}

/// Presence of the source-backed menu domain.
///
/// Individual menu controls and inquiries still require their own typed grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct MenuCaps {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn temperature_range_has_admitted_endpoints_and_keeps_the_wire_order() {
        let profile = crate::profiles::ProfileId::PtzOpticsG2
            .spec()
            .expect("profile");
        let mut caps = profile.capabilities().clone();
        let white = caps.white_balance.as_mut().expect("white balance");
        white.color_temperature_steps = [5700, 3000, 6200].into();
        assert_eq!(
            white
                .color_temperature_range()
                .map(|range| range.as_inclusive()),
            Some(3000..=6200),
        );
        for (kelvin, code) in [(3000, 1), (6200, 2)] {
            assert_eq!(
                white
                    .color_temperature_wire(
                        crate::types::Kelvin::new(kelvin).expect("valid Kelvin")
                    )
                    .expect("listed step"),
                code,
            );
        }
        assert!(white
            .color_temperature_wire(crate::types::Kelvin::new(5000).expect("valid Kelvin"))
            .is_err());
        assert!(caps.validate_domains().is_ok());

        caps.white_balance
            .as_mut()
            .expect("white balance")
            .color_temperature_steps = [].into();
        assert_eq!(
            caps.white_balance
                .as_ref()
                .expect("white balance")
                .color_temperature_range(),
            None
        );
        assert!(caps.validate_domains().is_ok());
    }

    #[test]
    fn temperature_step_validation_rejects_invalid_values_duplicates_and_excess_codes() {
        let profile = crate::profiles::ProfileId::PtzOpticsG2
            .spec()
            .expect("profile");
        let mut caps = profile.capabilities().clone();
        for steps in [vec![3200, 3200], vec![0], (2500..=2756).collect()] {
            caps.white_balance
                .as_mut()
                .expect("white balance")
                .color_temperature_steps = steps.into();
            let error = caps
                .validate_domains()
                .expect_err("invalid temperature steps");
            assert!(error
                .to_string()
                .contains("capabilities.white_balance.color_temperature_steps"));
        }
    }

    #[test]
    fn shutter_time_entries_require_an_admitted_code() {
        let profile = crate::profiles::ProfileId::PtzOpticsG2
            .spec()
            .expect("profile");
        let mut caps = profile.capabilities().clone();
        let entry =
            super::super::ShutterSpeedEntry::new(crate::units::ExposureTime::reciprocal(60), 1);
        for domain in [
            None,
            Some(CapabilityDomain::<u8>::new(2, 3, &[][..]).expect("valid domain")),
            Some(CapabilityDomain::<u8>::new(0, 3, &[1][..]).expect("valid domain")),
        ] {
            let exposure = caps.exposure.as_mut().expect("exposure domain");
            exposure.shutter_speeds = vec![entry];
            exposure.shutter_codes = domain;
            assert!(caps.validate_domains().is_err());
        }
        caps.exposure
            .as_mut()
            .expect("exposure domain")
            .shutter_codes =
            Some(CapabilityDomain::<u8>::new(0, 3, &[][..]).expect("valid domain"));
        assert!(caps.validate_domains().is_ok());
    }
}
