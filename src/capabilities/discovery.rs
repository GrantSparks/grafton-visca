//! Structured capabilities response for runtime feature discovery.
//!
//! This module provides a unified `Capabilities` struct that exposes
//! camera capabilities at runtime, complementing the compile-time
//! trait-based capability system.

use std::borrow::Cow;

use super::{profile_metadata::InquirySupport, TypedSupportSet, TypedSupportSurface};
use crate::{command::exposure::ExposureMode, WhiteBalanceMode};

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
pub struct Capabilities {
    /// Built-in identity, absent for a custom runtime profile.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub profile_id: Option<crate::camera::profiles::ProfileId>,
    /// Camera model name.
    pub model_name: String,
    /// Validated individual default target.
    pub default_camera_id: crate::IndividualCameraId,
    /// Pan/tilt facts, present exactly when the domain is supported.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub pan_tilt: Option<super::PanTiltCaps>,
    /// Optical and digital lens facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub zoom: Option<super::ZoomCaps>,
    /// Focus bounds, orientation and refinements.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub focus: Option<super::FocusCaps>,
    /// Exposure inventories and shared numeric domains.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub exposure: Option<super::ExposureCaps>,
    /// White-balance modes and numeric domains.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub white_balance: Option<super::WhiteBalanceCaps>,
    /// Image-processing adjustment facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub image: Option<super::ImageCaps>,
    /// Preset inventory and exact recall axes.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub presets: Option<super::PresetCaps>,
    /// Power sequencing facts.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = crate::profile::nullable_schema))]
    pub power: Option<super::PowerCaps>,
    /// Implemented ND mode; no separate presence flag.
    pub nd_filter_mode: super::NdFilterMode,
    /// Whether the tally domain is supported.
    pub tally: bool,
    /// Whether direct menu control is supported.
    pub direct_menu: bool,
    /// Whether USB audio control is supported.
    pub usb_audio: bool,
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
    /// documented facts, then pass the value to [`ProfileSpec::builder`](crate::ProfileSpec::builder).
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
            return Err(crate::Error::InvalidRequest(
                "runtime capability model name must not be empty".into(),
            ));
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
            nd_filter_mode: super::NdFilterMode::None,
            tally: false,
            direct_menu: false,
            usb_audio: false,
            inquiry_support: InquirySupport::None,
            typed_support: TypedSupportSet::empty(),
        })
    }

    /// Creates a new Capabilities struct from a camera profile.
    ///
    /// This method extracts all capability information from the compile-time
    /// trait constants and creates a runtime-queryable struct.
    pub fn from_profile<P: crate::CompileTimeProfile>() -> Self {
        use super::{
            ExposureCaps, FocusCaps, ImageCaps, PanTiltCaps, PowerCaps, PresetCaps, RgbGainRanges,
            RgbTuningRanges, WhiteBalanceCaps, ZoomCaps,
        };
        const {
            assert!(
                P::RED_GAIN_RANGE.is_some() == P::BLUE_GAIN_RANGE.is_some(),
                "RGB gain requires both source ranges"
            );
            assert!(
                P::RG_TUNING_RANGE.is_some() == P::BG_TUNING_RANGE.is_some(),
                "RGB tuning requires both source ranges"
            );
        }
        Self {
            profile_id: P::PROFILE_ID,
            model_name: P::MODEL_NAME.into(),
            default_camera_id: P::DEFAULT_CAMERA_ID,
            pan_tilt: Some(PanTiltCaps {
                pan_range: P::PAN_RANGE,
                tilt_ranges: P::TILT_RANGES,
                pan_speed: super::CapabilityRange::<u8>::new(1, P::MAX_PAN_SPEED),
                tilt_speed: super::CapabilityRange::<u8>::new(1, P::MAX_TILT_SPEED),
                coordinates: crate::PanTiltCoordinateConversion::for_profile::<P>(),
                endpoint_quantization: P::ENDPOINT_QUANTIZATION,
                simultaneous: P::PAN_TILT_SIMULTANEOUS,
                preset_recovery_time: P::PRESET_RECOVERY_TIME,
                variable_speed: P::SUPPORTS_VARIABLE_SPEED,
                motion_sync_speed_range: P::MOTION_SYNC_SPEED_RANGE,
            }),
            zoom: Some(ZoomCaps {
                optical_max: P::OPTICAL_ZOOM_MAX,
                digital_max: P::DIGITAL_ZOOM_MAX,
                speed: P::ZOOM_SPEED_RANGE,
                direct: P::SUPPORTS_DIRECT_ZOOM,
                variable: P::SUPPORTS_VARIABLE_ZOOM,
                optical_ratio: P::OPTICAL_ZOOM_RATIO,
            }),
            focus: Some(FocusCaps {
                range: P::FOCUS_RANGE,
                near_end: P::FOCUS_NEAR_END,
                speed: super::CapabilityRange::<u8>::new(0, P::MAX_FOCUS_SPEED),
                auto_focus: P::SUPPORTS_AUTO_FOCUS,
                one_push: P::SUPPORTS_ONE_PUSH_FOCUS,
                zone_inquiry: P::SUPPORTS_FOCUS_ZONE_INQUIRY,
                zones: P::FOCUS_ZONES.to_vec(),
                af_sensitivity: P::SUPPORTS_AF_SENSITIVITY,
                near_limit_inquiry: P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY,
            }),
            exposure: Some(ExposureCaps {
                modes: P::EXPOSURE_MODES.to_vec(),
                backlight: P::SUPPORTS_BACKLIGHT_COMP,
                wdr: P::SUPPORTS_WDR,
                compensation_range: P::EXPOSURE_COMP_RANGE,
                iris_range: P::IRIS_RANGE,
                gain_range: P::GAIN_RANGE,
                shutter_speeds: P::SHUTTER_SPEEDS.to_vec(),
                brightness_range: P::BRIGHTNESS_RANGE,
            }),
            white_balance: Some(WhiteBalanceCaps {
                modes: P::WB_MODES.to_vec(),
                one_push: P::SUPPORTS_ONE_PUSH_WB,
                color_temp_range: P::COLOR_TEMP_RANGE,
                color_temperature_steps: P::COLOR_TEMP_RANGE.map_or_else(
                    || std::sync::Arc::from([]),
                    |_| {
                        (0..=55)
                            .map(|code| 2500 + code * 100)
                            .collect::<Vec<_>>()
                            .into()
                    },
                ),
                rgb_gain: P::RED_GAIN_RANGE
                    .zip(P::BLUE_GAIN_RANGE)
                    .map(|(red, blue)| RgbGainRanges { red, blue }),
                rgb_tuning: P::RG_TUNING_RANGE
                    .zip(P::BG_TUNING_RANGE)
                    .map(|(red, blue)| RgbTuningRanges { red, blue }),
            }),
            image: P::SUPPORTS_IMAGE_PROCESSING.then_some(ImageCaps {
                contrast_range: P::CONTRAST_RANGE,
                sharpness_range: P::SHARPNESS_RANGE,
                saturation_range: P::SATURATION_RANGE,
                hue_range: P::HUE_RANGE,
                luminance_range: P::LUMINANCE_RANGE,
                gamma_range: P::GAMMA_RANGE,
                flip: P::SUPPORTS_FLIP,
                mirror: P::SUPPORTS_MIRROR,
                requires_settings_save_for_flip: P::REQUIRES_SETTINGS_SAVE_FOR_FLIP,
                picture_effect: P::SUPPORTS_PICTURE_EFFECT,
            }),
            presets: P::PRESET_RECALL_AXES.map(|recall_axes| PresetCaps {
                highest: P::HIGHEST_PRESET,
                recall_axes,
                recall_speed_range: P::PRESET_SPEED_RANGE,
                tour: P::SUPPORTS_PRESET_TOUR,
                thumbnail: P::SUPPORTS_PRESET_THUMBNAIL,
                recall_delay: P::PRESET_RECALL_DELAY,
                names: std::num::NonZeroUsize::new(P::MAX_PRESET_NAME_LENGTH),
            }),
            power: Some(PowerCaps {
                on_time: P::POWER_ON_TIME,
                standby: P::SUPPORTS_STANDBY.then_some(P::STANDBY_TIME),
                wake_on_lan: P::SUPPORTS_WAKE_ON_LAN,
                retains_settings: P::RETAINS_SETTINGS_ON_POWER_OFF,
                home_on_up: P::HOME_ON_POWER_UP,
            }),
            nd_filter_mode: P::ND_MODE,
            tally: P::SUPPORTS_TALLY,
            direct_menu: P::SUPPORTS_DIRECT_CONTROL,
            usb_audio: P::SUPPORTS_USB_AUDIO,
            inquiry_support: P::INQUIRY_SUPPORT,
            typed_support: P::TYPED_SUPPORT,
        }
    }

    /// Returns whether this profile grants an optional typed surface.
    #[must_use]
    pub const fn supports_typed(&self, surface: TypedSupportSurface) -> bool {
        self.typed_support.contains(surface)
    }

    /// Returns whether the full baseline inquiry set is documented.
    #[must_use]
    pub fn has_full_inquiry_support(&self) -> bool {
        self.inquiry_support == InquirySupport::Full
    }

    /// Returns whether pan/tilt facts are present.
    #[must_use]
    pub fn has_pan_tilt(&self) -> bool {
        self.pan_tilt.is_some()
    }

    /// Returns whether zoom facts are present.
    #[must_use]
    pub fn has_zoom(&self) -> bool {
        self.zoom.is_some()
    }

    /// Returns whether focus facts are present.
    #[must_use]
    pub fn has_focus(&self) -> bool {
        self.focus.is_some()
    }

    /// Returns whether exposure facts are present.
    #[must_use]
    pub fn has_exposure(&self) -> bool {
        self.exposure.is_some()
    }

    /// Returns whether white-balance facts are present.
    #[must_use]
    pub fn has_white_balance(&self) -> bool {
        self.white_balance.is_some()
    }

    /// Returns whether image-processing facts are present.
    #[must_use]
    pub fn has_image_processing(&self) -> bool {
        self.image.is_some()
    }

    /// Returns whether preset facts are present.
    #[must_use]
    pub fn has_presets(&self) -> bool {
        self.presets.is_some()
    }

    /// Returns whether power-control facts are present.
    #[must_use]
    pub fn has_power(&self) -> bool {
        self.power.is_some()
    }

    /// Returns whether the declared ND mode supports filtering.
    #[must_use]
    pub fn has_nd_filter(&self) -> bool {
        matches!(self.nd_filter_mode, super::NdFilterMode::Variable)
    }

    /// Returns whether tally control is documented.
    #[must_use]
    pub fn has_tally(&self) -> bool {
        self.tally
    }

    /// Returns whether direct menu control is documented.
    #[must_use]
    pub fn has_direct_menu_control(&self) -> bool {
        self.direct_menu
    }

    /// Returns whether USB audio control is documented.
    #[must_use]
    pub fn has_usb_audio(&self) -> bool {
        self.usb_audio
    }

    /// Returns whether the focus domain supports automatic focus.
    #[must_use]
    pub fn has_auto_focus(&self) -> bool {
        self.focus.as_ref().is_some_and(|domain| domain.auto_focus)
    }

    /// Returns whether the focus domain supports one-push focus.
    #[must_use]
    pub fn has_one_push_focus(&self) -> bool {
        self.focus.as_ref().is_some_and(|domain| domain.one_push)
    }

    /// Returns whether focus-zone readback is documented.
    #[must_use]
    pub fn has_focus_zone_inquiry(&self) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|domain| domain.zone_inquiry)
    }

    /// Returns whether AF sensitivity adjustment is documented.
    #[must_use]
    pub fn has_af_sensitivity(&self) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|domain| domain.af_sensitivity)
    }

    /// Returns whether focus near-limit readback is documented.
    #[must_use]
    pub fn has_focus_near_limit_inquiry(&self) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|domain| domain.near_limit_inquiry)
    }

    /// Returns whether shared backlight compensation is documented.
    #[must_use]
    pub fn has_backlight_comp(&self) -> bool {
        self.exposure
            .as_ref()
            .is_some_and(|domain| domain.backlight)
    }

    /// Returns whether shared dynamic-range control is documented.
    #[must_use]
    pub fn has_wdr(&self) -> bool {
        self.exposure.as_ref().is_some_and(|domain| domain.wdr)
    }

    /// Returns whether one-push white balance is documented.
    #[must_use]
    pub fn has_one_push_wb(&self) -> bool {
        self.white_balance
            .as_ref()
            .is_some_and(|domain| domain.one_push)
    }

    /// Returns whether vertical image flip is documented.
    #[must_use]
    pub fn supports_flip(&self) -> bool {
        self.image.as_ref().is_some_and(|domain| domain.flip)
    }

    /// Returns whether horizontal image mirroring is documented.
    #[must_use]
    pub fn supports_mirror(&self) -> bool {
        self.image.as_ref().is_some_and(|domain| domain.mirror)
    }

    /// Returns whether the picture-effect family is documented.
    #[must_use]
    pub fn has_picture_effect(&self) -> bool {
        self.image
            .as_ref()
            .is_some_and(|domain| domain.picture_effect)
    }

    /// Returns whether direct zoom positioning is documented.
    #[must_use]
    pub fn supports_direct_zoom(&self) -> bool {
        self.zoom.as_ref().is_some_and(|domain| domain.direct)
    }

    /// Returns whether variable-speed zoom drive is documented.
    #[must_use]
    pub fn supports_variable_zoom(&self) -> bool {
        self.zoom.as_ref().is_some_and(|domain| domain.variable)
    }

    /// Returns whether standby timing is present.
    #[must_use]
    pub fn supports_standby(&self) -> bool {
        self.power
            .as_ref()
            .is_some_and(|domain| domain.standby.is_some())
    }

    /// Returns whether a nonzero preset-name limit is present.
    #[must_use]
    pub fn supports_preset_names(&self) -> bool {
        self.presets
            .as_ref()
            .is_some_and(|domain| domain.names.is_some())
    }

    /// Returns whether preset tours are documented.
    #[must_use]
    pub fn supports_preset_tour(&self) -> bool {
        self.presets.as_ref().is_some_and(|domain| domain.tour)
    }

    /// Returns whether preset thumbnails are documented.
    #[must_use]
    pub fn supports_preset_thumbnail(&self) -> bool {
        self.presets.as_ref().is_some_and(|domain| domain.thumbnail)
    }

    /// Returns whether variable pan/tilt speed mode is documented.
    #[must_use]
    pub fn has_variable_speed(&self) -> bool {
        self.pan_tilt
            .as_ref()
            .is_some_and(|domain| domain.variable_speed)
    }

    /// Returns whether the combined flip packet is granted.
    #[must_use]
    pub fn uses_combined_flip_command(&self) -> bool {
        self.supports_typed(TypedSupportSurface::CombinedImageFlip)
    }

    /// Returns whether a 2D noise-reduction inquiry or control is granted.
    #[must_use]
    pub fn has_2d_nr(&self) -> bool {
        self.supports_typed(TypedSupportSurface::NoiseReduction2D)
            || self.supports_typed(TypedSupportSurface::NoiseReduction2DControl)
    }

    /// Returns whether a 3D noise-reduction inquiry or control is granted.
    #[must_use]
    pub fn has_3d_nr(&self) -> bool {
        self.supports_typed(TypedSupportSurface::NoiseReduction3D)
            || self.supports_typed(TypedSupportSurface::NoiseReduction3DControl)
    }

    /// The single source rule relating typed surfaces to runtime facts.
    pub(crate) fn surface_metadata(&self, surface: TypedSupportSurface) -> SurfaceMetadata {
        use SurfaceMetadata::{Dedicated, ParentDomain};
        use TypedSupportSurface as S;
        let exposure = self.exposure.as_ref();
        let focus = self.focus.as_ref();
        let white_balance = self.white_balance.as_ref();
        let image = self.image.as_ref();
        match surface {
            S::DirectZoom => Dedicated(self.supports_direct_zoom()),
            S::DigitalZoomToggle | S::DigitalZoomRange => {
                Dedicated(self.zoom.as_ref().is_some_and(|d| d.digital_max.is_some()))
            }
            S::ExposureMode => Dedicated(exposure.is_some_and(|d| !d.modes.is_empty())),
            S::IrisControl => Dedicated(exposure.is_some_and(|d| d.iris_range.is_some())),
            S::BacklightCompensation => Dedicated(image.is_some() && self.has_backlight_comp()),
            S::WideDynamicRange => Dedicated(self.has_wdr()),
            S::ExposureCompensation => {
                Dedicated(exposure.is_some_and(|d| d.compensation_range.is_some()))
            }
            S::BrightnessControl => {
                Dedicated(exposure.is_some_and(|d| d.brightness_range.is_some()))
            }
            S::OnePushFocus => Dedicated(self.has_one_push_focus()),
            S::FocusZone => Dedicated(focus.is_some_and(|d| !d.zones.is_empty())),
            S::FocusZoneInquiry => Dedicated(self.has_focus_zone_inquiry()),
            S::AutoFocusSensitivity => {
                Dedicated(self.has_auto_focus() && self.has_af_sensitivity())
            }
            S::FocusNearLimitInquiry => Dedicated(self.has_focus_near_limit_inquiry()),
            S::OnePushWhiteBalance => Dedicated(
                white_balance
                    .is_some_and(|d| d.one_push && d.modes.contains(&WhiteBalanceMode::OnePush)),
            ),
            S::AutoTrackingWhiteBalance => {
                Dedicated(white_balance.is_some_and(|d| d.modes.contains(&WhiteBalanceMode::Atw)))
            }
            S::ColorTemperature => Dedicated(white_balance.is_some_and(|d| {
                d.color_temp_range.is_some()
                    && d.modes.contains(&WhiteBalanceMode::ColorTemperature)
            })),
            S::RgbGain => Dedicated(white_balance.is_some_and(|d| d.rgb_gain.is_some())),
            S::RgbTuning => Dedicated(white_balance.is_some_and(|d| d.rgb_tuning.is_some())),
            S::ImageFlip => Dedicated(self.supports_flip()),
            S::ImageMirror => Dedicated(self.supports_mirror()),
            S::CombinedImageFlip => ParentDomain(self.supports_flip() && self.supports_mirror()),
            S::ContrastControl => Dedicated(image.is_some_and(|d| d.contrast_range.is_some())),
            S::SharpnessMode => ParentDomain(image.is_some()),
            S::SharpnessControl => Dedicated(image.is_some_and(|d| d.sharpness_range.is_some())),
            S::SaturationControl => Dedicated(image.is_some_and(|d| d.saturation_range.is_some())),
            S::HueControl => Dedicated(image.is_some_and(|d| d.hue_range.is_some())),
            S::LuminanceControl => Dedicated(image.is_some_and(|d| d.luminance_range.is_some())),
            S::GammaControl => Dedicated(image.is_some_and(|d| d.gamma_range.is_some())),
            S::NoiseReduction2D | S::NoiseReduction2DControl => ParentDomain(image.is_some()),
            S::NoiseReduction3D | S::NoiseReduction3DControl => ParentDomain(image.is_some()),
            S::PictureEffect => Dedicated(self.has_picture_effect()),
            S::Tally => Dedicated(self.tally),
            S::DirectMenu => Dedicated(self.direct_menu),
            S::NdFilter => Dedicated(self.has_nd_filter()),
            S::VariableSpeed => Dedicated(self.has_variable_speed()),
            S::MotionSync => Dedicated(
                self.pan_tilt
                    .as_ref()
                    .is_some_and(|d| d.motion_sync_speed_range.is_some()),
            ),
            S::UsbAudio => Dedicated(self.usb_audio),
            S::PtzOpticsAntiFlicker | S::SonySpotlight | S::SonyAutoSlowShutter => {
                ParentDomain(exposure.is_some())
            }
            S::PtzOpticsSnapFocus | S::FocusLock | S::PushAutoFocus => {
                ParentDomain(focus.is_some())
            }
            S::PtzOpticsPresetRecallSpeed => ParentDomain(
                self.presets
                    .as_ref()
                    .is_some_and(|d| d.recall_speed_range.is_some()),
            ),
            S::AutoWhiteBalanceSensitivity => ParentDomain(white_balance.is_some()),
            S::ImageFreeze => ParentDomain(image.is_some()),
            S::TallyBrightness | S::PtzOpticsTally => {
                ParentDomain(self.tally && self.supports_typed(S::Tally))
            }
            S::NoiseReduction2DMode => ParentDomain(image.is_some() && self.has_2d_nr()),
            S::ColorTemperatureInquiry => {
                ParentDomain(self.surface_metadata(S::ColorTemperature).permits())
            }
            S::SonyFr7AeLevel | S::SonyFr7AutoGain | S::SonyFr7AutoShutter | S::SonyFr7Iris => {
                ParentDomain(exposure.is_some())
            }
            S::SonyFr7WhiteBalance
            | S::SonyFr7PresetWhite
            | S::SonyFr7MemoryColorTemperature
            | S::SonyFr7MemoryGain
            | S::SonyFr7MemoryTint
            | S::SonyFr7AtwOffsets => ParentDomain(white_balance.is_some()),
            S::SonyFr7Black | S::SonyFr7Detail | S::SonyFr7Knee => ParentDomain(image.is_some()),
            S::SonyFr7HighResolutionZoom | S::SonyFr7ClearImageZoom => {
                ParentDomain(self.zoom.is_some())
            }
            S::SonyFr7PresetSpeed | S::SonyFr7PresetDuration => {
                ParentDomain(self.presets.is_some())
            }
            S::PowerStandby => Dedicated(
                self.power
                    .as_ref()
                    .is_some_and(|domain| domain.standby.is_some()),
            ),
            S::FocusInfinity => ParentDomain(focus.is_some()),
            S::PresetMemory => ParentDomain(self.presets.is_some()),
            S::ShutterControl | S::GainControl | S::GainLimit => ParentDomain(exposure.is_some()),
            S::StandardWhiteBalanceModes => ParentDomain(white_balance.is_some()),
            S::ShutterDirect => {
                ParentDomain(exposure.is_some_and(|domain| !domain.shutter_speeds.is_empty()))
            }
            S::PtzOpticsSettingsSave
            | S::PtzOpticsMulticastStreaming
            | S::PtzOpticsNdiQuality
            | S::VersionInquiry
            | S::MenuDisplay
            | S::MenuEnterReturn
            | S::MenuStatusInquiry => ParentDomain(true),
        }
    }

    pub(crate) fn permits_typed(&self, surface: TypedSupportSurface) -> bool {
        self.supports_typed(surface) && self.surface_metadata(surface).permits()
    }

    /// Returns whether all broad baseline domains are present.
    pub fn has_basic_features(&self) -> bool {
        self.pan_tilt.is_some()
            && self.zoom.is_some()
            && self.focus.is_some()
            && self.exposure.is_some()
            && self.white_balance.is_some()
            && self.image.is_some()
            && self.presets.is_some()
            && self.power.is_some()
    }

    /// Returns whether at least one advanced feature is documented.
    pub fn has_advanced_features(&self) -> bool {
        self.zoom.as_ref().is_some_and(|d| d.digital_max.is_some())
            || self.has_one_push_focus()
            || self.has_one_push_wb()
            || self.has_wdr()
            || self
                .white_balance
                .as_ref()
                .is_some_and(|d| d.color_temp_range.is_some())
            || self.supports_preset_tour()
    }

    /// Returns whether a focus-zone value is in the source inventory.
    pub fn supports_focus_zone(&self, zone: crate::command::FocusZone) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|domain| domain.zones.contains(&zone))
    }

    /// Returns whether a shared exposure mode is in the source inventory.
    pub fn supports_exposure_mode(&self, mode: ExposureMode) -> bool {
        self.exposure
            .as_ref()
            .is_some_and(|domain| domain.modes.contains(&mode))
    }

    /// Creates the fixed lens's optical magnification scale.
    pub fn zoom_scale(&self) -> Result<super::ZoomScale, crate::Error> {
        let zoom = self
            .zoom
            .as_ref()
            .ok_or(crate::Error::FeatureNotSupported { feature: "zoom" })?;
        let ratio = zoom.optical_ratio.ok_or_else(|| crate::Error::InvalidRequest(
            format!("{} does not fix its lens's optical zoom ratio; declare the installed lens with `Capabilities::zoom_scale_for_lens(ratio)` or `ProfileSpecBuilder::zoom`", self.model_name).into()))?;
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
        if self.has_nd_filter() {
            lines.push(format!("ND Filter: {:?}", self.nd_filter_mode));
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
    use std::time::Duration;

    use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    use crate::capabilities::{
        exposure::ShutterSpeedEntry, CapabilityDomain, CapabilityRange, Exposure, Focus,
        ImageProcessing, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt, Power,
        Presets, ProfileMetadata, ProfileTypedSupport, Tally, VariableSpeedMetadata, WhiteBalance,
        Zoom,
    };
    use crate::command::exposure::ExposureMode;
    use crate::transport::RawVisca;
    use crate::{ProfileSpec, WhiteBalanceMode};

    use super::*;

    const SYNTHETIC_EXPOSURE_MODES: &[ExposureMode] = &[ExposureMode::Auto];
    const SYNTHETIC_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[ShutterSpeedEntry::new(
        crate::units::ExposureTime::reciprocal(60),
        0x01,
    )];
    const SYNTHETIC_WB_MODES: &[WhiteBalanceMode] = &[WhiteBalanceMode::Auto];

    #[derive(Debug, Default, Clone, Copy)]
    struct MetadataEnabledNoTypedSupport;

    impl ProfileMetadata for MetadataEnabledNoTypedSupport {
        const INQUIRY_SUPPORT: InquirySupport = InquirySupport::Full;
        const SUPPORTS_COMMAND_CANCEL: bool = true;

        const MODEL_NAME: &'static str = "Metadata Enabled Without Typed Support";
        const DEFAULT_CAMERA_ID: crate::IndividualCameraId = crate::IndividualCameraId::CAMERA_1;
        type Envelope = RawVisca;
        const ACK_TIMEOUT: Duration = Duration::from_millis(100);
        const COMMAND_TIMEOUTS: crate::CommandTimeouts = crate::CommandTimeouts::new(
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        );
    }

    impl PanTilt for MetadataEnabledNoTypedSupport {
        const PAN_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-1700, 1700);
        const TILT_RANGES: crate::capabilities::TiltRanges =
            crate::capabilities::TiltRanges::Single(CapabilityRange::<i32>::new(-300, 900));
        const MAX_PAN_SPEED: u8 = 24;
        const MAX_TILT_SPEED: u8 = 20;
        const PAN_DEGREES_TO_UNITS: f32 = 10.0;
        const TILT_DEGREES_TO_UNITS: f32 = 10.0;
    }

    impl Zoom for MetadataEnabledNoTypedSupport {
        const SUPPORTS_VARIABLE_ZOOM: bool = true;

        const OPTICAL_ZOOM_MAX: u16 = 0x4000;
        const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
        const ZOOM_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 7);
        const SUPPORTS_DIRECT_ZOOM: bool = true;
        const OPTICAL_ZOOM_RATIO: Option<f32> = Some(20.0);
    }

    impl Focus for MetadataEnabledNoTypedSupport {
        const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = true;

        const MAX_FOCUS_SPEED: u8 = 7;

        const FOCUS_RANGE: CapabilityRange<u16> = CapabilityRange::<u16>::new(0x1000, 0xF000);
        const FOCUS_NEAR_END: crate::capabilities::FocusNearEnd =
            crate::capabilities::FocusNearEnd::Minimum;
        const SUPPORTS_AUTO_FOCUS: bool = true;
        const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
        const FOCUS_ZONES: &'static [crate::command::FocusZone] =
            crate::capabilities::focus::DOCUMENTED_FOCUS_ZONES;
        const SUPPORTS_AF_SENSITIVITY: bool = true;
    }

    impl Exposure for MetadataEnabledNoTypedSupport {
        const EXPOSURE_MODES: &'static [ExposureMode] = SYNTHETIC_EXPOSURE_MODES;
        const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
        const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = SYNTHETIC_SHUTTER_SPEEDS;
        const GAIN_RANGE: Option<CapabilityRange<u8>> = Some(CapabilityRange::<u8>::new(0, 15));
        const SUPPORTS_BACKLIGHT_COMP: bool = false;
    }

    impl WhiteBalance for MetadataEnabledNoTypedSupport {
        const WB_MODES: &'static [WhiteBalanceMode] = SYNTHETIC_WB_MODES;
        const SUPPORTS_ONE_PUSH_WB: bool = false;
        const RG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
        const BG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
    }

    impl ImageProcessing for MetadataEnabledNoTypedSupport {
        const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
        const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
        const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
        const SUPPORTS_FLIP: bool = false;
        const SUPPORTS_MIRROR: bool = false;
    }

    impl Presets for MetadataEnabledNoTypedSupport {
        const HIGHEST_PRESET: u8 = 6;
        const PRESET_SPEED_RANGE: Option<CapabilityRange<u8>> =
            Some(CapabilityRange::<u8>::new(1, 23));
        const SUPPORTS_PRESET_TOUR: bool = false;
    }

    impl Power for MetadataEnabledNoTypedSupport {
        const POWER_ON_TIME: Duration = Duration::from_secs(5);
        const SUPPORTS_STANDBY: bool = false;
    }

    impl MenuCapability for MetadataEnabledNoTypedSupport {}
    impl Tally for MetadataEnabledNoTypedSupport {}
    impl MotionSyncMetadata for MetadataEnabledNoTypedSupport {}
    impl NdFilterMetadata for MetadataEnabledNoTypedSupport {}
    impl VariableSpeedMetadata for MetadataEnabledNoTypedSupport {}

    impl ProfileTypedSupport for MetadataEnabledNoTypedSupport {
        const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::EMPTY;
    }

    impl crate::CompileTimeProfile for MetadataEnabledNoTypedSupport {
        const TRANSPORTS: crate::TransportCompatibility =
            crate::TransportCompatibility::new(Some(5678), None, false);
        const INQUIRY_TIMEOUT: Duration = Duration::from_secs(1);
        const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(1);
        const AMBIGUITY_TIMEOUT: Duration = Duration::from_secs(1);
        const MAXIMUM_COMMAND_SOCKETS: u8 = 1;
        const PRESET_RECALL_AXES: Option<crate::AffectedAxes> = Some(crate::AffectedAxes::MOVEMENT);
        const POSITION_INQUIRIES: crate::PositionInquirySupport =
            crate::PositionInquirySupport::new(true, true, true);
    }

    #[test]
    fn test_capabilities_from_ptzoptics_g2() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        assert_eq!(
            caps.profile_id,
            Some(crate::profiles::ProfileId::PtzOpticsG2)
        );
        assert_eq!(caps.model_name, "PtzOptics G2");
        assert_eq!(caps.default_camera_id.value(), 1);
        let transports = <PtzOpticsG2 as crate::CompileTimeProfile>::TRANSPORTS;
        assert_eq!(transports.tcp_port(), Some(5678));
        assert_eq!(transports.udp_port(), Some(1259));

        assert!(caps.has_pan_tilt());
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

        assert!(caps.has_zoom());
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

        assert!(caps.has_auto_focus());
        assert!(!caps.has_one_push_focus());
        assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(caps.has_focus_zone_inquiry());
        assert!(!caps.has_af_sensitivity());
        assert!(!caps.has_focus_near_limit_inquiry());
        assert!(caps
            .exposure
            .as_ref()
            .and_then(|domain| domain.iris_range.clone())
            .is_some());
        assert!(caps.supports_exposure_mode(ExposureMode::Iris));
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
        assert!(!caps.supports_preset_tour());
        assert!(!caps.has_nd_filter());
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(!caps.has_variable_speed());
        assert!(caps.has_usb_audio());
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            Some(CapabilityDomain::<u8>::new(0, 17))
        );
        assert!(caps.has_image_processing());
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
        assert!(caps.has_picture_effect());

        assert!(caps.has_basic_features());
    }

    #[test]
    fn test_capabilities_from_sony_fr7() {
        let caps = Capabilities::from_profile::<SonyFR7>();

        assert_eq!(caps.model_name, "Sony FR7");
        let transports = <SonyFR7 as crate::CompileTimeProfile>::TRANSPORTS;
        assert_eq!(transports.tcp_port(), None);
        assert_eq!(transports.udp_port(), Some(52381));

        assert_eq!(caps.presets.as_ref().expect("presets").highest, 99);
        assert!(!caps.supports_preset_tour());
        assert!(!caps.has_one_push_focus());
        assert!(caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(!caps.has_focus_zone_inquiry());
        assert!(!caps.has_af_sensitivity());
        assert!(!caps.has_focus_near_limit_inquiry());
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
        assert!(caps.has_nd_filter());
        assert_eq!(
            caps.nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        );
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(caps.has_variable_speed());
        assert!(!caps.has_usb_audio());
        assert_eq!(
            caps.exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            None
        );
        assert!(caps.has_image_processing());
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
        assert!(!caps.has_picture_effect());

        // The shared advanced refinements (digital range, one-push, WDR,
        // shared Kelvin control and preset tour) were withdrawn for FR7.
        // Its source-backed vendor controls have independent typed grants.
        assert!(!caps.has_advanced_features());
    }

    #[test]
    fn test_typed_support_discovery_for_built_in_profiles() {
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        assert_eq!(
            g2.typed_support,
            <PtzOpticsG2 as ProfileTypedSupport>::TYPED_SUPPORT
        );
        assert!(g2.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(g2.supports_typed(TypedSupportSurface::FocusZone));
        assert!(g2.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(g2.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsAntiFlicker));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsSettingsSave));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsMulticastStreaming));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsNdiQuality));
        assert!(g2.supports_typed(TypedSupportSurface::ExposureMode));
        assert!(!g2.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(!g2.supports_typed(TypedSupportSurface::DigitalZoomRange));

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert_eq!(
            fr7.typed_support,
            <SonyFR7 as ProfileTypedSupport>::TYPED_SUPPORT
        );
        assert!(fr7.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(!fr7.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(!fr7.supports_typed(TypedSupportSurface::DigitalZoomRange));
        assert!(!fr7.supports_typed(TypedSupportSurface::FocusZone));
        assert!(!fr7.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!fr7.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(!fr7.supports_typed(TypedSupportSurface::BrightnessControl));
        assert!(!fr7.supports_typed(TypedSupportSurface::PictureEffect));
        assert!(!fr7.supports_typed(TypedSupportSurface::AutoFocusSensitivity));
        assert!(fr7.supports_typed(TypedSupportSurface::SonySpotlight));
        assert!(!fr7.supports_typed(TypedSupportSurface::SonyAutoSlowShutter));
        assert!(!fr7.supports_typed(TypedSupportSurface::ExposureMode));
    }

    #[test]
    fn iris_control_status_inquiry_is_not_inferred_from_other_iris_facts() {
        let g3 = Capabilities::from_profile::<PtzOpticsG3>();
        assert!(g3
            .exposure
            .as_ref()
            .and_then(|domain| domain.iris_range.clone())
            .is_some());
        assert!(g3.supports_typed(TypedSupportSurface::IrisControl));
    }

    #[test]
    fn shared_exposure_mode_support_exactly_matches_discovery_inventory() {
        for caps in [
            Capabilities::from_profile::<PtzOpticsG2>(),
            Capabilities::from_profile::<PtzOpticsG3>(),
            Capabilities::from_profile::<PtzOptics30X>(),
            Capabilities::from_profile::<SonyBRCH900>(),
            Capabilities::from_profile::<SonyEVIH100>(),
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
            Capabilities::from_profile::<GenericVisca>(),
        ] {
            assert!(!caps.exposure.as_ref().expect("exposure").modes.is_empty());
            assert!(caps.supports_typed(TypedSupportSurface::ExposureMode));
        }

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert!(fr7.exposure.as_ref().expect("exposure").modes.is_empty());
        assert!(!fr7.supports_typed(TypedSupportSurface::ExposureMode));
    }

    #[test]
    fn sony_vendor_exposure_typed_support_matches_the_model_command_lists() {
        let expected = [
            (
                "Sony FR7",
                Capabilities::from_profile::<SonyFR7>(),
                true,
                false,
            ),
            (
                "Sony BRC-H900",
                Capabilities::from_profile::<SonyBRCH900>(),
                false,
                false,
            ),
            (
                "Sony EVI-H100",
                Capabilities::from_profile::<SonyEVIH100>(),
                false,
                true,
            ),
            (
                "Sony BRC-300",
                Capabilities::from_profile::<SonyBRC300>(),
                false,
                true,
            ),
            (
                "Nearus BRC-300",
                Capabilities::from_profile::<NearusBRC300>(),
                false,
                true,
            ),
        ];

        for (model, caps, spotlight, auto_slow_shutter) in expected {
            assert_eq!(
                caps.supports_typed(TypedSupportSurface::SonySpotlight),
                spotlight,
                "{model} spotlight support"
            );
            assert_eq!(
                caps.supports_typed(TypedSupportSurface::SonyAutoSlowShutter),
                auto_slow_shutter,
                "{model} auto slow-shutter support"
            );
        }
    }

    #[test]
    fn test_metadata_enabled_profile_reports_typed_support_independently() {
        let caps = Capabilities::from_profile::<MetadataEnabledNoTypedSupport>();

        assert!(caps.supports_direct_zoom());
        assert!(caps
            .zoom
            .as_ref()
            .and_then(|domain| domain.digital_range())
            .map(|range| range.as_inclusive())
            .is_some());
        assert!(caps.has_one_push_focus());
        assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
        assert!(!caps.has_focus_zone_inquiry());
        assert!(caps.has_af_sensitivity());
        assert!(!caps.has_usb_audio());

        assert!(caps.typed_support.is_empty());
        assert!(!caps.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(!caps.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(!caps.supports_typed(TypedSupportSurface::DigitalZoomRange));
        assert!(!caps.supports_typed(TypedSupportSurface::OnePushFocus));
        assert!(!caps.supports_typed(TypedSupportSurface::FocusZone));
        assert!(!caps.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!caps.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(!caps.supports_typed(TypedSupportSurface::AutoFocusSensitivity));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsAntiFlicker));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsSettingsSave));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed));
        assert!(!caps.supports_typed(TypedSupportSurface::SonySpotlight));
        assert!(!caps.supports_typed(TypedSupportSurface::SonyAutoSlowShutter));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsMulticastStreaming));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsNdiQuality));
    }

    #[test]
    fn source_backed_focus_zone_inquiry_and_usb_audio_boundaries_match_profiles() {
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        let g3 = Capabilities::from_profile::<PtzOpticsG3>();
        let thirty_x = Capabilities::from_profile::<PtzOptics30X>();

        for caps in [&g2, &g3, &thirty_x] {
            assert!(!caps.focus.as_ref().expect("focus").zones.is_empty());
            assert!(caps.supports_typed(TypedSupportSurface::FocusZone));
            assert!(caps.has_picture_effect());
            assert!(caps.supports_typed(TypedSupportSurface::PictureEffect));
        }

        for caps in [&g2, &thirty_x] {
            assert!(caps.has_focus_zone_inquiry());
            assert!(caps.supports_typed(TypedSupportSurface::FocusZoneInquiry));
            assert!(caps.has_usb_audio());
            assert!(caps.supports_typed(TypedSupportSurface::UsbAudio));
        }

        assert!(!g3.has_focus_zone_inquiry());
        assert!(!g3.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!g3.has_usb_audio());
        assert!(!g3.supports_typed(TypedSupportSurface::UsbAudio));
    }

    #[test]
    fn test_color_temperature_profile_metadata_matches_sources() {
        for caps in [
            Capabilities::from_profile::<PtzOpticsG2>(),
            Capabilities::from_profile::<PtzOpticsG3>(),
            Capabilities::from_profile::<PtzOptics30X>(),
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
            Capabilities::from_profile::<SonyEVIH100>(),
            Capabilities::from_profile::<SonyBRCH900>(),
            Capabilities::from_profile::<SonyFR7>(),
        ] {
            let white = caps.white_balance.as_ref().expect("white balance");
            assert_eq!(white.color_temp_range, None, "{}", caps.model_name);
            assert!(
                !white.modes.contains(&WhiteBalanceMode::ColorTemperature),
                "{}",
                caps.model_name
            );
            assert!(
                !caps.supports_typed(TypedSupportSurface::ColorTemperature),
                "{}",
                caps.model_name
            );
        }
    }

    #[test]
    fn test_capabilities_from_generic() {
        let caps = Capabilities::from_profile::<GenericVisca>();

        assert_eq!(caps.model_name, "Generic VISCA Camera");
        assert!(caps
            .zoom
            .as_ref()
            .and_then(|domain| domain.digital_range())
            .map(|range| range.as_inclusive())
            .is_none());
        assert!(!caps.supports_direct_zoom());
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
        assert!(caps.supports_exposure_mode(ExposureMode::Iris));

        assert!(!caps.has_image_processing());
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
        assert!(!caps.has_basic_features());
        // GenericVisca has one-push white balance, which counts as an advanced feature
        assert!(caps.has_one_push_wb());
        assert!(!caps.has_nd_filter());
        assert_eq!(
            caps.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(!caps.has_variable_speed());
        assert!(caps.has_advanced_features());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn brc300_static_profile_preserves_documented_preset_metadata() {
        let profile =
            ProfileSpec::from_compile_time::<SonyBRC300>().expect("Sony BRC-300 static profile");
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
        let caps = Capabilities::from_profile::<SonyBRC300>();

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
            <SonyBRC300 as ProfileTypedSupport>::TYPED_SUPPORT,
            "discovery must mirror the static typed-support registry"
        );
        assert!(
            !caps.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed),
            "BRC-300 metadata must not grant the distinct PTZOptics typed command"
        );
    }

    #[test]
    fn brc300_capabilities_keep_reverse_axis_degree_ranges_ordered() {
        let caps = Capabilities::from_profile::<SonyBRC300>();

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
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
            Capabilities::from_profile::<GenericVisca>(),
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
        let evi = Capabilities::from_profile::<SonyEVIH100>();
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
            Capabilities::from_profile::<SonyEVIH100>()
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
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
        ] {
            assert_eq!(
                caps.exposure
                    .as_ref()
                    .and_then(|domain| domain.brightness_range.clone()),
                Some(CapabilityDomain::<u8>::new(0x00, 0x17))
            );
        }
        assert_eq!(
            Capabilities::from_profile::<GenericVisca>()
                .exposure
                .as_ref()
                .and_then(|domain| domain.brightness_range.clone()),
            None
        );

        assert!(Capabilities::from_profile::<SonyEVIH100>().has_image_processing());
        assert!(Capabilities::from_profile::<SonyBRC300>().has_image_processing());
    }

    #[test]
    fn test_optional_vendor_feature_metadata_matrix() {
        for caps in [
            Capabilities::from_profile::<PtzOpticsG2>(),
            Capabilities::from_profile::<PtzOpticsG3>(),
            Capabilities::from_profile::<PtzOptics30X>(),
        ] {
            assert!(!caps.has_one_push_focus());
            assert!(!caps.has_nd_filter());
            assert_eq!(caps.nd_filter_mode, crate::capabilities::NdFilterMode::None);
            assert_eq!(
                caps.pan_tilt
                    .as_ref()
                    .and_then(|domain| domain.motion_sync_speed_range)
                    .map(|range| range.as_inclusive()),
                None
            );
            assert!(!caps.has_variable_speed());
        }

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert!(fr7.has_nd_filter());
        assert_eq!(
            fr7.nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        );
        assert!(!fr7.has_one_push_focus());
        assert_eq!(
            fr7.pan_tilt
                .as_ref()
                .and_then(|domain| domain.motion_sync_speed_range)
                .map(|range| range.as_inclusive()),
            None
        );
        assert!(fr7.has_variable_speed());

        for caps in [
            Capabilities::from_profile::<GenericVisca>(),
            Capabilities::from_profile::<SonyBRCH900>(),
            Capabilities::from_profile::<SonyEVIH100>(),
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
        ] {
            // R8, R12 and R21 document the One Push AF trigger; the BRC-H900
            // source does not.
            assert_eq!(
                caps.has_one_push_focus(),
                caps.model_name != "Sony BRC-H900",
                "{}",
                caps.model_name
            );
            assert!(!caps.has_nd_filter());
            assert_eq!(caps.nd_filter_mode, crate::capabilities::NdFilterMode::None);
            assert_eq!(
                caps.pan_tilt
                    .as_ref()
                    .and_then(|domain| domain.motion_sync_speed_range)
                    .map(|range| range.as_inclusive()),
                None
            );
            assert!(!caps.has_variable_speed());
        }
    }

    #[test]
    fn test_capabilities_summary() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();
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
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        let undeclared = g2.zoom_scale().expect_err("G2 lens is not a profile fact");
        assert!(
            matches!(&undeclared, crate::Error::InvalidRequest(message)
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

        let pt30x = Capabilities::from_profile::<PtzOptics30X>();
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
