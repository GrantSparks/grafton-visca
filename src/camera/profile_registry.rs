//! Crate-private built-in profile registry.
//!
//! This module is the source of truth for built-in profile facts. The macro at
//! the bottom expands those facts into the public zero-sized profile types,
//! profile ID/group methods, runtime metadata impls, typed support markers, and
//! invariant tests.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvelopeKind {
    RawVisca,
    SonyEncapsulated,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NetworkTransportFacts {
    pub(crate) default_port: u16,
    pub(crate) envelope: EnvelopeKind,
    pub(crate) evidence: Option<&'static str>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SerialTransportFacts {
    pub(crate) envelope: EnvelopeKind,
    pub(crate) evidence: Option<&'static str>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProfileEvidence {
    pub(crate) key: &'static str,
    pub(crate) rationale: &'static str,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinProfileFacts {
    pub(crate) id: super::ProfileId,
    pub(crate) group: super::ProfileGroup,
    pub(crate) type_name: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) vendor: &'static str,
    pub(crate) description: &'static str,
    pub(crate) envelope: EnvelopeKind,
    pub(crate) tcp: Option<NetworkTransportFacts>,
    pub(crate) udp: Option<NetworkTransportFacts>,
    pub(crate) serial: Option<SerialTransportFacts>,
    pub(crate) default_camera_id: u8,
    pub(crate) inquiry_support: crate::capabilities::InquirySupport,
    pub(crate) typed_support: crate::capabilities::TypedSupportSet,
    pub(crate) evidence: &'static [ProfileEvidence],
}

#[cfg(test)]
impl BuiltinProfileFacts {
    pub(crate) fn has_typed_support(
        self,
        surface: crate::capabilities::TypedSupportSurface,
    ) -> bool {
        self.typed_support.contains(surface)
    }
}

macro_rules! __transport_is_supported {
    (none) => {
        false
    };
    ({ $($fields:tt)* }) => {
        true
    };
}

macro_rules! __transport_default_port {
    (none) => {
        None
    };
    ({
        default_port: $default_port:expr,
        envelope_kind: $envelope_kind:ident
        $(, evidence: $evidence:literal)?
        $(,)?
    }) => {
        Some($default_port)
    };
}

macro_rules! __transport_evidence {
    () => {
        None
    };
    ($evidence:literal) => {
        Some($evidence)
    };
}

macro_rules! __network_transport_facts {
    (none) => {
        None
    };
    ({
        default_port: $default_port:expr,
        envelope_kind: $envelope_kind:ident
        $(, evidence: $evidence:literal)?
        $(,)?
    }) => {
        Some(profile_registry::NetworkTransportFacts {
            default_port: $default_port,
            envelope: profile_registry::EnvelopeKind::$envelope_kind,
            evidence: __transport_evidence!($($evidence)?),
        })
    };
}

macro_rules! __serial_transport_facts {
    (none) => {
        None
    };
    ({
        envelope_kind: $envelope_kind:ident
        $(, evidence: $evidence:literal)?
        $(,)?
    }) => {
        Some(profile_registry::SerialTransportFacts {
            envelope: profile_registry::EnvelopeKind::$envelope_kind,
            evidence: __transport_evidence!($($evidence)?),
        })
    };
}

macro_rules! __impl_supports_tcp {
    ($profile:ty, none) => {};
    (
        $profile:ty,
        {
            default_port: $default_port:expr,
            envelope_kind: $envelope_kind:ident
            $(, evidence: $evidence:literal)?
            $(,)?
        }
    ) => {
        impl $crate::capabilities::SupportsTcp for $profile {
            const DEFAULT_TCP_PORT: u16 = $default_port;
        }
    };
}

macro_rules! __impl_supports_udp {
    ($profile:ty, none) => {};
    (
        $profile:ty,
        {
            default_port: $default_port:expr,
            envelope_kind: $envelope_kind:ident
            $(, evidence: $evidence:literal)?
            $(,)?
        }
    ) => {
        impl $crate::capabilities::SupportsUdp for $profile {
            const DEFAULT_UDP_PORT: u16 = $default_port;
        }
    };
}

macro_rules! __impl_supports_serial {
    ($profile:ty, none) => {};
    (
        $profile:ty,
        {
            envelope_kind: $envelope_kind:ident
            $(, evidence: $evidence:literal)?
            $(,)?
        }
    ) => {
        impl $crate::capabilities::SupportsSerial for $profile {}
    };
}

macro_rules! __impl_typed_support_marker {
    (DirectZoom for $profile:ty) => {
        impl $crate::capabilities::HasDirectZoom for $profile {}
    };
    (DigitalZoomToggle for $profile:ty) => {
        impl $crate::capabilities::HasDigitalZoomToggle for $profile {}
    };
    (DigitalZoomRange for $profile:ty) => {
        impl $crate::capabilities::HasDigitalZoomRange for $profile {}
    };
    (IrisControl for $profile:ty) => {
        impl $crate::capabilities::HasIrisControl for $profile {}
    };
    (OnePushFocus for $profile:ty) => {
        impl $crate::capabilities::HasOnePushFocus for $profile {}
    };
    (PtzOpticsSnapFocus for $profile:ty) => {
        impl $crate::capabilities::HasPtzOpticsSnapFocus for $profile {}
    };
    (FocusLock for $profile:ty) => {
        impl $crate::capabilities::HasFocusLock for $profile {}
    };
    (PushAutoFocus for $profile:ty) => {
        impl $crate::capabilities::HasPushAutoFocus for $profile {}
    };
    (FocusZone for $profile:ty) => {
        impl $crate::capabilities::HasFocusZone for $profile {}
    };
    (AutoFocusSensitivity for $profile:ty) => {
        impl $crate::capabilities::HasAutoFocusSensitivity for $profile {}
    };
    (FocusNearLimitInquiry for $profile:ty) => {
        impl $crate::capabilities::HasFocusNearLimitInquiry for $profile {}
    };
    (BacklightCompensation for $profile:ty) => {
        impl $crate::capabilities::HasBacklightCompensation for $profile {}
    };
    (WideDynamicRange for $profile:ty) => {
        impl $crate::capabilities::HasWideDynamicRange for $profile {}
    };
    (ExposureCompensation for $profile:ty) => {
        impl $crate::capabilities::HasExposureCompensation for $profile {}
    };
    (BrightnessControl for $profile:ty) => {
        impl $crate::capabilities::HasBrightnessControl for $profile {}
    };
    (OnePushWhiteBalance for $profile:ty) => {
        impl $crate::capabilities::HasOnePushWhiteBalance for $profile {}
    };
    (AutoTrackingWhiteBalance for $profile:ty) => {
        impl $crate::capabilities::HasAutoTrackingWhiteBalance for $profile {}
    };
    (AutoWhiteBalanceSensitivity for $profile:ty) => {
        impl $crate::capabilities::HasAutoWhiteBalanceSensitivity for $profile {}
    };
    (ColorTemperature for $profile:ty) => {
        impl $crate::capabilities::HasColorTemperature for $profile {}
    };
    (RgbGain for $profile:ty) => {
        impl $crate::capabilities::HasRgbGain for $profile {}
    };
    (RgbTuning for $profile:ty) => {
        impl $crate::capabilities::HasRgbTuning for $profile {}
    };
    (ImageFlip for $profile:ty) => {
        impl $crate::capabilities::HasImageFlip for $profile {}
    };
    (ImageMirror for $profile:ty) => {
        impl $crate::capabilities::HasImageMirror for $profile {}
    };
    (CombinedImageFlip for $profile:ty) => {
        impl $crate::capabilities::HasCombinedImageFlip for $profile {}
    };
    (ContrastControl for $profile:ty) => {
        impl $crate::capabilities::HasContrastControl for $profile {}
    };
    (SharpnessControl for $profile:ty) => {
        impl $crate::capabilities::HasSharpnessControl for $profile {}
    };
    (SaturationControl for $profile:ty) => {
        impl $crate::capabilities::HasSaturationControl for $profile {}
    };
    (HueControl for $profile:ty) => {
        impl $crate::capabilities::HasHueControl for $profile {}
    };
    (LuminanceControl for $profile:ty) => {
        impl $crate::capabilities::HasLuminanceControl for $profile {}
    };
    (GammaControl for $profile:ty) => {
        impl $crate::capabilities::HasGammaControl for $profile {}
    };
    (NoiseReduction for $profile:ty) => {
        impl $crate::capabilities::HasNoiseReduction for $profile {}
    };
    (NoiseReduction2D for $profile:ty) => {
        impl $crate::capabilities::HasNoiseReduction2D for $profile {}
    };
    (NoiseReduction3D for $profile:ty) => {
        impl $crate::capabilities::HasNoiseReduction3D for $profile {}
    };
    (PictureEffect for $profile:ty) => {
        impl $crate::capabilities::HasPictureEffect for $profile {}
    };
    (Tally for $profile:ty) => {
        impl $crate::capabilities::HasTally for $profile {}
    };
    (DirectMenu for $profile:ty) => {
        impl $crate::capabilities::menu_control::HasDirectMenuControl for $profile {}
    };
    (NdFilter for $profile:ty) => {
        impl $crate::capabilities::HasNdFilter for $profile {}
    };
    (VariableSpeed for $profile:ty) => {
        impl $crate::capabilities::HasVariableSpeed for $profile {}
    };
    (MotionSync for $profile:ty) => {
        impl $crate::capabilities::HasMotionSync for $profile {}
    };
}

macro_rules! __define_builtin_profiles {
    (
        groups {
            $(
                group $group:ident {
                    doc: $group_doc:literal,
                    display: $group_display:literal,
                    inquiry_support: $group_inquiry:expr,
                    uses_sony_encapsulation: $group_sony:expr,
                    transport: {
                        tcp: $group_tcp:expr,
                        udp: $group_udp:expr,
                        serial: $group_serial:expr $(,)?
                    },
                    profiles: [$( $group_profile:ident ),* $(,)?],
                }
            )*
        }
        profiles {
            $(
                profile $profile:ident {
                    doc: $profile_doc:literal,
                    id: $id:ident,
                    id_doc: $id_doc:literal,
                    id_attrs: [$(#[$id_attr:meta])*],
                    group: $profile_group:ident,
                    vendor: $vendor:literal,
                    description: $description:literal,
                    envelope: $envelope:ty,
                    envelope_kind: $envelope_kind:ident,
                    transport: {
                        tcp: $tcp:tt,
                        udp: $udp:tt,
                        serial: $serial:tt $(,)?
                    },
                    metadata: {
                        model_name: $model_name:literal,
                        default_camera_id: $default_camera_id:expr,
                        ack_timeout_ms: $ack_timeout_ms:expr,
                        completion_timeout_ms: $completion_timeout_ms:expr,
                        busy_timeout_ms: $busy_timeout_ms:expr,
                        inquiry_support: $inquiry_support:expr,
                        supports_operation_complete: $supports_operation_complete:expr,
                        min_inquiry_spacing_ms: $min_inquiry_spacing_ms:expr,
                        min_command_spacing_ms: $min_command_spacing_ms:expr,
                    },
                    pan_tilt: {
                        pan_range: $pan_range:expr,
                        tilt_range: $tilt_range:expr,
                        max_pan_speed: $max_pan_speed:expr,
                        max_tilt_speed: $max_tilt_speed:expr,
                        simultaneous: $pan_tilt_simultaneous:expr,
                        preset_recovery_ms: $preset_recovery_ms:expr,
                        pan_degrees_to_units: $pan_degrees_to_units:expr,
                        tilt_degrees_to_units: $tilt_degrees_to_units:expr,
                        coordinate_system: $coordinate_system:expr,
                    },
                    zoom: {
                        optical_max: $optical_zoom_max:expr,
                        digital_max: $digital_zoom_max:expr,
                        speed_range: $zoom_speed_range:expr,
                        supports_direct: $supports_direct_zoom:expr,
                        supports_variable: $supports_variable_zoom:expr,
                        magnification_to_units: $zoom_magnification_to_units:expr,
                    },
                    focus: {
                        near_limit: $focus_near_limit:expr,
                        far_limit: $focus_far_limit:expr,
                        auto_focus: $supports_auto_focus:expr,
                        one_push: $supports_one_push_focus:expr,
                        focus_zone: $supports_focus_zone:expr,
                        max_speed: $max_focus_speed:expr,
                        af_sensitivity: $supports_af_sensitivity:expr,
                        near_limit_inquiry: $supports_focus_near_limit_inquiry:expr,
                    },
                    exposure: {
                        modes: $exposure_modes:expr,
                        iris_range: $iris_range:expr,
                        shutter_speeds: $shutter_speeds:expr,
                        gain_range: $gain_range:expr,
                        brightness_range: $brightness_range:expr,
                        backlight_comp: $supports_backlight_comp:expr,
                        exposure_comp: $supports_exposure_comp:expr,
                        exposure_comp_range: $exposure_comp_range:expr,
                        wdr: $supports_wdr:expr,
                    },
                    white_balance: {
                        modes: $wb_modes:expr,
                        one_push: $supports_one_push_wb:expr,
                        rg_tuning_range: $rg_tuning_range:expr,
                        bg_tuning_range: $bg_tuning_range:expr,
                        color_temp: $supports_color_temp:expr,
                        color_temp_range: $color_temp_range:expr,
                        rgb_gain: $supports_rgb_gain:expr,
                        red_gain_range: $red_gain_range:expr,
                        blue_gain_range: $blue_gain_range:expr,
                    },
                    image: {
                        contrast_range: $contrast_range:expr,
                        sharpness_range: $sharpness_range:expr,
                        saturation_range: $saturation_range:expr,
                        flip: $supports_flip:expr,
                        mirror: $supports_mirror:expr,
                        hue: $supports_hue:expr,
                        hue_range: $hue_range:expr,
                        noise_reduction: $supports_noise_reduction:expr,
                        nr_2d: $supports_2d_nr:expr,
                        nr_3d: $supports_3d_nr:expr,
                        luminance: $supports_luminance:expr,
                        picture_effect: $supports_picture_effect:expr,
                        luminance_range: $luminance_range:expr,
                        combined_flip: $uses_combined_flip_command:expr,
                        save_after_flip: $requires_settings_save_for_flip:expr,
                        gamma: $supports_gamma:expr,
                        gamma_range: $gamma_range:expr,
                    },
                    presets: {
                        max: $max_presets:expr,
                        speed_range: $preset_speed_range:expr,
                        tour: $supports_preset_tour:expr,
                        recall_delay_ms: $preset_recall_delay_ms:expr,
                        thumbnail: $supports_preset_thumbnail:expr,
                        names: $supports_preset_names:expr,
                        max_name_len: $max_preset_name_length:expr,
                    },
                    power: {
                        on_secs: $power_on_secs:expr,
                        standby: $supports_standby:expr,
                        standby_secs: $standby_secs:expr,
                        wake_on_lan: $supports_wake_on_lan:expr,
                        retains_settings: $retains_settings_on_power_off:expr,
                        home_on_power_up: $home_on_power_up:expr,
                    },
                    menu: {
                        direct: $supports_direct_menu:expr $(,)?
                    },
                    tally: {
                        supported: $supports_tally:expr $(,)?
                    },
                    motion_sync: {
                        supported: $supports_motion_sync:expr,
                        max_speed: $max_motion_sync_speed:expr $(,)?
                    },
                    nd_filter: {
                        mode: $nd_mode:expr,
                        steps: $nd_steps:expr $(,)?
                    },
                    variable_speed: {
                        supported: $supports_variable_speed:expr $(,)?
                    },
                    typed_support: [$( $support:ident ),* $(,)?],
                    evidence: [$(($evidence_key:literal, $evidence_text:literal)),* $(,)?],
                }
            )*
        }
    ) => {
        $(
            #[doc = $profile_doc]
            #[derive(Debug, Default, Clone, Copy)]
            pub struct $profile;

            impl $crate::capabilities::ProfileMetadata for $profile {
                const PROFILE_ID: Option<$crate::camera::profiles::ProfileId> =
                    Some($crate::camera::profiles::ProfileId::$id);
                const MODEL_NAME: &'static str = $model_name;
                const DEFAULT_CAMERA_ID: u8 = $default_camera_id;
                type Envelope = $envelope;
                const ACK_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($ack_timeout_ms);
                const COMPLETION_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($completion_timeout_ms);
                const BUSY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($busy_timeout_ms);
                const INQUIRY_SUPPORT: $crate::capabilities::InquirySupport = $inquiry_support;
                const SUPPORTS_OPERATION_COMPLETE: bool = $supports_operation_complete;
                const MIN_INQUIRY_SPACING: std::time::Duration =
                    std::time::Duration::from_millis($min_inquiry_spacing_ms);
                const MIN_COMMAND_SPACING: std::time::Duration =
                    std::time::Duration::from_millis($min_command_spacing_ms);
            }

            impl $crate::capabilities::PanTilt for $profile {
                const PAN_RANGE: std::ops::Range<i16> = $pan_range;
                const TILT_RANGE: std::ops::Range<i16> = $tilt_range;
                const MAX_PAN_SPEED: u8 = $max_pan_speed;
                const MAX_TILT_SPEED: u8 = $max_tilt_speed;
                const PAN_TILT_SIMULTANEOUS: bool = $pan_tilt_simultaneous;
                const PRESET_RECOVERY_TIME: std::time::Duration =
                    std::time::Duration::from_millis($preset_recovery_ms);
                const PAN_DEGREES_TO_UNITS: f32 = $pan_degrees_to_units;
                const TILT_DEGREES_TO_UNITS: f32 = $tilt_degrees_to_units;
                const COORDINATE_SYSTEM: $crate::capabilities::CoordinateSystem = $coordinate_system;
            }

            impl $crate::capabilities::Zoom for $profile {
                const OPTICAL_ZOOM_MAX: u16 = $optical_zoom_max;
                const DIGITAL_ZOOM_MAX: Option<u16> = $digital_zoom_max;
                const ZOOM_SPEED_RANGE: std::ops::Range<u8> = $zoom_speed_range;
                const SUPPORTS_DIRECT_ZOOM: bool = $supports_direct_zoom;
                const SUPPORTS_VARIABLE_ZOOM: bool = $supports_variable_zoom;
                const ZOOM_MAGNIFICATION_TO_UNITS: f32 = $zoom_magnification_to_units;
            }

            impl $crate::capabilities::Focus for $profile {
                const FOCUS_NEAR_LIMIT: u16 = $focus_near_limit;
                const FOCUS_FAR_LIMIT: u16 = $focus_far_limit;
                const SUPPORTS_AUTO_FOCUS: bool = $supports_auto_focus;
                const SUPPORTS_ONE_PUSH_FOCUS: bool = $supports_one_push_focus;
                const SUPPORTS_FOCUS_ZONE: bool = $supports_focus_zone;
                const MAX_FOCUS_SPEED: u8 = $max_focus_speed;
                const SUPPORTS_AF_SENSITIVITY: bool = $supports_af_sensitivity;
                const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool =
                    $supports_focus_near_limit_inquiry;
            }

            impl $crate::capabilities::Exposure for $profile {
                const EXPOSURE_MODES: &'static [$crate::command::exposure::ExposureMode] =
                    $exposure_modes;
                const IRIS_RANGE: Option<std::ops::Range<u16>> = $iris_range;
                const SHUTTER_SPEEDS: &'static [$crate::capabilities::ShutterSpeed] =
                    $shutter_speeds;
                const GAIN_RANGE: std::ops::Range<u8> = $gain_range;
                const BRIGHTNESS_RANGE: Option<std::ops::Range<u16>> = $brightness_range;
                const SUPPORTS_BACKLIGHT_COMP: bool = $supports_backlight_comp;
                const SUPPORTS_EXPOSURE_COMP: bool = $supports_exposure_comp;
                const EXPOSURE_COMP_RANGE: std::ops::Range<i8> = $exposure_comp_range;
                const SUPPORTS_WDR: bool = $supports_wdr;
            }

            impl $crate::capabilities::WhiteBalance for $profile {
                const WB_MODES: &'static [$crate::WhiteBalanceMode] = $wb_modes;
                const SUPPORTS_ONE_PUSH_WB: bool = $supports_one_push_wb;
                const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = $rg_tuning_range;
                const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = $bg_tuning_range;
                const SUPPORTS_COLOR_TEMP: bool = $supports_color_temp;
                const COLOR_TEMP_RANGE: Option<std::ops::Range<u16>> = $color_temp_range;
                const SUPPORTS_RGB_GAIN: bool = $supports_rgb_gain;
                const RED_GAIN_RANGE: Option<std::ops::Range<u8>> = $red_gain_range;
                const BLUE_GAIN_RANGE: Option<std::ops::Range<u8>> = $blue_gain_range;
            }

            impl $crate::capabilities::ImageProcessing for $profile {
                const CONTRAST_RANGE: Option<std::ops::Range<u8>> = $contrast_range;
                const SHARPNESS_RANGE: Option<std::ops::Range<u8>> = $sharpness_range;
                const SATURATION_RANGE: Option<std::ops::Range<u8>> = $saturation_range;
                const SUPPORTS_FLIP: bool = $supports_flip;
                const SUPPORTS_MIRROR: bool = $supports_mirror;
                const SUPPORTS_HUE: bool = $supports_hue;
                const HUE_RANGE: Option<std::ops::Range<u8>> = $hue_range;
                const SUPPORTS_NOISE_REDUCTION: bool = $supports_noise_reduction;
                const SUPPORTS_2D_NR: bool = $supports_2d_nr;
                const SUPPORTS_3D_NR: bool = $supports_3d_nr;
                const SUPPORTS_LUMINANCE: bool = $supports_luminance;
                const SUPPORTS_PICTURE_EFFECT: bool = $supports_picture_effect;
                const LUMINANCE_RANGE: Option<std::ops::Range<u8>> = $luminance_range;
                const USES_COMBINED_FLIP_COMMAND: bool = $uses_combined_flip_command;
                const REQUIRES_SETTINGS_SAVE_FOR_FLIP: bool = $requires_settings_save_for_flip;
                const SUPPORTS_GAMMA: bool = $supports_gamma;
                const GAMMA_RANGE: Option<std::ops::Range<u8>> = $gamma_range;
            }

            impl $crate::capabilities::Presets for $profile {
                const MAX_PRESETS: u8 = $max_presets;
                const PRESET_SPEED_RANGE: std::ops::Range<u8> = $preset_speed_range;
                const SUPPORTS_PRESET_TOUR: bool = $supports_preset_tour;
                const PRESET_RECALL_DELAY: std::time::Duration =
                    std::time::Duration::from_millis($preset_recall_delay_ms);
                const SUPPORTS_PRESET_THUMBNAIL: bool = $supports_preset_thumbnail;
                const SUPPORTS_PRESET_NAMES: bool = $supports_preset_names;
                const MAX_PRESET_NAME_LENGTH: usize = $max_preset_name_length;
            }

            impl $crate::capabilities::Power for $profile {
                const POWER_ON_TIME: std::time::Duration =
                    std::time::Duration::from_secs($power_on_secs);
                const SUPPORTS_STANDBY: bool = $supports_standby;
                const STANDBY_TIME: std::time::Duration =
                    std::time::Duration::from_secs($standby_secs);
                const SUPPORTS_WAKE_ON_LAN: bool = $supports_wake_on_lan;
                const RETAINS_SETTINGS_ON_POWER_OFF: bool = $retains_settings_on_power_off;
                const HOME_ON_POWER_UP: bool = $home_on_power_up;
            }

            impl $crate::capabilities::MenuCapability for $profile {
                const SUPPORTS_DIRECT_CONTROL: bool = $supports_direct_menu;
            }

            impl $crate::capabilities::Tally for $profile {
                const SUPPORTS_TALLY: bool = $supports_tally;
            }

            impl $crate::capabilities::MotionSyncMetadata for $profile {
                const SUPPORTS_MOTION_SYNC: bool = $supports_motion_sync;
                const MAX_MOTION_SYNC_SPEED: u8 = $max_motion_sync_speed;
            }

            impl $crate::capabilities::NdFilterMetadata for $profile {
                const ND_MODE: $crate::capabilities::NdFilterMode = $nd_mode;
                const ND_STEPS: Option<u8> = $nd_steps;
            }

            impl $crate::capabilities::VariableSpeedMetadata for $profile {
                const SUPPORTS_VARIABLE_SPEED: bool = $supports_variable_speed;
            }

            impl $crate::capabilities::ProfileTypedSupport for $profile {
                const TYPED_SUPPORT: $crate::capabilities::TypedSupportSet =
                    $crate::capabilities::TypedSupportSet::from_surfaces(&[
                        $($crate::capabilities::TypedSupportSurface::$support,)*
                    ]);
            }

            __impl_supports_tcp!($profile, $tcp);
            __impl_supports_udp!($profile, $udp);
            __impl_supports_serial!($profile, $serial);
            $(__impl_typed_support_marker!($support for $profile);)*
        )*

        /// Profile group for runtime polymorphism and profile dispatch.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
        #[non_exhaustive]
        pub enum ProfileGroup {
            $(
                #[doc = $group_doc]
                $group,
            )*
        }

        /// Serializable camera profile identifier.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
        #[non_exhaustive]
        pub enum ProfileId {
            $(
                #[doc = $id_doc]
                $(#[$id_attr])*
                $id,
            )*
        }

        #[cfg(test)]
        pub(crate) const BUILTIN_PROFILE_FACTS: &[profile_registry::BuiltinProfileFacts] = &[
            $(
                profile_registry::BuiltinProfileFacts {
                    id: ProfileId::$id,
                    group: ProfileGroup::$profile_group,
                    type_name: stringify!($profile),
                    display_name: $model_name,
                    vendor: $vendor,
                    description: $description,
                    envelope: profile_registry::EnvelopeKind::$envelope_kind,
                    tcp: __network_transport_facts!($tcp),
                    udp: __network_transport_facts!($udp),
                    serial: __serial_transport_facts!($serial),
                    default_camera_id: $default_camera_id,
                    inquiry_support: $inquiry_support,
                    typed_support: <$profile as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT,
                    evidence: &[
                        $(profile_registry::ProfileEvidence {
                            key: $evidence_key,
                            rationale: $evidence_text,
                        },)*
                    ],
                },
            )*
        ];

        impl ProfileId {
            /// Returns the human-readable display name for this profile.
            pub const fn display_name(&self) -> &'static str {
                match self {
                    $(ProfileId::$id => $model_name,)*
                }
            }

            /// Returns the default TCP port for this profile when TCP is supported.
            pub const fn default_tcp_port(&self) -> Option<u16> {
                match self {
                    $(ProfileId::$id => __transport_default_port!($tcp),)*
                }
            }

            /// Returns the default UDP port for this profile when UDP is supported.
            pub const fn default_udp_port(&self) -> Option<u16> {
                match self {
                    $(ProfileId::$id => __transport_default_port!($udp),)*
                }
            }

            /// Returns the default camera ID for this profile.
            pub const fn default_camera_id(&self) -> u8 {
                match self {
                    $(ProfileId::$id => $default_camera_id,)*
                }
            }

            /// Returns whether this profile uses Sony encapsulation.
            pub const fn uses_sony_encapsulation(&self) -> bool {
                match self {
                    $(ProfileId::$id => matches!(
                        profile_registry::EnvelopeKind::$envelope_kind,
                        profile_registry::EnvelopeKind::SonyEncapsulated
                    ),)*
                }
            }

            /// Returns the profile group for this camera profile.
            pub const fn profile_group(&self) -> ProfileGroup {
                match self {
                    $(ProfileId::$id => ProfileGroup::$profile_group,)*
                }
            }

            /// Returns all available profile IDs.
            pub const fn all() -> &'static [ProfileId] {
                &[$(ProfileId::$id,)*]
            }

            /// Returns whether this profile supports TCP transport.
            pub const fn supports_tcp(&self) -> bool {
                match self {
                    $(ProfileId::$id => __transport_is_supported!($tcp),)*
                }
            }

            /// Returns whether this profile supports UDP transport.
            pub const fn supports_udp(&self) -> bool {
                match self {
                    $(ProfileId::$id => __transport_is_supported!($udp),)*
                }
            }

            /// Returns whether this profile supports serial (RS-232/RS-422) transport.
            pub const fn supports_serial(&self) -> bool {
                match self {
                    $(ProfileId::$id => __transport_is_supported!($serial),)*
                }
            }

            /// Returns whether this profile supports the selected standard transport.
            pub const fn supports_transport(
                &self,
                transport: $crate::camera::TransportKind,
            ) -> bool {
                match transport {
                    $crate::camera::TransportKind::Tcp => self.supports_tcp(),
                    $crate::camera::TransportKind::Udp => self.supports_udp(),
                    $crate::camera::TransportKind::Serial => self.supports_serial(),
                    $crate::camera::TransportKind::Custom => true,
                }
            }

            /// Returns a vendor identifier for this profile.
            pub const fn vendor(&self) -> &'static str {
                match self {
                    $(ProfileId::$id => $vendor,)*
                }
            }

            /// Returns the level of VISCA inquiry command support for this profile.
            pub const fn inquiry_support(&self) -> $crate::capabilities::InquirySupport {
                match self {
                    $(ProfileId::$id => $inquiry_support,)*
                }
            }

            /// Returns a brief description of this profile's capabilities.
            pub const fn description(&self) -> &'static str {
                match self {
                    $(ProfileId::$id => $description,)*
                }
            }

            #[cfg(test)]
            pub(crate) fn registry_facts(&self) -> &'static profile_registry::BuiltinProfileFacts {
                BUILTIN_PROFILE_FACTS
                    .iter()
                    .find(|facts| facts.id == *self)
                    .expect("all ProfileId variants are generated from BUILTIN_PROFILE_FACTS")
            }
        }

        impl ProfileGroup {
            /// Returns all profile IDs that belong to this group.
            pub const fn profiles(&self) -> &'static [ProfileId] {
                match self {
                    $(ProfileGroup::$group => &[$(ProfileId::$group_profile,)*],)*
                }
            }

            /// Returns a human-readable display name for this profile group.
            pub const fn display_name(&self) -> &'static str {
                match self {
                    $(ProfileGroup::$group => $group_display,)*
                }
            }

            /// Returns whether this profile group uses Sony encapsulation protocol.
            pub const fn uses_sony_encapsulation(&self) -> bool {
                match self {
                    $(ProfileGroup::$group => $group_sony,)*
                }
            }

            /// Returns whether this profile group supports TCP transport.
            pub const fn supports_tcp(&self) -> bool {
                match self {
                    $(ProfileGroup::$group => $group_tcp,)*
                }
            }

            /// Returns whether this profile group supports UDP transport.
            pub const fn supports_udp(&self) -> bool {
                match self {
                    $(ProfileGroup::$group => $group_udp,)*
                }
            }

            /// Returns whether this profile group supports serial transport.
            pub const fn supports_serial(&self) -> bool {
                match self {
                    $(ProfileGroup::$group => $group_serial,)*
                }
            }

            /// Returns the level of VISCA inquiry command support for this profile group.
            pub const fn inquiry_support(&self) -> $crate::capabilities::InquirySupport {
                match self {
                    $(ProfileGroup::$group => $group_inquiry,)*
                }
            }
        }

        impl std::fmt::Display for ProfileGroup {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }

        impl std::fmt::Display for ProfileId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }

        #[cfg(test)]
        fn registry_profile_type_names_for(
            surface: $crate::capabilities::TypedSupportSurface,
        ) -> String {
            BUILTIN_PROFILE_FACTS
                .iter()
                .filter(|facts| facts.has_typed_support(surface))
                .map(|facts| format!("`{}`", facts.type_name))
                .collect::<Vec<_>>()
                .join(", ")
        }

        #[cfg(test)]
        fn typed_support_marker_trait_name(
            surface: $crate::capabilities::TypedSupportSurface,
        ) -> &'static str {
            match surface {
                $crate::capabilities::TypedSupportSurface::DirectZoom => "HasDirectZoom",
                $crate::capabilities::TypedSupportSurface::DigitalZoomToggle => {
                    "HasDigitalZoomToggle"
                }
                $crate::capabilities::TypedSupportSurface::DigitalZoomRange => "HasDigitalZoomRange",
                $crate::capabilities::TypedSupportSurface::IrisControl => "HasIrisControl",
                $crate::capabilities::TypedSupportSurface::OnePushFocus => "HasOnePushFocus",
                $crate::capabilities::TypedSupportSurface::PtzOpticsSnapFocus => {
                    "HasPtzOpticsSnapFocus"
                }
                $crate::capabilities::TypedSupportSurface::FocusLock => "HasFocusLock",
                $crate::capabilities::TypedSupportSurface::PushAutoFocus => "HasPushAutoFocus",
                $crate::capabilities::TypedSupportSurface::FocusZone => "HasFocusZone",
                $crate::capabilities::TypedSupportSurface::AutoFocusSensitivity => {
                    "HasAutoFocusSensitivity"
                }
                $crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry => {
                    "HasFocusNearLimitInquiry"
                }
                $crate::capabilities::TypedSupportSurface::BacklightCompensation => {
                    "HasBacklightCompensation"
                }
                $crate::capabilities::TypedSupportSurface::WideDynamicRange => "HasWideDynamicRange",
                $crate::capabilities::TypedSupportSurface::ExposureCompensation => {
                    "HasExposureCompensation"
                }
                $crate::capabilities::TypedSupportSurface::BrightnessControl => "HasBrightnessControl",
                $crate::capabilities::TypedSupportSurface::OnePushWhiteBalance => {
                    "HasOnePushWhiteBalance"
                }
                $crate::capabilities::TypedSupportSurface::AutoTrackingWhiteBalance => {
                    "HasAutoTrackingWhiteBalance"
                }
                $crate::capabilities::TypedSupportSurface::AutoWhiteBalanceSensitivity => {
                    "HasAutoWhiteBalanceSensitivity"
                }
                $crate::capabilities::TypedSupportSurface::ColorTemperature => "HasColorTemperature",
                $crate::capabilities::TypedSupportSurface::RgbGain => "HasRgbGain",
                $crate::capabilities::TypedSupportSurface::RgbTuning => "HasRgbTuning",
                $crate::capabilities::TypedSupportSurface::ImageFlip => "HasImageFlip",
                $crate::capabilities::TypedSupportSurface::ImageMirror => "HasImageMirror",
                $crate::capabilities::TypedSupportSurface::CombinedImageFlip => "HasCombinedImageFlip",
                $crate::capabilities::TypedSupportSurface::ContrastControl => "HasContrastControl",
                $crate::capabilities::TypedSupportSurface::SharpnessControl => "HasSharpnessControl",
                $crate::capabilities::TypedSupportSurface::SaturationControl => "HasSaturationControl",
                $crate::capabilities::TypedSupportSurface::HueControl => "HasHueControl",
                $crate::capabilities::TypedSupportSurface::LuminanceControl => "HasLuminanceControl",
                $crate::capabilities::TypedSupportSurface::GammaControl => "HasGammaControl",
                $crate::capabilities::TypedSupportSurface::NoiseReduction => "HasNoiseReduction",
                $crate::capabilities::TypedSupportSurface::NoiseReduction2D => "HasNoiseReduction2D",
                $crate::capabilities::TypedSupportSurface::NoiseReduction3D => "HasNoiseReduction3D",
                $crate::capabilities::TypedSupportSurface::PictureEffect => "HasPictureEffect",
                $crate::capabilities::TypedSupportSurface::Tally => "HasTally",
                $crate::capabilities::TypedSupportSurface::DirectMenu => "HasDirectMenuControl",
                $crate::capabilities::TypedSupportSurface::NdFilter => "HasNdFilter",
                $crate::capabilities::TypedSupportSurface::VariableSpeed => "HasVariableSpeed",
                $crate::capabilities::TypedSupportSurface::MotionSync => "HasMotionSync",
            }
        }

        #[cfg(test)]
        mod registry_tests {
            use super::*;

            fn assert_profile_registry<P>(
                facts: &profile_registry::BuiltinProfileFacts,
                id: ProfileId,
            )
            where
                P: $crate::capabilities::Profile + Default,
            {
                let caps = $crate::capabilities::Capabilities::from_profile::<P>();

                assert_eq!(facts.id, id);
                assert_eq!(facts.group, id.profile_group());
                assert_eq!(facts.display_name, P::MODEL_NAME);
                assert_eq!(facts.display_name, id.display_name());
                assert_eq!(facts.vendor, id.vendor());
                assert_eq!(facts.description, id.description());
                assert_eq!(facts.tcp.map(|facts| facts.default_port), id.default_tcp_port());
                assert_eq!(facts.udp.map(|facts| facts.default_port), id.default_udp_port());
                assert_eq!(facts.tcp.is_some(), id.supports_tcp());
                assert_eq!(facts.udp.is_some(), id.supports_udp());
                assert_eq!(facts.serial.is_some(), id.supports_serial());
                assert_eq!(facts.default_camera_id, P::DEFAULT_CAMERA_ID);
                assert_eq!(facts.default_camera_id, id.default_camera_id());
                assert_eq!(facts.inquiry_support, P::INQUIRY_SUPPORT);
                assert_eq!(facts.inquiry_support, id.inquiry_support());
                assert_eq!(
                    facts.typed_support,
                    <P as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT
                );
                assert_eq!(
                    facts.envelope == profile_registry::EnvelopeKind::SonyEncapsulated,
                    id.uses_sony_encapsulation()
                );
                if let Some(range) = P::BRIGHTNESS_RANGE {
                    assert!(
                        range.start < range.end,
                        "{id:?} exposure brightness metadata must be non-empty"
                    );
                }
                if let Some(range) = P::CONTRAST_RANGE {
                    assert!(
                        range.start < range.end,
                        "{id:?} contrast metadata must be non-empty"
                    );
                }
                if let Some(range) = P::SHARPNESS_RANGE {
                    assert!(
                        range.start < range.end,
                        "{id:?} sharpness metadata must be non-empty"
                    );
                }
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::BrightnessControl),
                    P::BRIGHTNESS_RANGE.is_some(),
                    "{id:?} exposure brightness marker and metadata must match"
                );
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::ContrastControl),
                    P::CONTRAST_RANGE.is_some(),
                    "{id:?} contrast marker and metadata must match"
                );
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::SharpnessControl),
                    P::SHARPNESS_RANGE.is_some(),
                    "{id:?} sharpness marker and metadata must match"
                );
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::IrisControl),
                    P::IRIS_RANGE.is_some(),
                    "{id:?} iris marker and metadata must match"
                );
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::PictureEffect),
                    P::SUPPORTS_PICTURE_EFFECT,
                    "{id:?} picture-effect marker and metadata must match"
                );

                assert_eq!(caps.model_name, P::MODEL_NAME);
                assert_eq!(caps.default_camera_id, P::DEFAULT_CAMERA_ID);
                assert_eq!(caps.default_tcp_port, id.default_tcp_port());
                assert_eq!(caps.default_udp_port, id.default_udp_port());
                assert_eq!(caps.pan_speed, 1..=P::MAX_PAN_SPEED);
                assert_eq!(caps.tilt_speed, 1..=P::MAX_TILT_SPEED);
                assert_eq!(caps.pan_range, P::PAN_RANGE.start..=(P::PAN_RANGE.end - 1));
                assert_eq!(caps.tilt_range, P::TILT_RANGE.start..=(P::TILT_RANGE.end - 1));
                assert_eq!(caps.pan_tilt_simultaneous, P::PAN_TILT_SIMULTANEOUS);
                assert_eq!(caps.has_digital_zoom, P::DIGITAL_ZOOM_MAX.is_some());
                assert_eq!(caps.zoom_range_optical, 0..=P::OPTICAL_ZOOM_MAX);
                assert_eq!(
                    caps.zoom_range_digital,
                    P::DIGITAL_ZOOM_MAX.map(|max| P::OPTICAL_ZOOM_MAX..=max)
                );
                assert_eq!(caps.zoom_speed, P::ZOOM_SPEED_RANGE.start..=(P::ZOOM_SPEED_RANGE.end - 1));
                assert_eq!(caps.supports_direct_zoom, P::SUPPORTS_DIRECT_ZOOM);
                assert_eq!(caps.supports_variable_zoom, P::SUPPORTS_VARIABLE_ZOOM);
                assert_eq!(caps.has_auto_focus, P::SUPPORTS_AUTO_FOCUS);
                assert_eq!(caps.has_one_push_focus, P::SUPPORTS_ONE_PUSH_FOCUS);
                assert_eq!(caps.focus_range, P::FOCUS_NEAR_LIMIT..=P::FOCUS_FAR_LIMIT);
                assert_eq!(caps.focus_speed, 0..=7);
                assert_eq!(caps.has_focus_zone, P::SUPPORTS_FOCUS_ZONE);
                assert_eq!(caps.has_af_sensitivity, P::SUPPORTS_AF_SENSITIVITY);
                assert_eq!(
                    caps.has_focus_near_limit_inquiry,
                    P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY
                );
                assert_eq!(caps.has_iris_control, P::IRIS_RANGE.is_some());
                assert_eq!(caps.exposure_modes, P::EXPOSURE_MODES);
                assert_eq!(caps.has_backlight_comp, P::SUPPORTS_BACKLIGHT_COMP);
                assert_eq!(caps.has_wdr, P::SUPPORTS_WDR);
                assert_eq!(caps.has_exposure_comp, P::SUPPORTS_EXPOSURE_COMP);
                assert_eq!(
                    caps.iris_range,
                    P::IRIS_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(caps.gain_range, P::GAIN_RANGE.start..=P::GAIN_RANGE.end - 1);
                assert_eq!(caps.shutter_speed_count, P::SHUTTER_SPEEDS.len());
                assert_eq!(
                    caps.exposure_brightness_range,
                    P::BRIGHTNESS_RANGE
                        .as_ref()
                        .map(|range| range.start..=range.end - 1)
                );
                assert_eq!(caps.has_one_push_wb, P::SUPPORTS_ONE_PUSH_WB);
                assert_eq!(caps.has_color_temp, P::SUPPORTS_COLOR_TEMP);
                assert_eq!(
                    caps.color_temp_range,
                    P::COLOR_TEMP_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(
                    caps.rg_tuning_range,
                    P::RG_TUNING_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(
                    caps.bg_tuning_range,
                    P::BG_TUNING_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(caps.has_rgb_gain, P::SUPPORTS_RGB_GAIN);
                assert_eq!(caps.wb_mode_count, P::WB_MODES.len());
                assert_eq!(
                    caps.contrast_range,
                    P::CONTRAST_RANGE
                        .as_ref()
                        .map(|range| range.start..=range.end - 1)
                );
                assert_eq!(
                    caps.sharpness_range,
                    P::SHARPNESS_RANGE
                        .as_ref()
                        .map(|range| range.start..=range.end - 1)
                );
                assert_eq!(
                    caps.saturation_range,
                    P::SATURATION_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(
                    caps.hue_range,
                    P::HUE_RANGE.as_ref().map(|range| range.start..=range.end - 1)
                );
                assert_eq!(caps.supports_flip, P::SUPPORTS_FLIP);
                assert_eq!(caps.supports_mirror, P::SUPPORTS_MIRROR);
                assert_eq!(caps.has_noise_reduction, P::SUPPORTS_NOISE_REDUCTION);
                assert_eq!(caps.has_2d_nr, P::SUPPORTS_2D_NR);
                assert_eq!(caps.has_3d_nr, P::SUPPORTS_3D_NR);
                assert_eq!(caps.has_picture_effect, P::SUPPORTS_PICTURE_EFFECT);
                assert_eq!(caps.has_gamma, P::SUPPORTS_GAMMA);
                assert_eq!(caps.has_luminance, P::SUPPORTS_LUMINANCE);
                assert_eq!(caps.has_tally, P::SUPPORTS_TALLY);
                assert_eq!(caps.max_presets, P::MAX_PRESETS);
                assert_eq!(
                    caps.preset_speed_range,
                    P::PRESET_SPEED_RANGE.start..=P::PRESET_SPEED_RANGE.end - 1
                );
                assert_eq!(caps.supports_preset_tour, P::SUPPORTS_PRESET_TOUR);
                assert_eq!(caps.supports_preset_thumbnail, P::SUPPORTS_PRESET_THUMBNAIL);
                assert_eq!(caps.supports_standby, P::SUPPORTS_STANDBY);
                assert_eq!(caps.supports_wake_on_lan, P::SUPPORTS_WAKE_ON_LAN);
                assert_eq!(caps.power_on_time_secs, P::POWER_ON_TIME.as_secs());
                assert_eq!(
                    caps.has_nd_filter,
                    !matches!(P::ND_MODE, $crate::capabilities::NdFilterMode::None)
                );
                assert_eq!(caps.has_motion_sync, P::SUPPORTS_MOTION_SYNC);
                assert_eq!(caps.max_motion_sync_speed, P::SUPPORTS_MOTION_SYNC.then_some(P::MAX_MOTION_SYNC_SPEED));
                assert_eq!(caps.has_direct_menu_control, P::SUPPORTS_DIRECT_CONTROL);
                assert_eq!(caps.has_variable_speed, P::SUPPORTS_VARIABLE_SPEED);
                assert_eq!(caps.inquiry_support, P::INQUIRY_SUPPORT);
                assert_eq!(caps.supports_operation_complete, P::SUPPORTS_OPERATION_COMPLETE);
                assert_eq!(
                    caps.typed_support,
                    <P as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT
                );
            }

            #[test]
            fn profile_ids_and_groups_match_registry() {
                assert_eq!(ProfileId::all().len(), BUILTIN_PROFILE_FACTS.len());
                for facts in BUILTIN_PROFILE_FACTS {
                    let id = facts.id;
                    assert_eq!(id.registry_facts().type_name, facts.type_name);
                    assert!(facts.group.profiles().contains(&id));
                    assert_eq!(
                        facts.envelope == profile_registry::EnvelopeKind::SonyEncapsulated,
                        facts.group.uses_sony_encapsulation()
                    );
                }
            }

            #[test]
            fn builtin_profile_capabilities_match_registry() {
                $(
                    assert_profile_registry::<$profile>(
                        ProfileId::$id.registry_facts(),
                        ProfileId::$id,
                    );
                )*
            }

            #[test]
            fn typed_support_registry_has_evidence_for_surprising_gaps() {
                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30X,
                ] {
                    let facts = id.registry_facts();
                    assert!(
                        facts.evidence.iter().any(|item| item.key == "digital_zoom"),
                        "{id:?} must document why digital zoom typed support is absent"
                    );
                    if !facts.has_typed_support($crate::capabilities::TypedSupportSurface::IrisControl)
                    {
                        assert!(
                            facts.evidence.iter().any(|item| item.key == "iris"),
                            "{id:?} must document why iris typed support is absent"
                        );
                    }
                }
            }

            #[test]
            fn ptzoptics_profiles_follow_consolidated_reference_boundaries() {
                fn assert_ptzoptics_raw_profile<P>()
                where
                    P: $crate::capabilities::Profile
                        + $crate::capabilities::SupportsTcp
                        + $crate::capabilities::SupportsUdp
                        + $crate::capabilities::SupportsSerial
                        + Default,
                {
                    assert_eq!(
                        <P as $crate::capabilities::SupportsTcp>::DEFAULT_TCP_PORT,
                        5678
                    );
                    assert_eq!(
                        <P as $crate::capabilities::SupportsUdp>::DEFAULT_UDP_PORT,
                        1259
                    );
                    assert_eq!(P::PAN_RANGE, -2448..2449);
                    assert_eq!(P::TILT_RANGE, -432..1297);
                    assert_eq!(P::MAX_PAN_SPEED, 24);
                    assert_eq!(P::MAX_TILT_SPEED, 20);
                    assert_eq!(P::OPTICAL_ZOOM_MAX, 0x4000);
                    assert_eq!(P::DIGITAL_ZOOM_MAX, None);
                    assert!(P::SUPPORTS_DIRECT_ZOOM);
                    assert!(P::IRIS_RANGE.is_some());
                    assert!(
                        P::EXPOSURE_MODES
                            .contains(&$crate::command::exposure::ExposureMode::Iris)
                    );
                    assert!(P::SUPPORTS_FOCUS_ZONE);
                    assert!(!P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY);
                    assert_eq!(P::MAX_PRESETS, 127);
                    assert!(P::SUPPORTS_PICTURE_EFFECT);
                    assert!(!P::SUPPORTS_PRESET_TOUR);
                }

                assert_ptzoptics_raw_profile::<PtzOpticsG2>();
                assert_ptzoptics_raw_profile::<PtzOpticsG3>();
                assert_ptzoptics_raw_profile::<PtzOptics30X>();

                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30X,
                ] {
                    let facts = id.registry_facts();
                    assert!(facts.has_typed_support($crate::capabilities::TypedSupportSurface::FocusZone));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::IrisControl));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::PictureEffect));
                    assert!(!facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry
                    ));
                }
            }

            #[test]
            fn typed_support_registry_covers_declared_surface_vocabulary() {
                for facts in BUILTIN_PROFILE_FACTS {
                    for surface in facts.typed_support.iter() {
                        assert!(
                            $crate::capabilities::TypedSupportSurface::ALL.contains(&surface),
                            "{surface:?} is not in ALL_TYPED_SUPPORT_SURFACES"
                        );
                    }
                }

                for surface in $crate::capabilities::TypedSupportSurface::ALL {
                    let profiles = registry_profile_type_names_for(surface);
                    if matches!(
                        surface,
                        $crate::capabilities::TypedSupportSurface::OnePushFocus
                            | $crate::capabilities::TypedSupportSurface::PtzOpticsSnapFocus
                            | $crate::capabilities::TypedSupportSurface::MotionSync
                    ) {
                        assert!(
                            profiles.is_empty(),
                            "{surface:?} should remain absent for built-ins until evidenced"
                        );
                    } else {
                        assert!(
                            !profiles.is_empty(),
                            "{surface:?} has no built-in typed support and should be removed or classified"
                        );
                    }
                }
            }

            #[test]
            fn readme_profile_support_matrix_matches_registry() {
                let readme = include_str!("../../README.md");

                fn assert_row(
                    readme: &str,
                    label: &str,
                    surface: $crate::capabilities::TypedSupportSurface,
                ) {
                    let profiles = registry_profile_type_names_for(surface);
                    let row = format!("| {label} | {profiles} |");
                    assert!(readme.contains(&row), "missing README row: {row}");
                }

                fn assert_literal_row(readme: &str, label: &str, value: &str) {
                    let row = format!("| {label} | {value} |");
                    assert!(readme.contains(&row), "missing README row: {row}");
                }

                assert_row(
                    readme,
                    "ND filter controls and inquiries",
                    $crate::capabilities::TypedSupportSurface::NdFilter,
                );
                assert_row(
                    readme,
                    "Variable speed mode controls",
                    $crate::capabilities::TypedSupportSurface::VariableSpeed,
                );
                assert_row(
                    readme,
                    "Tally controls and inquiries",
                    $crate::capabilities::TypedSupportSurface::Tally,
                );
                assert_row(
                    readme,
                    "Direct menu controls",
                    $crate::capabilities::TypedSupportSurface::DirectMenu,
                );
                assert_literal_row(
                    readme,
                    "Motion Sync controls and inquiries",
                    "Custom/evidenced profiles that explicitly implement `HasMotionSync`; no built-in profile is marked from the current specs",
                );

                assert_row(
                    readme,
                    "Direct absolute zoom positioning",
                    $crate::capabilities::TypedSupportSurface::DirectZoom,
                );
                assert_eq!(
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::DigitalZoomToggle
                    ),
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::DigitalZoomRange
                    )
                );
                assert_row(
                    readme,
                    "VISCA digital zoom toggle and optical-plus-digital positioning",
                    $crate::capabilities::TypedSupportSurface::DigitalZoomToggle,
                );
                assert_row(
                    readme,
                    "Iris control, iris-priority mode, and iris inquiry",
                    $crate::capabilities::TypedSupportSurface::IrisControl,
                );
                assert_literal_row(
                    readme,
                    "Standard one-push focus",
                    "No built-in profile currently marks this typed capability",
                );
                assert_literal_row(
                    readme,
                    "PTZOptics snap focus",
                    "No built-in profile currently marks this typed capability",
                );
                assert_row(
                    readme,
                    "Focus lock",
                    $crate::capabilities::TypedSupportSurface::FocusLock,
                );
                assert_row(
                    readme,
                    "Push auto focus",
                    $crate::capabilities::TypedSupportSurface::PushAutoFocus,
                );
                assert_row(
                    readme,
                    "Focus zone",
                    $crate::capabilities::TypedSupportSurface::FocusZone,
                );
                assert_row(
                    readme,
                    "Auto focus sensitivity",
                    $crate::capabilities::TypedSupportSurface::AutoFocusSensitivity,
                );
                assert_row(
                    readme,
                    "Focus near-limit inquiry",
                    $crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry,
                );
                assert_row(
                    readme,
                    "Backlight compensation",
                    $crate::capabilities::TypedSupportSurface::BacklightCompensation,
                );
                assert_row(
                    readme,
                    "Wide dynamic range",
                    $crate::capabilities::TypedSupportSurface::WideDynamicRange,
                );
                assert_row(
                    readme,
                    "Exposure compensation",
                    $crate::capabilities::TypedSupportSurface::ExposureCompensation,
                );
                assert_row(
                    readme,
                    "Exposure brightness control and inquiry",
                    $crate::capabilities::TypedSupportSurface::BrightnessControl,
                );
                assert_row(
                    readme,
                    "One-push white balance",
                    $crate::capabilities::TypedSupportSurface::OnePushWhiteBalance,
                );
                assert_row(
                    readme,
                    "Auto-tracking white balance",
                    $crate::capabilities::TypedSupportSurface::AutoTrackingWhiteBalance,
                );
                assert_row(
                    readme,
                    "Auto white-balance sensitivity",
                    $crate::capabilities::TypedSupportSurface::AutoWhiteBalanceSensitivity,
                );
                assert_row(
                    readme,
                    "Color temperature controls and inquiry",
                    $crate::capabilities::TypedSupportSurface::ColorTemperature,
                );
                assert_row(
                    readme,
                    "RGB gain controls and inquiries",
                    $crate::capabilities::TypedSupportSurface::RgbGain,
                );
                assert_row(
                    readme,
                    "RGB tuning controls and inquiries",
                    $crate::capabilities::TypedSupportSurface::RgbTuning,
                );
                assert_eq!(
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::ImageFlip
                    ),
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::ImageMirror
                    )
                );
                assert_row(
                    readme,
                    "Flip and mirror controls",
                    $crate::capabilities::TypedSupportSurface::ImageFlip,
                );
                assert_row(
                    readme,
                    "Combined image flip mode",
                    $crate::capabilities::TypedSupportSurface::CombinedImageFlip,
                );
                assert_row(
                    readme,
                    "Contrast control and inquiry",
                    $crate::capabilities::TypedSupportSurface::ContrastControl,
                );
                assert_row(
                    readme,
                    "Sharpness control and inquiry",
                    $crate::capabilities::TypedSupportSurface::SharpnessControl,
                );
                assert_row(
                    readme,
                    "Saturation control and inquiry",
                    $crate::capabilities::TypedSupportSurface::SaturationControl,
                );
                assert_row(
                    readme,
                    "Hue control and inquiry",
                    $crate::capabilities::TypedSupportSurface::HueControl,
                );
                assert_row(
                    readme,
                    "Luminance control and inquiry",
                    $crate::capabilities::TypedSupportSurface::LuminanceControl,
                );
                assert_row(
                    readme,
                    "Gamma control and inquiry",
                    $crate::capabilities::TypedSupportSurface::GammaControl,
                );
                assert_row(
                    readme,
                    "Aggregate noise-reduction inquiry",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction,
                );
                assert_eq!(
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2D
                    ),
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3D
                    )
                );
                assert_row(
                    readme,
                    "2D/3D noise reduction",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                );
                assert_row(
                    readme,
                    "Picture effects",
                    $crate::capabilities::TypedSupportSurface::PictureEffect,
                );
            }

            #[test]
            fn camera_profile_support_marker_matrix_matches_registry() {
                let guide = include_str!("../../docs/camera_profile_support.md");

                for facts in BUILTIN_PROFILE_FACTS {
                    let prefix = format!("| `{}` | ", facts.type_name);
                    let row = guide
                        .lines()
                        .find(|line| line.starts_with(&prefix) && line.contains("Has"));
                    assert!(
                        row.is_some(),
                        "missing profile marker row for {}",
                        facts.type_name
                    );
                    let row = row.unwrap_or("");
                    for surface in $crate::capabilities::TypedSupportSurface::ALL {
                        let marker = format!("`{}`", typed_support_marker_trait_name(surface));
                        assert_eq!(
                            row.contains(&marker),
                            facts.has_typed_support(surface),
                            "{:?} marker matrix mismatch for {}",
                            facts.id,
                            typed_support_marker_trait_name(surface)
                        );
                    }

                    let tcp = facts
                        .tcp
                        .map(|transport| transport.default_port.to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    let udp = facts
                        .udp
                        .map(|transport| transport.default_port.to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    let serial = if facts.serial.is_some() { "yes" } else { "no" };
                    let envelope = if facts.envelope == profile_registry::EnvelopeKind::SonyEncapsulated {
                        "Sony encapsulated UDP"
                    } else {
                        "Raw VISCA"
                    };
                    let row = format!(
                        "| `{}` | {tcp} | {udp} | {serial} | {envelope} |",
                        facts.type_name
                    );
                    assert!(guide.contains(&row), "missing profile transport row: {row}");
                }
            }
        }
    };
}

macro_rules! define_builtin_profiles {
    () => {
        __define_builtin_profiles! {
            groups {
                group GenericVisca {
                    doc: "Generic VISCA-compatible cameras with basic features.",
                    display: "Generic VISCA",
                    inquiry_support: $crate::capabilities::InquirySupport::Partial,
                    uses_sony_encapsulation: false,
                    transport: { tcp: true, udp: true, serial: true },
                    profiles: [GenericVisca, SonyBrc300, SonyEviH100, NearusBrc300],
                }
                group PtzOpticsG2 {
                    doc: "PtzOptics G2/G3/30X cameras using raw VISCA with PTZOptics timing limits.",
                    display: "PtzOptics Series",
                    inquiry_support: $crate::capabilities::InquirySupport::Full,
                    uses_sony_encapsulation: false,
                    transport: { tcp: true, udp: true, serial: true },
                    profiles: [PtzOpticsG2, PtzOpticsG3, PtzOptics30X],
                }
                group SonyProfessional {
                    doc: "Sony professional cameras using Sony encapsulated VISCA.",
                    display: "Sony Professional",
                    inquiry_support: $crate::capabilities::InquirySupport::Full,
                    uses_sony_encapsulation: true,
                    transport: { tcp: false, udp: true, serial: false },
                    profiles: [SonyFr7, SonyBrcH900],
                }
            }
            profiles {
                profile PtzOpticsG2 {
                    doc: "PtzOptics G2 camera profile.",
                    id: PtzOpticsG2,
                    id_doc: "PtzOptics G2 series cameras",
                    id_attrs: [],
                    group: PtzOpticsG2,
                    vendor: "PtzOptics",
                    description: "20x optical zoom PTZ camera with 128 presets",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "PtzOptics G2",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: -2448..2449,
                        tilt_range: -432..1297,
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x0D),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: 0..8,
                        brightness_range: Some(0..18),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: -7..8,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-10..11),
                        bg_tuning_range: Some(-10..11),
                        color_temp: true,
                        color_temp_range: Some(2500..8001),
                        rgb_gain: true,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: Some(0..15),
                        sharpness_range: Some(0..16),
                        saturation_range: Some(0..15),
                        flip: true,
                        mirror: true,
                        hue: true,
                        hue_range: Some(0..15),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(0..15),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 127,
                        speed_range: 1..25,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 10,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        ExposureCompensation,
                        BrightnessControl,
                        FocusLock,
                        DirectZoom,
                        IrisControl,
                        FocusZone,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        RgbGain,
                        RgbTuning,
                        OnePushWhiteBalance,
                        AutoWhiteBalanceSensitivity,
                        ImageFlip,
                        ImageMirror,
                        CombinedImageFlip,
                        ContrastControl,
                        SharpnessControl,
                        SaturationControl,
                        HueControl,
                        LuminanceControl,
                        GammaControl,
                        NoiseReduction,
                        NoiseReduction2D,
                        NoiseReduction3D,
                        PictureEffect,
                    ],
                    evidence: [
                        ("digital_zoom", "Hardware rejects VISCA digital zoom control on tested G2 firmware."),
                    ],
                }

                profile PtzOpticsG3 {
                    doc: "PtzOptics G3 camera profile.",
                    id: PtzOpticsG3,
                    id_doc: "PtzOptics G3 series cameras",
                    id_attrs: [],
                    group: PtzOpticsG2,
                    vendor: "PtzOptics",
                    description: "Latest generation PTZ camera using the conservative raw VISCA preset range",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "PtzOptics G3",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: -2448..2449,
                        tilt_range: -432..1297,
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x0D),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: 0..8,
                        brightness_range: Some(0..18),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: -7..8,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-10..11),
                        bg_tuning_range: Some(-10..11),
                        color_temp: true,
                        color_temp_range: Some(2500..8001),
                        rgb_gain: true,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: Some(0..15),
                        sharpness_range: Some(0..16),
                        saturation_range: Some(0..15),
                        flip: true,
                        mirror: true,
                        hue: true,
                        hue_range: Some(0..15),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(0..15),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 127,
                        speed_range: 1..25,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 10,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        ExposureCompensation,
                        BrightnessControl,
                        FocusLock,
                        DirectZoom,
                        IrisControl,
                        FocusZone,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        RgbGain,
                        RgbTuning,
                        OnePushWhiteBalance,
                        AutoWhiteBalanceSensitivity,
                        ImageFlip,
                        ImageMirror,
                        CombinedImageFlip,
                        ContrastControl,
                        SharpnessControl,
                        SaturationControl,
                        HueControl,
                        LuminanceControl,
                        GammaControl,
                        NoiseReduction,
                        NoiseReduction2D,
                        NoiseReduction3D,
                        PictureEffect,
                    ],
                    evidence: [
                        ("digital_zoom", "PTZOptics built-ins keep VISCA digital zoom unavailable until model-specific evidence exists."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                    ],
                }

                profile PtzOptics30X {
                    doc: "PtzOptics 30X camera profile.",
                    id: PtzOptics30X,
                    id_doc: "PtzOptics 30X cameras",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "ptzoptics-30x"))]],
                    group: PtzOpticsG2,
                    vendor: "PtzOptics",
                    description: "High-end 30x optical zoom PTZ camera with extended optical range",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "PtzOptics 30X",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: -2448..2449,
                        tilt_range: -432..1297,
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 565.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x0D),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: 0..8,
                        brightness_range: Some(0..18),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: -7..8,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-10..11),
                        bg_tuning_range: Some(-10..11),
                        color_temp: true,
                        color_temp_range: Some(2500..8001),
                        rgb_gain: true,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: Some(0..15),
                        sharpness_range: Some(0..16),
                        saturation_range: Some(0..15),
                        flip: true,
                        mirror: true,
                        hue: true,
                        hue_range: Some(0..15),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(0..15),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 127,
                        speed_range: 1..25,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 10,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        ExposureCompensation,
                        BrightnessControl,
                        FocusLock,
                        DirectZoom,
                        IrisControl,
                        FocusZone,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        RgbGain,
                        RgbTuning,
                        OnePushWhiteBalance,
                        AutoWhiteBalanceSensitivity,
                        ImageFlip,
                        ImageMirror,
                        CombinedImageFlip,
                        ContrastControl,
                        SharpnessControl,
                        SaturationControl,
                        HueControl,
                        LuminanceControl,
                        GammaControl,
                        NoiseReduction,
                        NoiseReduction2D,
                        NoiseReduction3D,
                        PictureEffect,
                    ],
                    evidence: [
                        ("digital_zoom", "The Axis 0x7AC0 digital endpoint is not applied to PTZOptics; the 30X raw VISCA profile keeps the standard 0x4000 optical endpoint and no typed digital zoom."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                    ],
                }

                profile SonyFR7 {
                    doc: "Sony FR7 camera profile.",
                    id: SonyFr7,
                    id_doc: "Sony FR7 camera",
                    id_attrs: [],
                    group: SonyProfessional,
                    vendor: "Sony",
                    description: "Professional cinema camera with variable ND filter and full feature set",
                    envelope: $crate::transport::SonyEncapsulated,
                    envelope_kind: SonyEncapsulated,
                    transport: {
                        tcp: none,
                        udp: {
                            default_port: 52381,
                            envelope_kind: SonyEncapsulated,
                            evidence: "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header.",
                        },
                        serial: none,
                    },
                    metadata: {
                        model_name: "Sony FR7",
                        default_camera_id: 1,
                        ack_timeout_ms: 200,
                        completion_timeout_ms: 8000,
                        busy_timeout_ms: 240,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 35,
                        min_command_spacing_ms: 35,
                    },
                    pan_tilt: {
                        pan_range: -2700..2701,
                        tilt_range: -300..1201,
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 15.88,
                        tilt_degrees_to_units: 15.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7000),
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 1000.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        max_speed: 7,
                        af_sensitivity: true,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x1F),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: 0..16,
                        brightness_range: Some(0..18),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: -7..8,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_FR7_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-7..8),
                        bg_tuning_range: Some(-7..8),
                        color_temp: false,
                        color_temp_range: None,
                        rgb_gain: true,
                        red_gain_range: Some(0..255),
                        blue_gain_range: Some(0..255),
                    },
                    image: {
                        contrast_range: Some(0..15),
                        sharpness_range: Some(0..15),
                        saturation_range: Some(0..15),
                        flip: true,
                        mirror: true,
                        hue: true,
                        hue_range: Some(0..15),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: false,
                        picture_effect: true,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 255,
                        speed_range: 1..25,
                        tour: true,
                        recall_delay_ms: 0,
                        thumbnail: true,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 15,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: true,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: true },
                    tally: { supported: true },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::Variable, steps: None },
                    variable_speed: { supported: true },
                    typed_support: [
                        ExposureCompensation,
                        BrightnessControl,
                        PushAutoFocus,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
                        IrisControl,
                        FocusZone,
                        AutoFocusSensitivity,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                        WideDynamicRange,
                        RgbGain,
                        RgbTuning,
                        OnePushWhiteBalance,
                        AutoTrackingWhiteBalance,
                        ImageFlip,
                        ImageMirror,
                        ContrastControl,
                        SharpnessControl,
                        SaturationControl,
                        HueControl,
                        GammaControl,
                        NoiseReduction,
                        NoiseReduction2D,
                        NoiseReduction3D,
                        PictureEffect,
                        Tally,
                        DirectMenu,
                        NdFilter,
                        VariableSpeed,
                    ],
                    evidence: [
                        ("tally", "Sony professional profile metadata and typed controls expose tally for FR7."),
                        ("color_temperature", "FR7 uses ATW/manual WB surfaces; built-in typed color-temperature control remains unavailable."),
                    ],
                }

                profile SonyBRCH900 {
                    doc: "Sony BRC-H900 camera profile.",
                    id: SonyBrcH900,
                    id_doc: "Sony BRC-H900 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-brch900"))]],
                    group: SonyProfessional,
                    vendor: "Sony",
                    description: "Professional PTZ camera with advanced image processing and 100 presets",
                    envelope: $crate::transport::SonyEncapsulated,
                    envelope_kind: SonyEncapsulated,
                    transport: {
                        tcp: none,
                        udp: {
                            default_port: 52381,
                            envelope_kind: SonyEncapsulated,
                            evidence: "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header.",
                        },
                        serial: none,
                    },
                    metadata: {
                        model_name: "Sony BRC-H900",
                        default_camera_id: 1,
                        ack_timeout_ms: 150,
                        completion_timeout_ms: 6000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 35,
                        min_command_spacing_ms: 35,
                    },
                    pan_tilt: {
                        pan_range: -2700..2701,
                        tilt_range: -300..1201,
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 15.88,
                        tilt_degrees_to_units: 15.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7000),
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x1F),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: 0..16,
                        brightness_range: Some(0..18),
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: -7..8,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-7..8),
                        bg_tuning_range: Some(-7..8),
                        color_temp: true,
                        color_temp_range: Some(2500..8001),
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: Some(0..15),
                        sharpness_range: Some(0..15),
                        saturation_range: Some(0..15),
                        flip: true,
                        mirror: true,
                        hue: false,
                        hue_range: None,
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: false,
                        picture_effect: true,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 100,
                        speed_range: 1..25,
                        tour: true,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 12,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: true },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        BrightnessControl,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        RgbTuning,
                        OnePushWhiteBalance,
                        ImageFlip,
                        ImageMirror,
                        ContrastControl,
                        SharpnessControl,
                        SaturationControl,
                        GammaControl,
                        NoiseReduction,
                        NoiseReduction2D,
                        NoiseReduction3D,
                        PictureEffect,
                        Tally,
                    ],
                    evidence: [
                        ("tally", "Sony professional profile metadata and typed controls expose tally for BRC-H900."),
                    ],
                }

                profile SonyEVIH100 {
                    doc: "Sony EVI-H100 camera profile.",
                    id: SonyEviH100,
                    id_doc: "Sony EVI-H100 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-evih100"))]],
                    group: GenericVisca,
                    vendor: "Sony",
                    description: "Compact HD PTZ camera with basic feature set",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "Sony EVI-H100",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: -1440..1441,
                        tilt_range: -480..481,
                        max_pan_speed: 18,
                        max_tilt_speed: 18,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 16.0,
                        tilt_degrees_to_units: 16.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x1C),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: 0..8,
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: -7..8,
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(-7..8),
                        bg_tuning_range: Some(-7..8),
                        color_temp: true,
                        color_temp_range: Some(2500..8001),
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: true,
                        mirror: true,
                        hue: false,
                        hue_range: None,
                        noise_reduction: true,
                        nr_2d: false,
                        nr_3d: false,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: true,
                        gamma_range: Some(0..5),
                    },
                    presets: {
                        max: 6,
                        speed_range: 1..20,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 8,
                        standby: true,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        DirectZoom,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                        ColorTemperature,
                        RgbTuning,
                        OnePushWhiteBalance,
                        ImageFlip,
                        ImageMirror,
                        GammaControl,
                        NoiseReduction,
                    ],
                    evidence: [],
                }

                profile SonyBRC300 {
                    doc: "Sony BRC-300 camera profile.",
                    id: SonyBrc300,
                    id_doc: "Sony BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-brc300"))]],
                    group: GenericVisca,
                    vendor: "Sony",
                    description: "Legacy PTZ camera with unsigned coordinate system",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "Sony BRC-300",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: -1170..1171,
                        tilt_range: -390..391,
                        max_pan_speed: 18,
                        max_tilt_speed: 17,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 13.0,
                        tilt_degrees_to_units: 13.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::UnsignedCentered,
                    },
                    zoom: {
                        optical_max: 0x1068,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 455.1,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xC000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x11),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: 0..7,
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: -7..8,
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: false,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp: false,
                        color_temp_range: None,
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: false,
                        mirror: false,
                        hue: false,
                        hue_range: None,
                        noise_reduction: false,
                        nr_2d: false,
                        nr_3d: false,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: false,
                        gamma_range: None,
                    },
                    presets: {
                        max: 16,
                        speed_range: 1..18,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 10,
                        standby: false,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        DirectZoom,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                    ],
                    evidence: [],
                }

                profile NearusBRC300 {
                    doc: "Nearus BRC-300 camera profile.",
                    id: NearusBrc300,
                    id_doc: "Nearus BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "nearus-brc300"))]],
                    group: GenericVisca,
                    vendor: "Nearus",
                    description: "Rebranded Sony BRC-300 with image processing features",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "Nearus BRC-300",
                        default_camera_id: 1,
                        ack_timeout_ms: 100,
                        completion_timeout_ms: 5000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: -1170..1171,
                        tilt_range: -390..391,
                        max_pan_speed: 18,
                        max_tilt_speed: 17,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 13.0,
                        tilt_degrees_to_units: 13.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::UnsignedCentered,
                    },
                    zoom: {
                        optical_max: 0x1068,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: true,
                        supports_variable: true,
                        magnification_to_units: 455.1,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xC000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x11),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: 0..7,
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: -7..8,
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: false,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp: false,
                        color_temp_range: None,
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: Some(0..16),
                        flip: false,
                        mirror: false,
                        hue: false,
                        hue_range: None,
                        noise_reduction: false,
                        nr_2d: false,
                        nr_3d: false,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: false,
                        gamma_range: None,
                    },
                    presets: {
                        max: 16,
                        speed_range: 1..18,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 10,
                        standby: false,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        DirectZoom,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                        SaturationControl,
                    ],
                    evidence: [],
                }

                profile GenericVisca {
                    doc: "Generic VISCA camera profile.",
                    id: GenericVisca,
                    id_doc: "Generic VISCA-compatible camera",
                    id_attrs: [#[default]],
                    group: GenericVisca,
                    vendor: "Generic",
                    description: "Conservative profile for unknown VISCA-compatible cameras",
                    envelope: $crate::transport::RawVisca,
                    envelope_kind: RawVisca,
                    transport: {
                        tcp: { default_port: 5678, envelope_kind: RawVisca },
                        udp: { default_port: 1259, envelope_kind: RawVisca },
                        serial: { envelope_kind: RawVisca },
                    },
                    metadata: {
                        model_name: "Generic VISCA Camera",
                        default_camera_id: 1,
                        ack_timeout_ms: 200,
                        completion_timeout_ms: 10000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: -2880..2881,
                        tilt_range: -1440..1441,
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 16.0,
                        tilt_degrees_to_units: 16.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                    },
                    zoom: {
                        optical_max: 0xFFFF,
                        digital_max: None,
                        speed_range: 0..8,
                        supports_direct: false,
                        supports_variable: true,
                        magnification_to_units: 1000.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xE000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(0x00..0x1C),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: 0..8,
                        brightness_range: None,
                        backlight_comp: false,
                        exposure_comp: false,
                        exposure_comp_range: -7..8,
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: true,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp: false,
                        color_temp_range: None,
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: false,
                        mirror: false,
                        hue: false,
                        hue_range: None,
                        noise_reduction: false,
                        nr_2d: false,
                        nr_3d: false,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: false,
                        gamma_range: None,
                    },
                    presets: {
                        max: 6,
                        speed_range: 1..24,
                        tour: false,
                        recall_delay_ms: 0,
                        thumbnail: false,
                        names: false,
                        max_name_len: 0,
                    },
                    power: {
                        on_secs: 30,
                        standby: false,
                        standby_secs: 1,
                        wake_on_lan: false,
                        retains_settings: true,
                        home_on_power_up: false,
                    },
                    menu: { direct: false },
                    tally: { supported: false },
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        IrisControl,
                        FocusNearLimitInquiry,
                        OnePushWhiteBalance,
                    ],
                    evidence: [
                        ("direct_zoom", "Generic profile keeps absolute zoom positioning unavailable despite baseline zoom movement."),
                    ],
                }
            }
        }
    };
}
