//! Crate-private built-in profile registry.
//!
//! This module is the source of truth for built-in profile facts. The macro at
//! the bottom expands those facts into the public zero-sized profile types,
//! profile ID/group methods, runtime metadata impls, typed support markers, and
//! invariant tests.

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
    pub(crate) envelope: crate::ProfileEnvelope,
    pub(crate) tcp: Option<u16>,
    pub(crate) udp: Option<u16>,
    pub(crate) serial: bool,
    pub(crate) default_camera_id: u8,
    pub(crate) inquiry_support: crate::capabilities::InquirySupport,
    pub(crate) position_inquiries: crate::profile::PositionInquirySupport,
    pub(crate) image_base_support: bool,
    pub(crate) focus_zone_inquiry: bool,
    pub(crate) usb_audio: bool,
    pub(crate) typed_support: crate::capabilities::TypedSupportSet,
    /// Surfaces whose dedicated metadata is advertised but whose typed marker
    /// is withheld; each needs an evidence entry saying why.
    pub(crate) withheld: crate::capabilities::TypedSupportSet,
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

/// Acknowledgement deadline every built-in profile uses until per-profile
/// values are measured on hardware (#689). Operational tuning may lengthen it,
/// never shorten it.
pub(crate) const INTERIM_ACK_TIMEOUT_MS: u64 = 500;

/// Inquiry-response deadline every built-in profile uses until per-profile
/// values are measured on hardware (#689). Operational tuning may lengthen it,
/// never shorten it.
pub(crate) const INTERIM_INQUIRY_TIMEOUT_MS: u64 = 1000;

/// A command-category deadline table written in milliseconds, for the rows
/// whose table differs from [`crate::CommandTimeouts::DEFAULT`].
pub(crate) const fn command_timeouts_ms(
    quick: u64,
    movement: u64,
    preset: u64,
    long_running: u64,
    network: u64,
) -> crate::CommandTimeouts {
    use std::time::Duration;

    crate::CommandTimeouts::new(
        Duration::from_millis(quick),
        Duration::from_millis(movement),
        Duration::from_millis(preset),
        Duration::from_millis(long_running),
        Duration::from_millis(network),
    )
}

/// A per-profile fact a profile group reports for every member.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberFact {
    SonyEncapsulation,
    Tcp,
    Udp,
    Serial,
}

/// The weaker of two inquiry support levels.
pub(crate) const fn weaker_inquiry_support(
    left: crate::capabilities::InquirySupport,
    right: crate::capabilities::InquirySupport,
) -> crate::capabilities::InquirySupport {
    use crate::capabilities::InquirySupport::{Full, None, Partial};

    match (left, right) {
        (None, _) | (_, None) => None,
        (Partial, _) | (_, Partial) => Partial,
        (Full, Full) => Full,
    }
}

macro_rules! __transport_is_supported {
    (none) => {
        false
    };
    (supported) => {
        true
    };
    ({ default_port: $default_port:expr $(,)? }) => {
        true
    };
}

/// Expands an omitted optional registry fact to the conservative `false`.
///
/// The registry grammar makes new source-backed facts opt-in so downstream
/// rows cannot accidentally gain a capability while the per-profile evidence
/// is being collected.
macro_rules! __optional_support_bool {
    () => {
        false
    };
    ($supported:expr) => {
        $supported
    };
}

macro_rules! range {
    ($ty:ty, $min:expr, $max:expr) => {
        $crate::capabilities::CapabilityRange::<$ty>::new($min, $max)
    };
}

/// A table-sourced value domain: its bounds and, when the table skips
/// positions, the unlisted ones.
macro_rules! domain {
    ($ty:ty, $min:expr, $max:expr) => {
        $crate::capabilities::CapabilityDomain::<$ty>::new($min, $max)
    };
    ($ty:ty, $min:expr, $max:expr, gaps: [$($gap:expr),+ $(,)?]) => {
        $crate::capabilities::CapabilityDomain::<$ty>::with_gaps($min, $max, &[$($gap),+])
    };
}

macro_rules! __transport_default_port {
    (none) => {
        None
    };
    ({ default_port: $default_port:expr $(,)? }) => {
        Some($default_port)
    };
}

macro_rules! __impl_supports_tcp {
    ($profile:ty, none) => {};
    ($profile:ty, { default_port: $default_port:expr $(,)? }) => {
        impl $crate::capabilities::SupportsTcp for $profile {}
    };
}

macro_rules! __impl_supports_udp {
    ($profile:ty, none) => {};
    ($profile:ty, { default_port: $default_port:expr $(,)? }) => {
        impl $crate::capabilities::SupportsUdp for $profile {}
    };
}

macro_rules! __impl_supports_serial {
    ($profile:ty, none) => {};
    ($profile:ty, supported) => {
        impl $crate::capabilities::SupportsSerial for $profile {}
    };
}

/// Emits the base image-noun marker from the same registry fact that supplies
/// `ImageProcessing::SUPPORTS_IMAGE_PROCESSING` for a built-in profile.
///
/// `ImageProcessing` itself is metadata required by every `Profile`, so a
/// blanket marker would turn an all-`None` metadata implementation into typed
/// permission. Keep the positive and negative cases declarative in each
/// profile row instead.
macro_rules! __impl_image_processing_marker {
    (true for $profile:ty) => {
        impl $crate::capabilities::HasImageProcessing for $profile {}
    };
    (false for $profile:ty) => {};
}

#[cfg(test)]
macro_rules! __assert_image_processing_marker {
    (true for $profile:ty, $assert_marker:ident) => {
        $assert_marker::<$profile>();
    };
    (false for $profile:ty, $assert_marker:ident) => {};
}

macro_rules! define_typed_support_marker_impls {
    (
        $dollar:tt
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
        macro_rules! __impl_typed_support_marker {
            $(
                ($surface for $dollar profile:ty) => {
                    impl $crate::capabilities::$marker for $dollar profile {}
                };
            )*
        }
    };
}

crate::capabilities::typed_support_registry::typed_support_registry!(
    define_typed_support_marker_impls,
    $
);
macro_rules! __define_builtin_profiles {
    (
        groups {
            $(
                group $group:ident {
                    doc: $group_doc:literal,
                    display: $group_display:literal,
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
                    vendor: $vendor:literal,
                    description: $description:literal,
                    envelope: $envelope:ty,
                    transport: {
                        tcp: $tcp:tt,
                        udp: $udp:tt,
                        serial: $serial:tt $(,)?
                    },
                    metadata: {
                        model_name: $model_name:literal,
                        default_camera_id: $default_camera_id:expr,
                        // Every built-in profile names the shared interim
                        // deadlines below until per-profile values are measured.
                        ack_timeout_ms: $ack_timeout_ms:expr,
                        command_timeouts: $command_timeouts:expr,
                        inquiry_timeout_ms: $inquiry_timeout_ms:expr,
                        cancellation_timeout_ms: $cancellation_timeout_ms:expr,
                        ambiguity_timeout_ms: $ambiguity_timeout_ms:expr,
                        busy_timeout_ms: $busy_timeout_ms:expr,
                        inquiry_support: $inquiry_support:expr,
                        supports_operation_complete: $supports_operation_complete:expr,
                        supports_command_cancel: $supports_command_cancel:expr,
                        maximum_command_sockets: $maximum_command_sockets:expr,
                        position_inquiries: {
                            pan_tilt: $pan_tilt_position_inquiry:expr,
                            zoom: $zoom_position_inquiry:expr,
                            focus: $focus_position_inquiry:expr,
                            iris: $iris_position_inquiry:expr,
                            nd_filter: $nd_filter_position_inquiry:expr $(,)?
                        },
                        preset_recall_axes: $preset_recall_axes:expr,
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
                        wire_codec: $pan_tilt_wire_codec:expr,
                    },
                    zoom: {
                        optical_max: $optical_zoom_max:expr,
                        digital_max: $digital_zoom_max:expr,
                        speed_range: $zoom_speed_range:expr,
                        supports_direct: $supports_direct_zoom:expr,
                        supports_variable: $supports_variable_zoom:expr,
                        optical_zoom_ratio: $optical_zoom_ratio:expr,
                    },
                    focus: {
                        near_limit: $focus_near_limit:expr,
                        far_limit: $focus_far_limit:expr,
                        auto_focus: $supports_auto_focus:expr,
                        one_push: $supports_one_push_focus:expr,
                        focus_zone_inquiry: $supports_focus_zone_inquiry:expr,
                        zones: $focus_zones:expr,
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
                        exposure_comp_range: $exposure_comp_range:expr,
                        wdr: $supports_wdr:expr,
                    },
                    white_balance: {
                        modes: $wb_modes:expr,
                        one_push: $supports_one_push_wb:expr,
                        rg_tuning_range: $rg_tuning_range:expr,
                        bg_tuning_range: $bg_tuning_range:expr,
                        color_temp_range: $color_temp_range:expr,
                        red_gain_range: $red_gain_range:expr,
                        blue_gain_range: $blue_gain_range:expr,
                    },
                    image: {
                        // Source-backed permission for the base `image()` noun.
                        // This is distinct from the metadata facts below: every
                        // profile implements `ImageProcessing` for discovery.
                        base_support: $supports_image_processing:tt,
                        contrast_range: $contrast_range:expr,
                        sharpness_range: $sharpness_range:expr,
                        saturation_range: $saturation_range:expr,
                        flip: $supports_flip:expr,
                        mirror: $supports_mirror:expr,
                        hue_range: $hue_range:expr,
                        nr_2d: $supports_2d_nr:expr,
                        nr_3d: $supports_3d_nr:expr,
                        picture_effect: $supports_picture_effect:expr,
                        luminance_range: $luminance_range:expr,
                        combined_flip: $uses_combined_flip_command:expr,
                        save_after_flip: $requires_settings_save_for_flip:expr,
                        gamma_range: $gamma_range:expr,
                    },
                    presets: {
                        highest: $highest_preset:expr,
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
                        speed_range: $motion_sync_speed_range:expr $(,)?
                    },
                    nd_filter: {
                        mode: $nd_mode:expr,
                        steps: $nd_steps:expr $(,)?
                    },
                    variable_speed: {
                        supported: $supports_variable_speed:expr $(,)?
                    },
                    $(usb_audio: {
                        supported: $supports_usb_audio:expr $(,)?
                    },)?
                    typed_support: [$( $support:ident ),* $(,)?],
                    withheld: [$( $withheld:ident ),* $(,)?],
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
                const COMMAND_TIMEOUTS: $crate::CommandTimeouts = $command_timeouts;
                const BUSY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($busy_timeout_ms);
                const INQUIRY_SUPPORT: $crate::capabilities::InquirySupport = $inquiry_support;
                const SUPPORTS_OPERATION_COMPLETE: bool = $supports_operation_complete;
                const SUPPORTS_COMMAND_CANCEL: bool = $supports_command_cancel;
                const MIN_INQUIRY_SPACING: std::time::Duration =
                    std::time::Duration::from_millis($min_inquiry_spacing_ms);
                const MIN_COMMAND_SPACING: std::time::Duration =
                    std::time::Duration::from_millis($min_command_spacing_ms);
                const SUPPORTS_USB_AUDIO: bool =
                    __optional_support_bool!($($supports_usb_audio)?);
            }

            impl $crate::capabilities::PanTilt for $profile {
                const PAN_RANGE: $crate::capabilities::CapabilityRange<i32> = $pan_range;
                const TILT_RANGE: $crate::capabilities::CapabilityRange<i32> = $tilt_range;
                const MAX_PAN_SPEED: u8 = $max_pan_speed;
                const MAX_TILT_SPEED: u8 = $max_tilt_speed;
                const PAN_TILT_SIMULTANEOUS: bool = $pan_tilt_simultaneous;
                const PRESET_RECOVERY_TIME: std::time::Duration =
                    std::time::Duration::from_millis($preset_recovery_ms);
                const PAN_DEGREES_TO_UNITS: f32 = $pan_degrees_to_units;
                const TILT_DEGREES_TO_UNITS: f32 = $tilt_degrees_to_units;
                const COORDINATE_SYSTEM: $crate::capabilities::CoordinateSystem = $coordinate_system;
                const PAN_TILT_WIRE_CODEC: $crate::capabilities::PanTiltWireCodec =
                    $pan_tilt_wire_codec;
            }

            impl $crate::capabilities::Zoom for $profile {
                const OPTICAL_ZOOM_MAX: u16 = $optical_zoom_max;
                const DIGITAL_ZOOM_MAX: Option<u16> = $digital_zoom_max;
                const ZOOM_SPEED_RANGE: $crate::capabilities::CapabilityRange<u8> = $zoom_speed_range;
                const SUPPORTS_DIRECT_ZOOM: bool = $supports_direct_zoom;
                const SUPPORTS_VARIABLE_ZOOM: bool = $supports_variable_zoom;
                const OPTICAL_ZOOM_RATIO: Option<f32> = $optical_zoom_ratio;
            }

            impl $crate::capabilities::Focus for $profile {
                const FOCUS_NEAR_LIMIT: u16 = $focus_near_limit;
                const FOCUS_FAR_LIMIT: u16 = $focus_far_limit;
                const SUPPORTS_AUTO_FOCUS: bool = $supports_auto_focus;
                const SUPPORTS_ONE_PUSH_FOCUS: bool = $supports_one_push_focus;
                const SUPPORTS_FOCUS_ZONE_INQUIRY: bool = $supports_focus_zone_inquiry;
                const FOCUS_ZONES: &'static [$crate::command::FocusZone] = $focus_zones;
                const MAX_FOCUS_SPEED: u8 = $max_focus_speed;
                const SUPPORTS_AF_SENSITIVITY: bool = $supports_af_sensitivity;
                const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool =
                    $supports_focus_near_limit_inquiry;
            }

            impl $crate::capabilities::Exposure for $profile {
                const EXPOSURE_MODES: &'static [$crate::command::exposure::ExposureMode] =
                    $exposure_modes;
                const IRIS_RANGE: Option<$crate::capabilities::CapabilityDomain<u16>> = $iris_range;
                const SHUTTER_SPEEDS: &'static [$crate::capabilities::ShutterSpeedEntry] =
                    $shutter_speeds;
                const GAIN_RANGE: $crate::capabilities::CapabilityRange<u8> = $gain_range;
                const BRIGHTNESS_RANGE: Option<$crate::capabilities::CapabilityDomain<u8>> = $brightness_range;
                const SUPPORTS_BACKLIGHT_COMP: bool = $supports_backlight_comp;
                const EXPOSURE_COMP_RANGE: Option<$crate::capabilities::CapabilityRange<i8>> = $exposure_comp_range;
                const SUPPORTS_WDR: bool = $supports_wdr;
            }

            impl $crate::capabilities::WhiteBalance for $profile {
                const WB_MODES: &'static [$crate::WhiteBalanceMode] = $wb_modes;
                const SUPPORTS_ONE_PUSH_WB: bool = $supports_one_push_wb;
                const RG_TUNING_RANGE: Option<$crate::capabilities::CapabilityRange<i8>> = $rg_tuning_range;
                const BG_TUNING_RANGE: Option<$crate::capabilities::CapabilityRange<i8>> = $bg_tuning_range;
                const COLOR_TEMP_RANGE: Option<$crate::capabilities::CapabilityRange<u16>> = $color_temp_range;
                const RED_GAIN_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $red_gain_range;
                const BLUE_GAIN_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $blue_gain_range;
            }

            impl $crate::capabilities::ImageProcessing for $profile {
                const CONTRAST_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $contrast_range;
                const SHARPNESS_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $sharpness_range;
                const SATURATION_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $saturation_range;
                const SUPPORTS_FLIP: bool = $supports_flip;
                const SUPPORTS_MIRROR: bool = $supports_mirror;
                const HUE_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $hue_range;
                const SUPPORTS_2D_NR: bool = $supports_2d_nr;
                const SUPPORTS_3D_NR: bool = $supports_3d_nr;
                const SUPPORTS_PICTURE_EFFECT: bool = $supports_picture_effect;
                const LUMINANCE_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $luminance_range;
                const USES_COMBINED_FLIP_COMMAND: bool = $uses_combined_flip_command;
                const REQUIRES_SETTINGS_SAVE_FOR_FLIP: bool = $requires_settings_save_for_flip;
                const GAMMA_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $gamma_range;
                const SUPPORTS_IMAGE_PROCESSING: bool = $supports_image_processing;
            }

            impl $crate::capabilities::Presets for $profile {
                const HIGHEST_PRESET: u8 = $highest_preset;
                const PRESET_SPEED_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $preset_speed_range;
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
                const MOTION_SYNC_SPEED_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> =
                    $motion_sync_speed_range;
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

            impl $crate::profile::CompileTimeProfile for $profile {
                const TRANSPORTS: $crate::profile::TransportCompatibility =
                    $crate::profile::TransportCompatibility::new(
                        __transport_default_port!($tcp),
                        __transport_default_port!($udp),
                        __transport_is_supported!($serial),
                    );
                const INQUIRY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($inquiry_timeout_ms);
                const CANCELLATION_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($cancellation_timeout_ms);
                const AMBIGUITY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($ambiguity_timeout_ms);
                const MAXIMUM_COMMAND_SOCKETS: u8 = $maximum_command_sockets;
                const PRESET_RECALL_AXES: Option<$crate::AffectedAxes> = Some($preset_recall_axes);
                const POSITION_INQUIRIES: $crate::profile::PositionInquirySupport =
                    $crate::profile::PositionInquirySupport::new_with_iris_nd(
                        $pan_tilt_position_inquiry,
                        $zoom_position_inquiry,
                        $focus_position_inquiry,
                        $iris_position_inquiry,
                        $nd_filter_position_inquiry,
                    );
            }

            __impl_supports_tcp!($profile, $tcp);
            __impl_supports_udp!($profile, $udp);
            __impl_supports_serial!($profile, $serial);
            __impl_image_processing_marker!($supports_image_processing for $profile);
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
                    group: ProfileId::$id.profile_group(),
                    type_name: stringify!($profile),
                    display_name: $model_name,
                    vendor: $vendor,
                    description: $description,
                    envelope: if ProfileId::$id.uses_sony_encapsulation() {
                        $crate::ProfileEnvelope::SonyEncapsulated
                    } else {
                        $crate::ProfileEnvelope::RawVisca
                    },
                    tcp: __transport_default_port!($tcp),
                    udp: __transport_default_port!($udp),
                    serial: __transport_is_supported!($serial),
                    default_camera_id: $default_camera_id,
                    inquiry_support: $inquiry_support,
                    position_inquiries: <$profile as $crate::profile::CompileTimeProfile>::POSITION_INQUIRIES,
                    image_base_support: $supports_image_processing,
                    focus_zone_inquiry: $supports_focus_zone_inquiry,
                    usb_audio: __optional_support_bool!($($supports_usb_audio)?),
                    typed_support: <$profile as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT,
                    withheld: $crate::capabilities::TypedSupportSet::from_surfaces(&[
                        $($crate::capabilities::TypedSupportSurface::$withheld,)*
                    ]),
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
            /// Returns whether the runtime profile carries this built-in
            /// profile's complete protocol identity.
            ///
            /// `profile_id` is public so downstream callers can mutate a
            /// discovered inventory.  It is therefore an identity claim,
            /// not an authority token; vendor-specific request validation
            /// must only rely on it after every protocol identity fact has
            /// been checked against the generated profile row.
            pub(crate) fn matches_profile_facts(
                &self,
                facts: &$crate::profile::ProfileFacts,
            ) -> bool {
                match self {
                    $(ProfileId::$id => facts.matches_compile_time::<$profile>(),)*
                }
            }

            /// Returns the public compile-time type name for this profile.
            pub(crate) const fn compile_time_type_name(&self) -> &'static str {
                match self {
                    $(ProfileId::$id => stringify!($profile),)*
                }
            }

            /// Returns the typed-support set the current registry grants.
            pub(crate) const fn registry_typed_support(
                &self,
            ) -> $crate::capabilities::TypedSupportSet {
                match self {
                    $(ProfileId::$id => {
                        <$profile as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT
                    })*
                }
            }

            /// Returns the focus-zone values this built-in profile admits, for
            /// the generated documentation table check.
            #[cfg(test)]
            pub(crate) fn focus_zones(&self) -> &'static [$crate::command::FocusZone] {
                match self {
                    $(ProfileId::$id => <$profile as $crate::capabilities::Focus>::FOCUS_ZONES,)*
                }
            }

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
            ///
            /// This is the profile's envelope type: only the Sony envelope
            /// carries sequence correlation.
            pub const fn uses_sony_encapsulation(&self) -> bool {
                match self {
                    $(ProfileId::$id => {
                        <$envelope as $crate::transport::Envelope>::SUPPORTS_SEQUENCE_CORRELATION
                    })*
                }
            }

            /// Returns the profile group for this camera profile.
            pub const fn profile_group(&self) -> ProfileGroup {
                match self {
                    $($(ProfileId::$group_profile => ProfileGroup::$group,)*)*
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

            /// Returns whether every profile in this group uses Sony
            /// encapsulation.
            pub const fn uses_sony_encapsulation(&self) -> bool {
                self.every_member(profile_registry::MemberFact::SonyEncapsulation)
            }

            /// Returns whether every profile in this group supports TCP
            /// transport.
            pub const fn supports_tcp(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Tcp)
            }

            /// Returns whether every profile in this group supports UDP
            /// transport.
            pub const fn supports_udp(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Udp)
            }

            /// Returns whether every profile in this group supports serial
            /// transport.
            pub const fn supports_serial(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Serial)
            }

            /// Returns the VISCA inquiry support every profile in this group
            /// has: the weakest level among its members.
            pub const fn inquiry_support(&self) -> $crate::capabilities::InquirySupport {
                let members = self.profiles();
                let mut weakest = $crate::capabilities::InquirySupport::Full;
                let mut index = 0;
                while index < members.len() {
                    weakest = profile_registry::weaker_inquiry_support(
                        weakest,
                        members[index].inquiry_support(),
                    );
                    index += 1;
                }
                weakest
            }

            /// Group facts are derived from the member rows, so they cannot
            /// disagree with them.
            const fn every_member(&self, fact: profile_registry::MemberFact) -> bool {
                let members = self.profiles();
                let mut index = 0;
                while index < members.len() {
                    let member = members[index];
                    let holds = match fact {
                        profile_registry::MemberFact::SonyEncapsulation => {
                            member.uses_sony_encapsulation()
                        }
                        profile_registry::MemberFact::Tcp => member.supports_tcp(),
                        profile_registry::MemberFact::Udp => member.supports_udp(),
                        profile_registry::MemberFact::Serial => member.supports_serial(),
                    };
                    if !holds {
                        return false;
                    }
                    index += 1;
                }
                true
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
            surface.marker_trait_name()
        }
        #[cfg(test)]
        mod registry_tests {
            use super::*;

            fn assert_profile_registry<P>(
                facts: &profile_registry::BuiltinProfileFacts,
                id: ProfileId,
            )
            where
                P: $crate::profile::CompileTimeProfile + Default,
            {
                let caps = $crate::capabilities::Capabilities::from_profile::<P>();
                assert_eq!(caps.profile_id, P::PROFILE_ID);
                let spec = $crate::ProfileSpec::from_compile_time::<P>()
                    .unwrap_or_else(|error| panic!("{id:?} ProfileSpec failed: {error}"));

                assert_eq!(spec.capabilities(), &caps);
                assert_eq!(
                    spec.pan_tilt_coordinates().map(|conversion| (
                        conversion.coordinate_system(),
                        conversion.wire_codec(),
                        conversion.pan_degrees_to_units(),
                        conversion.tilt_degrees_to_units(),
                    )),
                    Some((
                        P::COORDINATE_SYSTEM,
                        P::PAN_TILT_WIRE_CODEC,
                        P::PAN_DEGREES_TO_UNITS,
                        P::TILT_DEGREES_TO_UNITS,
                    ))
                );
                assert_eq!(spec.transports(), P::TRANSPORTS);
                assert_eq!(spec.preset_recall_axes(), P::PRESET_RECALL_AXES);
                assert_eq!(
                    spec.envelope(),
                    if <P::Envelope as $crate::transport::Envelope>::SUPPORTS_SEQUENCE_CORRELATION {
                        $crate::ProfileEnvelope::SonyEncapsulated
                    } else {
                        $crate::ProfileEnvelope::RawVisca
                    }
                );

                assert_eq!(facts.id, id);
                assert_eq!(facts.group, id.profile_group());
                assert_eq!(facts.display_name, P::MODEL_NAME);
                assert_eq!(facts.display_name, id.display_name());
                assert_eq!(facts.vendor, id.vendor());
                assert_eq!(facts.description, id.description());
                assert_eq!(facts.tcp, id.default_tcp_port());
                assert_eq!(facts.udp, id.default_udp_port());
                assert_eq!(facts.tcp, spec.transports().tcp_port());
                assert_eq!(facts.udp, spec.transports().udp_port());
                assert_eq!(facts.serial, spec.transports().supports_serial());
                assert_eq!(facts.tcp.is_some(), id.supports_tcp());
                assert_eq!(facts.udp.is_some(), id.supports_udp());
                assert_eq!(facts.serial, id.supports_serial());
                assert_eq!(facts.default_camera_id, P::DEFAULT_CAMERA_ID);
                assert_eq!(facts.default_camera_id, id.default_camera_id());
                assert_eq!(facts.inquiry_support, P::INQUIRY_SUPPORT);
                assert_eq!(facts.inquiry_support, id.inquiry_support());
                assert_eq!(facts.position_inquiries, P::POSITION_INQUIRIES);
                assert_eq!(spec.position_inquiries(), P::POSITION_INQUIRIES);
                assert_eq!(
                    facts.image_base_support,
                    P::SUPPORTS_IMAGE_PROCESSING,
                    "{id:?} image base-support registry fact and metadata must match"
                );
                assert_eq!(
                    caps.has_image_processing,
                    facts.image_base_support,
                    "{id:?} runtime image permission must come from the registry fact"
                );
                // These are the optional marker surfaces whose canonical rows
                // live under `camera.image()`. A row marker without the base
                // accessor would be publicly advertised but unreachable.
                for surface in [
                    $crate::capabilities::TypedSupportSurface::BacklightCompensation,
                    $crate::capabilities::TypedSupportSurface::ImageFlip,
                    $crate::capabilities::TypedSupportSurface::ImageMirror,
                    $crate::capabilities::TypedSupportSurface::CombinedImageFlip,
                    $crate::capabilities::TypedSupportSurface::ContrastControl,
                    $crate::capabilities::TypedSupportSurface::SharpnessControl,
                    $crate::capabilities::TypedSupportSurface::SaturationControl,
                    $crate::capabilities::TypedSupportSurface::HueControl,
                    $crate::capabilities::TypedSupportSurface::LuminanceControl,
                    $crate::capabilities::TypedSupportSurface::GammaControl,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3DControl,
                    $crate::capabilities::TypedSupportSurface::PictureEffect,
                    $crate::capabilities::TypedSupportSurface::ImageFreeze,
                    $crate::capabilities::TypedSupportSurface::DefogLevel,
                ] {
                    assert!(
                        facts.image_base_support || !facts.has_typed_support(surface),
                        "{id:?} exposes {surface:?} under image() without base image-noun support"
                    );
                }
                assert_eq!(
                    facts.focus_zone_inquiry,
                    P::SUPPORTS_FOCUS_ZONE_INQUIRY,
                    "{id:?} focus-zone inquiry registry fact and metadata must match"
                );
                assert_eq!(
                    caps.has_focus_zone_inquiry,
                    facts.focus_zone_inquiry,
                    "{id:?} runtime focus-zone inquiry fact must come from the registry"
                );
                assert_eq!(
                    facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::FocusZoneInquiry
                    ),
                    facts.focus_zone_inquiry,
                    "{id:?} focus-zone inquiry marker must follow its registry fact"
                );
                assert_eq!(
                    facts.usb_audio,
                    P::SUPPORTS_USB_AUDIO,
                    "{id:?} USB-audio registry fact and metadata must match"
                );
                assert_eq!(
                    caps.has_usb_audio,
                    facts.usb_audio,
                    "{id:?} runtime USB-audio fact must come from the registry"
                );
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::UsbAudio),
                    facts.usb_audio,
                    "{id:?} USB-audio marker must follow its registry fact"
                );
                assert!(
                    !facts.position_inquiries.iris()
                        || facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::IrisControl
                        ),
                    "{id:?} iris inquiry facts must be backed by typed-support evidence"
                );
                assert!(
                    !facts.position_inquiries.nd_filter()
                        || facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::NdFilter
                        ),
                    "{id:?} ND inquiry facts must be backed by typed-support evidence"
                );
                assert_eq!(
                    facts.typed_support,
                    <P as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT
                );
                assert_eq!(
                    facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::ExposureMode
                    ),
                    !P::EXPOSURE_MODES.is_empty(),
                    "{id:?} shared exposure-mode typed support and its source-backed mode inventory must agree"
                );
                // #822: the row states its envelope once, as a type. The
                // public flag and the lowered spec both derive from it.
                assert_eq!(facts.envelope, spec.envelope());
                assert_eq!(
                    id.uses_sony_encapsulation(),
                    spec.envelope() == $crate::ProfileEnvelope::SonyEncapsulated
                );
                assert!(P::PAN_RANGE.min() <= P::PAN_RANGE.max(), "{id:?} pan metadata must be non-empty");
                assert!(
                    P::TILT_RANGE.min() <= P::TILT_RANGE.max(),
                    "{id:?} tilt metadata must be non-empty"
                );
                assert!(
                    P::ZOOM_SPEED_RANGE.min() <= P::ZOOM_SPEED_RANGE.max(),
                    "{id:?} zoom-speed metadata must be non-empty"
                );
                if let Some(range) = P::IRIS_RANGE {
                    assert!(range.min() <= range.max(), "{id:?} iris metadata must be non-empty");
                }
                assert!(
                    P::GAIN_RANGE.min() <= P::GAIN_RANGE.max(),
                    "{id:?} gain metadata must be non-empty"
                );
                if let Some(range) = P::BRIGHTNESS_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} exposure brightness metadata must be non-empty"
                    );
                }
                if let Some(range) = P::EXPOSURE_COMP_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} exposure-compensation metadata must be non-empty"
                    );
                }
                if let Some(range) = P::RG_TUNING_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} red tuning metadata must be non-empty"
                    );
                }
                if let Some(range) = P::BG_TUNING_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} blue tuning metadata must be non-empty"
                    );
                }
                if let Some(range) = P::COLOR_TEMP_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} color-temperature metadata must be non-empty"
                    );
                }
                if let Some(range) = P::RED_GAIN_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} red gain metadata must be non-empty"
                    );
                }
                if let Some(range) = P::BLUE_GAIN_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} blue gain metadata must be non-empty"
                    );
                }
                if let Some(range) = P::CONTRAST_RANGE {
                    assert!(range.min() <= range.max(), "{id:?} contrast metadata must be non-empty");
                }
                if let Some(range) = P::SHARPNESS_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} sharpness metadata must be non-empty"
                    );
                }
                if let Some(range) = P::SATURATION_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} saturation metadata must be non-empty"
                    );
                }
                if let Some(range) = P::HUE_RANGE {
                    assert!(range.min() <= range.max(), "{id:?} hue metadata must be non-empty");
                }
                if let Some(range) = P::LUMINANCE_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} luminance metadata must be non-empty"
                    );
                }
                if let Some(range) = P::GAMMA_RANGE {
                    assert!(range.min() <= range.max(), "{id:?} gamma metadata must be non-empty");
                }
                if let Some(range) = P::PRESET_SPEED_RANGE {
                    assert!(
                        range.min() <= range.max(),
                        "{id:?} preset-speed metadata must be non-empty"
                    );
                }
                // Marker-versus-metadata agreement for every surface is the
                // single rule in `typed_support_markers_match_dedicated_metadata`.

                assert_eq!(caps.model_name, P::MODEL_NAME);
                assert_eq!(caps.default_camera_id, P::DEFAULT_CAMERA_ID);
                assert_eq!(caps.pan_speed, 1..=P::MAX_PAN_SPEED);
                assert_eq!(caps.tilt_speed, 1..=P::MAX_TILT_SPEED);
                assert_eq!(caps.pan_range, P::PAN_RANGE.as_inclusive());
                assert_eq!(caps.tilt_range, P::TILT_RANGE.as_inclusive());
                assert_eq!(caps.pan_tilt_simultaneous, P::PAN_TILT_SIMULTANEOUS);
                assert_eq!(caps.preset_recovery_time, P::PRESET_RECOVERY_TIME);
                assert_eq!(caps.zoom_range_optical, 0..=P::OPTICAL_ZOOM_MAX);
                assert_eq!(
                    caps.zoom_range_digital,
                    P::DIGITAL_ZOOM_MAX.map(|max| P::OPTICAL_ZOOM_MAX..=max)
                );
                assert_eq!(caps.zoom_speed, P::ZOOM_SPEED_RANGE.as_inclusive());
                assert_eq!(caps.supports_direct_zoom, P::SUPPORTS_DIRECT_ZOOM);
                assert_eq!(caps.supports_variable_zoom, P::SUPPORTS_VARIABLE_ZOOM);
                assert_eq!(caps.has_auto_focus, P::SUPPORTS_AUTO_FOCUS);
                assert_eq!(caps.has_one_push_focus, P::SUPPORTS_ONE_PUSH_FOCUS);
                assert_eq!(caps.focus_range, P::FOCUS_NEAR_LIMIT..=P::FOCUS_FAR_LIMIT);
                assert_eq!(caps.focus_speed, 0..=P::MAX_FOCUS_SPEED);
                assert_eq!(caps.focus_zones, P::FOCUS_ZONES);
                assert_eq!(
                    caps.has_focus_zone_inquiry,
                    P::SUPPORTS_FOCUS_ZONE_INQUIRY
                );
                assert_eq!(caps.has_af_sensitivity, P::SUPPORTS_AF_SENSITIVITY);
                assert_eq!(
                    caps.has_focus_near_limit_inquiry,
                    P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY
                );
                assert_eq!(caps.exposure_modes, P::EXPOSURE_MODES);
                assert_eq!(caps.has_backlight_comp, P::SUPPORTS_BACKLIGHT_COMP);
                assert_eq!(caps.has_wdr, P::SUPPORTS_WDR);
                assert_eq!(
                    caps.iris_range,
                    P::IRIS_RANGE
                );
                assert_eq!(caps.gain_range, P::GAIN_RANGE.as_inclusive());
                assert_eq!(caps.shutter_speeds.len(), P::SHUTTER_SPEEDS.len());
                for (runtime, static_speed) in caps.shutter_speeds.iter().zip(P::SHUTTER_SPEEDS) {
                    assert_eq!(runtime.exposure, static_speed.exposure);
                    assert_eq!(runtime.value, static_speed.value);
                }
                assert_eq!(
                    caps.exposure_brightness_range,
                    P::BRIGHTNESS_RANGE
                );
                assert_eq!(
                    caps.exposure_comp_range,
                    P::EXPOSURE_COMP_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.has_one_push_wb, P::SUPPORTS_ONE_PUSH_WB);
                assert_eq!(
                    caps.color_temp_range,
                    P::COLOR_TEMP_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.rg_tuning_range,
                    P::RG_TUNING_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.bg_tuning_range,
                    P::BG_TUNING_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.red_gain_range,
                    P::RED_GAIN_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.blue_gain_range,
                    P::BLUE_GAIN_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.white_balance_modes, P::WB_MODES);
                assert_eq!(
                    caps.contrast_range,
                    P::CONTRAST_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.sharpness_range,
                    P::SHARPNESS_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.saturation_range,
                    P::SATURATION_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.hue_range,
                    P::HUE_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.luminance_range,
                    P::LUMINANCE_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.gamma_range,
                    P::GAMMA_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.supports_flip, P::SUPPORTS_FLIP);
                assert_eq!(caps.supports_mirror, P::SUPPORTS_MIRROR);
                assert_eq!(caps.uses_combined_flip_command, P::USES_COMBINED_FLIP_COMMAND);
                assert_eq!(caps.requires_settings_save_for_flip, P::REQUIRES_SETTINGS_SAVE_FOR_FLIP);
                assert_eq!(caps.has_2d_nr, P::SUPPORTS_2D_NR);
                assert_eq!(caps.has_3d_nr, P::SUPPORTS_3D_NR);
                assert_eq!(caps.has_picture_effect, P::SUPPORTS_PICTURE_EFFECT);
                assert_eq!(caps.has_tally, P::SUPPORTS_TALLY);
                assert_eq!(caps.highest_preset, P::HIGHEST_PRESET);
                assert_eq!(
                    caps.preset_speed_range,
                    P::PRESET_SPEED_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.supports_preset_tour, P::SUPPORTS_PRESET_TOUR);
                assert_eq!(caps.supports_preset_thumbnail, P::SUPPORTS_PRESET_THUMBNAIL);
                assert_eq!(caps.preset_recall_delay, P::PRESET_RECALL_DELAY);
                assert_eq!(caps.supports_preset_names, P::SUPPORTS_PRESET_NAMES);
                assert_eq!(caps.max_preset_name_length, P::MAX_PRESET_NAME_LENGTH);
                assert_eq!(caps.supports_standby, P::SUPPORTS_STANDBY);
                assert_eq!(caps.supports_wake_on_lan, P::SUPPORTS_WAKE_ON_LAN);
                assert_eq!(caps.power_on_time, P::POWER_ON_TIME);
                assert_eq!(caps.standby_time, P::STANDBY_TIME);
                assert_eq!(caps.retains_settings_on_power_off, P::RETAINS_SETTINGS_ON_POWER_OFF);
                assert_eq!(caps.home_on_power_up, P::HOME_ON_POWER_UP);
                assert_eq!(
                    caps.has_nd_filter,
                    !matches!(P::ND_MODE, $crate::capabilities::NdFilterMode::None)
                );
                assert_eq!(caps.nd_filter_mode, P::ND_MODE);
                assert_eq!(caps.nd_filter_steps, P::ND_STEPS);
                assert_eq!(
                    caps.motion_sync_speed_range,
                    P::MOTION_SYNC_SPEED_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.has_direct_menu_control, P::SUPPORTS_DIRECT_CONTROL);
                assert_eq!(caps.has_variable_speed, P::SUPPORTS_VARIABLE_SPEED);
                assert_eq!(caps.has_usb_audio, P::SUPPORTS_USB_AUDIO);
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
                    assert_eq!(id.profile_group(), facts.group);
                    assert_eq!(
                        id.uses_sony_encapsulation(),
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

            /// #818: the surface-to-metadata rule
            /// (`Capabilities::surface_metadata`) holds in both directions for
            /// every built-in row. A marker always needs its metadata (profile
            /// validation enforces that); a surface with its own metadata fact
            /// must also carry the marker whenever the metadata advertises it,
            /// unless the row lists the surface in `withheld` and its evidence
            /// says why.
            #[test]
            fn typed_support_markers_match_dedicated_metadata() {
                fn assert_row(id: ProfileId, caps: &$crate::capabilities::Capabilities) {
                    use $crate::capabilities::SurfaceMetadata::Dedicated;

                    let facts = id.registry_facts();
                    for surface in $crate::capabilities::TypedSupportSurface::ALL {
                        let marker = caps.supports_typed(surface);
                        let metadata = caps.surface_metadata(surface);
                        assert!(
                            !marker || metadata.permits(),
                            "{id:?} grants {surface:?} without its metadata"
                        );
                        let Dedicated(advertised) = metadata else {
                            assert!(
                                !facts.withheld.contains(surface),
                                "{id:?} withholds {surface:?}, which has no metadata of its own"
                            );
                            continue;
                        };
                        if facts.withheld.contains(surface) {
                            // An exemption is exact: the metadata advertises
                            // the surface, the marker is absent, and the
                            // evidence says why. A stale entry fails here.
                            assert!(
                                advertised && !marker,
                                "{id:?}: stale withheld {surface:?} (metadata {advertised}, marker {marker})"
                            );
                            assert!(
                                facts.evidence.iter().any(|item| item.key == surface.wire_name().replace('-', "_")),
                                "{id:?} withholds {surface:?} without a `{}` evidence entry",
                                surface.wire_name().replace('-', "_")
                            );
                        } else {
                            assert_eq!(
                                marker, advertised,
                                "{id:?}: {surface:?} marker disagrees with its metadata; fix the row or list the surface in `withheld`"
                            );
                        }
                    }
                }

                $(
                    assert_row(
                        ProfileId::$id,
                        $crate::ProfileSpec::from_compile_time::<$profile>()
                            .unwrap_or_else(|error| panic!("{} ProfileSpec failed: {error}", stringify!($profile)))
                            .capabilities(),
                    );
                )*
            }

            /// A STOP queued on a raw byte stream waits at most one ACK plus one
            /// ambiguity interval for the rules that keep its answers decisive
            /// (#795). For every built-in profile that bound is below half a
            /// halt's budget, which is at least the movement completion
            /// deadline, so a halt's STOPs are always written within it.
            #[test]
            fn stop_wait_bound_is_below_every_halt_budget() {
                $(
                    let ack = <$profile as $crate::capabilities::ProfileMetadata>::ACK_TIMEOUT;
                    let ambiguity =
                        <$profile as $crate::profile::CompileTimeProfile>::AMBIGUITY_TIMEOUT;
                    let movement = <$profile as $crate::capabilities::ProfileMetadata>::COMMAND_TIMEOUTS
                        .movement_timeout();
                    assert!(
                        (ack + ambiguity) * 2 <= movement,
                        "{}: STOP wait bound {:?} against halt budget {movement:?}",
                        stringify!($profile),
                        ack + ambiguity,
                    );
                )*
            }

            /// Every built-in unit conversion and shutter fact agrees with the
            /// profile tables (#807, #808, #819, #828 M2): the public pan/tilt
            /// helper equals the request path, every raw unit round-trips
            /// through degrees, each advertised shutter code is constructible
            /// and reachable by its exposure time, a profile's nominal optical
            /// ratio reaches the end of its optical range, and each Kelvin step
            /// round-trips.
            #[test]
            fn builtin_unit_conversions_agree_with_profile_tables() {
                use $crate::capabilities::pan_tilt::PanTiltExt;

                fn sweep(range: &std::ops::RangeInclusive<f32>) -> impl Iterator<Item = f32> + '_ {
                    let steps = ((range.end() - range.start()) / 0.05).floor() as i32;
                    (0..=steps).map(move |step| range.start() + step as f32 * 0.05)
                }

                fn assert_reachable(id: ProfileId, scale: $crate::capabilities::ZoomScale) {
                    let max = scale.optical_max();
                    let units = scale
                        .units(scale.optical_ratio())
                        .unwrap_or_else(|error| panic!("{id:?} nominal ratio: {error}"));
                    assert!(
                        units <= max && units >= max - 1,
                        "{id:?} {}x maps to {units:#06X}, not the optical end {max:#06X}",
                        scale.optical_ratio()
                    );
                }

                fn assert_profile<P>(id: ProfileId)
                where
                    P: $crate::profile::CompileTimeProfile + Default,
                {
                    let camera = P::default();
                    let spec = $crate::ProfileSpec::from_compile_time::<P>()
                        .unwrap_or_else(|error| panic!("{id:?} ProfileSpec failed: {error}"));
                    let caps = spec.capabilities();
                    let conversion = spec.pan_tilt_coordinates().expect("pan/tilt conversion");

                    for pan in sweep(&caps.pan_range_degrees) {
                        let expected = camera
                            .degrees_to_pan_units(pan)
                            .zip(camera.degrees_to_tilt_units(0.0))
                            .filter(|(pan, tilt)| {
                                caps.pan_range.contains(pan) && caps.tilt_range.contains(tilt)
                            });
                        assert_eq!(
                            spec.convert_pan_tilt_degrees(pan, 0.0).ok(),
                            expected,
                            "{id:?} pan {pan}°"
                        );
                    }
                    for tilt in sweep(&caps.tilt_range_degrees) {
                        let expected = camera
                            .degrees_to_pan_units(0.0)
                            .zip(camera.degrees_to_tilt_units(tilt))
                            .filter(|(pan, tilt)| {
                                caps.pan_range.contains(pan) && caps.tilt_range.contains(tilt)
                            });
                        assert_eq!(
                            spec.convert_pan_tilt_degrees(0.0, tilt).ok(),
                            expected,
                            "{id:?} tilt {tilt}°"
                        );
                    }
                    for units in caps.pan_range.clone() {
                        assert_eq!(
                            conversion.pan_units(conversion.pan_degrees(units)),
                            Some(units),
                            "{id:?} pan unit {units}"
                        );
                    }
                    for units in caps.tilt_range.clone() {
                        assert_eq!(
                            conversion.tilt_units(conversion.tilt_degrees(units)),
                            Some(units),
                            "{id:?} tilt unit {units}"
                        );
                    }

                    assert_eq!(caps.optical_zoom_ratio, P::OPTICAL_ZOOM_RATIO, "{id:?}");
                    match (P::OPTICAL_ZOOM_RATIO, caps.zoom_scale()) {
                        (Some(ratio), Ok(scale)) => {
                            assert_eq!(scale.optical_ratio(), ratio, "{id:?}");
                            assert_reachable(id, scale);
                        }
                        (None, Err($crate::Error::InvalidRequest(message))) => assert!(
                            message.contains("zoom_scale_for_lens"),
                            "{id:?}: {message}"
                        ),
                        (ratio, scale) => panic!("{id:?}: ratio {ratio:?} gave {scale:?}"),
                    }

                    for entry in &caps.shutter_speeds {
                        let code = $crate::types::ShutterSpeed::new(entry.value);
                        assert_eq!(
                            caps.shutter_speed_for(entry.exposure).ok(),
                            Some(code),
                            "{id:?} {}",
                            entry.exposure
                        );
                    }

                    if let Some(range) = &caps.color_temp_range {
                        for kelvin in (*range.start()..=*range.end()).step_by(100) {
                            let color_temp = $crate::types::ColorTemp::from_kelvin(kelvin)
                                .unwrap_or_else(|error| panic!("{id:?} {kelvin} K: {error}"));
                            assert_eq!(color_temp.to_kelvin(), kelvin, "{id:?}");
                        }
                    }
                }

                $(
                    assert_profile::<$profile>(ProfileId::$id);
                )*

                // The PTZOptics G2 family's lens is a camera fact: each bench
                // lens (PT12X/PT20X/PT30X-NDI G2, §4.1) reaches its optical end.
                let g2 = $crate::capabilities::Capabilities::from_profile::<PtzOpticsG2>();
                assert_eq!(g2.optical_zoom_ratio, None);
                for ratio in [12.0, 20.0, 30.0] {
                    let scale = g2.zoom_scale_for_lens(ratio).expect("G2 lens");
                    assert_reachable(ProfileId::PtzOpticsG2, scale);
                }
            }

            /// The built-in registry's `image.base_support` fact emits both the
            /// runtime permission and the static `HasImageProcessing` marker.
            /// Negative marker assertions live in public compile-contract
            /// fixtures, because Rust has no stable negative-trait assertion.
            #[test]
            fn image_processing_markers_follow_base_support() {
                fn assert_image_processing_marker<P: $crate::capabilities::HasImageProcessing>() {}

                $(
                    __assert_image_processing_marker!(
                        $supports_image_processing for $profile,
                        assert_image_processing_marker
                    );
                )*
            }

            /// No built-in profile has a hardware-measured acknowledgement or
            /// inquiry-response deadline yet (#689), so every row must name
            /// the shared interim values rather than a literal of its own. A
            /// row that needs another value must cite a measurement.
            #[test]
            fn builtin_deadlines_are_the_interim_values_until_measured() {
                let ack = std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS);
                let inquiry =
                    std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS);
                $(
                    let timing = $crate::ProfileSpec::from_compile_time::<$profile>()
                        .unwrap_or_else(|error| {
                            panic!("{} ProfileSpec failed: {error}", stringify!($profile))
                        })
                        .timing();
                    assert_eq!(timing.ack_timeout(), ack, "{}", stringify!($profile));
                    assert_eq!(timing.inquiry_timeout(), inquiry, "{}", stringify!($profile));
                )*
            }

            /// A scalar settlement inquiry exists exactly where its typed
            /// surface is granted: iris with `IrisControl`, ND with `NdFilter`.
            #[test]
            fn scalar_position_inquiries_follow_their_typed_surfaces() {
                for facts in BUILTIN_PROFILE_FACTS {
                    assert_eq!(
                        facts.position_inquiries.iris(),
                        facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::IrisControl
                        ),
                        "{:?} iris inquiry",
                        facts.id
                    );
                    assert_eq!(
                        facts.position_inquiries.nd_filter(),
                        facts.has_typed_support($crate::capabilities::TypedSupportSurface::NdFilter),
                        "{:?} ND-filter inquiry",
                        facts.id
                    );
                }
            }

            #[test]
            fn sony_fr7_has_no_shared_ae_mode_inventory() {
                let profile = $crate::ProfileSpec::from_compile_time::<$crate::profiles::SonyFR7>()
                    .expect("Sony FR7 profile");
                let capabilities = profile.capabilities();
                assert!(capabilities.has_exposure);
                assert!(capabilities.exposure_modes.is_empty());
                assert!(!capabilities.supports_typed(
                    $crate::capabilities::TypedSupportSurface::ExposureMode
                ));
                for mode in [
                    $crate::command::exposure::ExposureMode::Auto,
                    $crate::command::exposure::ExposureMode::Manual,
                    $crate::command::exposure::ExposureMode::Shutter,
                    $crate::command::exposure::ExposureMode::Iris,
                    $crate::command::exposure::ExposureMode::Bright,
                ] {
                    assert!(!capabilities.supports_exposure_mode(mode));
                }
            }

            #[test]
            fn shared_exposure_mode_support_is_exact_and_inventory_backed() {
                // `assert_profile_registry` ties each row's marker to its
                // mode inventory; this pins which rows have one.
                for facts in BUILTIN_PROFILE_FACTS {
                    let expected = facts.id != ProfileId::SonyFr7;
                    assert_eq!(
                        facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::ExposureMode
                        ),
                        expected,
                        "{:?} shared exposure-mode support must exactly follow the source-backed mode inventory",
                        facts.id
                    );
                }
            }

            #[test]
            fn typed_support_registry_has_evidence_for_surprising_gaps() {
                for facts in BUILTIN_PROFILE_FACTS {
                    assert!(
                        !facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::IrisControlInquiry,
                        ),
                        "{} must not expose `09 04 2B` iris status inquiry without model-specific evidence",
                        facts.type_name,
                    );
                    if !facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::IrisControl,
                    ) {
                        assert!(
                            !facts.position_inquiries.iris(),
                            "{} must not expose a typed iris settlement inquiry without typed iris support",
                            facts.type_name
                        );
                        assert!(
                            facts.evidence.iter().any(|item| item.key == "iris"),
                            "{} must document why iris typed support is absent",
                            facts.type_name
                        );
                    }
                    if !facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::NdFilter,
                    ) {
                        assert!(
                            !facts.position_inquiries.nd_filter(),
                            "{} must not expose an ND settlement inquiry without typed ND support",
                            facts.type_name
                        );
                    }
                }

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
                }
            }

            #[test]
            fn ptzoptics_profiles_follow_consolidated_reference_boundaries() {
                fn assert_ptzoptics_raw_profile<P>()
                where
                    P: $crate::profile::CompileTimeProfile
                        + $crate::capabilities::SupportsTcp
                        + $crate::capabilities::SupportsUdp
                        + $crate::capabilities::SupportsSerial
                        + Default,
                {
                    assert_eq!(
                        P::TRANSPORTS,
                        $crate::profile::TransportCompatibility::new(Some(5678), Some(1259), true)
                    );
                    assert_eq!(P::PAN_RANGE, range!(i32, -2448, 2448));
                    assert_eq!(P::TILT_RANGE, range!(i32, -432, 1296));
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
                    assert!(!P::FOCUS_ZONES.is_empty());
                    assert!(!P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY);
                    assert_eq!(P::HIGHEST_PRESET, 127);
                    assert!(P::SUPPORTS_PICTURE_EFFECT);
                    assert!(!P::SUPPORTS_PRESET_TOUR);
                }

                assert_ptzoptics_raw_profile::<PtzOpticsG2>();
                assert_ptzoptics_raw_profile::<PtzOpticsG3>();
                assert_ptzoptics_raw_profile::<PtzOptics30X>();

                // #828 L7: no PTZOptics source documents socket cancel for any
                // of the three profiles, so each records that and keeps it
                // unavailable.
                assert!(
                    !<PtzOpticsG2 as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                assert!(
                    !<PtzOpticsG3 as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                assert!(
                    !<PtzOptics30X as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30X,
                ] {
                    assert!(
                        id.registry_facts()
                            .evidence
                            .iter()
                            .any(|item| item.key == "command_cancel"),
                        "{id:?} must record why socket cancel is unavailable"
                    );
                }

                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30X,
                ] {
                    let facts = id.registry_facts();
                    assert!(facts.has_typed_support($crate::capabilities::TypedSupportSurface::FocusZone));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::IrisControl));
                    assert!(!facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::IrisControlInquiry
                    ));
                    assert!(facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::ExposureMode
                    ));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::PictureEffect));
                    assert!(!facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry
                    ));
                    for surface in [
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3DControl,
                    ] {
                        assert!(facts.has_typed_support(surface), "{id:?} {surface:?}");
                    }
                }

                const R14_NR_EVIDENCE: &str = "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain.";
                for id in [ProfileId::PtzOpticsG2, ProfileId::PtzOpticsG3] {
                    let evidence = id
                        .registry_facts()
                        .evidence
                        .iter()
                        .find(|item| item.key == "noise_reduction")
                        .expect("G2 and G3 must record the R14 NR evidence");
                    assert_eq!(evidence.rationale, R14_NR_EVIDENCE, "{id:?}");
                }
                let legacy_30x_evidence = ProfileId::PtzOptics30X
                    .registry_facts()
                    .evidence
                    .iter()
                    .find(|item| item.key == "noise_reduction")
                    .expect("legacy 30X must record separate inquiry and control sources");
                assert_eq!(
                    legacy_30x_evidence.rationale,
                    "R1 supplies legacy PT30X SDI/NDI G2 09 04 50/53/54 inquiries, including 3D 0..8, but no setters. R14 supplies 04 50/53/54 controls only because R15 explicitly narrows this profile to that legacy PT30X SDI/NDI G2 raw-VISCA family. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by query output domains; do not generalize this evidence to newer 30X products."
                );
                // R8 documents only the EVI-H100 `04 53` 2D level control and
                // inquiry; the `04 50` mode and the 3D level stay PTZOptics.
                let ptzoptics = "`PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`";
                let with_evi = "`PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyEVIH100`";
                for (surface, profiles) in [
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2D, with_evi),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2DControl, with_evi),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2DMode, ptzoptics),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction3D, ptzoptics),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction3DControl, ptzoptics),
                ] {
                    assert_eq!(registry_profile_type_names_for(surface), profiles, "{surface:?}");
                }

                for (id, focus_zone_inquiry, usb_audio) in [
                    (ProfileId::PtzOpticsG2, true, true),
                    (ProfileId::PtzOpticsG3, false, false),
                    (ProfileId::PtzOptics30X, true, true),
                ] {
                    let facts = id.registry_facts();
                    assert_eq!(facts.focus_zone_inquiry, focus_zone_inquiry, "{id:?}");
                    assert_eq!(
                        facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::FocusZoneInquiry
                        ),
                        focus_zone_inquiry,
                        "{id:?} focus-zone inquiry marker"
                    );
                    assert_eq!(facts.usb_audio, usb_audio, "{id:?}");
                    assert_eq!(
                        facts.has_typed_support($crate::capabilities::TypedSupportSurface::UsbAudio),
                        usb_audio,
                        "{id:?} USB-audio marker"
                    );
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
                        $crate::capabilities::TypedSupportSurface::PtzOpticsSnapFocus
                            | $crate::capabilities::TypedSupportSurface::MotionSync
                            | $crate::capabilities::TypedSupportSurface::IrisControlInquiry
                            | $crate::capabilities::TypedSupportSurface::AutoFocusSensitivity
                            | $crate::capabilities::TypedSupportSurface::ImageFreeze
                            | $crate::capabilities::TypedSupportSurface::DefogLevel
                            | $crate::capabilities::TypedSupportSurface::TallyBrightness
                            | $crate::capabilities::TypedSupportSurface::PtzOpticsTally
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
                assert_row(
                    readme,
                    "PTZOptics anti-flicker control and inquiry",
                    $crate::capabilities::TypedSupportSurface::PtzOpticsAntiFlicker,
                );
                assert_row(
                    readme,
                    "PTZOptics settings-save command",
                    $crate::capabilities::TypedSupportSurface::PtzOpticsSettingsSave,
                );
                assert_row(
                    readme,
                    "PTZOptics preset-recall speed control",
                    $crate::capabilities::TypedSupportSurface::PtzOpticsPresetRecallSpeed,
                );
                assert_row(
                    readme,
                    "Sony spotlight controls",
                    $crate::capabilities::TypedSupportSurface::SonySpotlight,
                );
                assert_row(
                    readme,
                    "Sony automatic slow-shutter controls",
                    $crate::capabilities::TypedSupportSurface::SonyAutoSlowShutter,
                );
                assert_row(
                    readme,
                    "PTZOptics multicast-streaming controls",
                    $crate::capabilities::TypedSupportSurface::PtzOpticsMulticastStreaming,
                );
                assert_row(
                    readme,
                    "PTZOptics NDI-quality control",
                    $crate::capabilities::TypedSupportSurface::PtzOpticsNdiQuality,
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
                    "Standard iris reset/up/down/direct control and `09 04 4B` iris-position inquiry",
                    $crate::capabilities::TypedSupportSurface::IrisControl,
                );
                assert_literal_row(
                    readme,
                    "Standard `09 04 2B` iris auto/manual-status inquiry (`iris_control()`)",
                    "No built-in profile currently marks this typed capability",
                );
                assert_row(
                    readme,
                    "Shared VISCA exposure mode control and inquiry (including `ExposureMode::Iris`)",
                    $crate::capabilities::TypedSupportSurface::ExposureMode,
                );
                assert_row(
                    readme,
                    "Standard one-push focus",
                    $crate::capabilities::TypedSupportSurface::OnePushFocus,
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
                    "Focus zone control",
                    $crate::capabilities::TypedSupportSurface::FocusZone,
                );
                assert_row(
                    readme,
                    "Focus zone inquiry",
                    $crate::capabilities::TypedSupportSurface::FocusZoneInquiry,
                );
                assert_literal_row(
                    readme,
                    "Auto focus sensitivity",
                    "No built-in profile currently marks this typed capability",
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
                    "Color temperature controls",
                    $crate::capabilities::TypedSupportSurface::ColorTemperature,
                );
                assert_row(
                    readme,
                    "Color temperature inquiry (sourced one-byte `pq` reply)",
                    $crate::capabilities::TypedSupportSurface::ColorTemperatureInquiry,
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
                assert_row(
                    readme,
                    "Vertical image flip control and inquiry",
                    $crate::capabilities::TypedSupportSurface::ImageFlip,
                );
                assert_row(
                    readme,
                    "Horizontal image mirror control and inquiry",
                    $crate::capabilities::TypedSupportSurface::ImageMirror,
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
                    "2D noise-reduction level inquiry",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                );
                assert_row(
                    readme,
                    "2D noise-reduction level control",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl,
                );
                assert_row(
                    readme,
                    "2D noise-reduction auto/manual mode control and inquiry",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2DMode,
                );
                assert_row(
                    readme,
                    "3D noise-reduction level inquiry",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                );
                assert_row(
                    readme,
                    "3D noise-reduction level control",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3DControl,
                );
                assert_row(
                    readme,
                    "Picture effects",
                    $crate::capabilities::TypedSupportSurface::PictureEffect,
                );
                assert_row(
                    readme,
                    "USB audio control and inquiry",
                    $crate::capabilities::TypedSupportSurface::UsbAudio,
                );
            }

            #[test]
            fn camera_profile_support_vocabulary_table_is_generated_from_the_registry() {
                const BEGIN: &str = "<!-- BEGIN GENERATED TYPED SUPPORT VOCABULARY -->";
                const END: &str = "<!-- END GENERATED TYPED SUPPORT VOCABULARY -->";

                let guide = include_str!("../../docs/camera_profile_support.md");
                let mut expected = String::from(BEGIN);
                expected.push_str(
                    "\n| Area | Marker | Typed surface |\n| ---- | ------ | ------------- |",
                );
                for surface in $crate::capabilities::TypedSupportSurface::ALL {
                    expected.push_str(&format!(
                        "\n| {} | `{}` | {} |",
                        surface.documentation_area(),
                        surface.marker_trait_name(),
                        surface.documentation_api(),
                    ));
                }
                expected.push('\n');
                expected.push_str(END);

                let start = guide
                    .find(BEGIN)
                    .expect("typed-support vocabulary table start marker");
                let relative_end = guide[start..]
                    .find(END)
                    .expect("typed-support vocabulary table end marker");
                let end = start + relative_end + END.len();
                let actual = guide[start..end].replace("\r\n", "\n");
                assert_eq!(
                    actual,
                    expected,
                    "regenerate the typed-support contributor table from the declarative registry"
                );
            }

            #[test]
            fn camera_profile_support_focus_zone_table_matches_registry() {
                const BEGIN: &str = "<!-- BEGIN GENERATED FOCUS ZONES -->";
                const END: &str = "<!-- END GENERATED FOCUS ZONES -->";

                let guide = include_str!("../../docs/camera_profile_support.md");
                let mut expected = String::from(BEGIN);
                expected.push_str("\n| Profile | `focus_zones` |\n| ------- | ------------- |");
                for facts in BUILTIN_PROFILE_FACTS {
                    let zones = facts.id.focus_zones();
                    let zones = if zones.is_empty() {
                        "none".to_owned()
                    } else {
                        zones
                            .iter()
                            .map(|zone| format!("`{zone:?}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    expected.push_str(&format!("\n| `{}` | {zones} |", facts.type_name));
                }
                expected.push('\n');
                expected.push_str(END);

                let start = guide.find(BEGIN).expect("focus-zone table start marker");
                let relative_end = guide[start..].find(END).expect("focus-zone table end marker");
                let end = start + relative_end + END.len();
                let actual = guide[start..end].replace("\r\n", "\n");
                assert_eq!(
                    actual, expected,
                    "regenerate the focus-zone table from the profile registry"
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
                        .map(|port| port.to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    let udp = facts
                        .udp
                        .map(|port| port.to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    let serial = if facts.serial { "yes" } else { "no" };
                    let envelope = if facts.envelope == $crate::ProfileEnvelope::SonyEncapsulated {
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

/// The built-in profile registry: one row per profile, passed to `$callback`.
///
/// `camera::profiles` expands it with `__define_builtin_profiles` into the
/// profile types and everything derived from them, and the crate's public
/// re-export lists expand it with [`reexport_builtin_profiles`], so a new row
/// needs no hand edit elsewhere.
macro_rules! builtin_profile_registry {
    ($callback:path) => {
        $callback! {
            groups {
                group GenericVisca {
                    doc: "Generic VISCA-compatible cameras with basic features.",
                    display: "Generic VISCA",
                    profiles: [GenericVisca, SonyBrc300, SonyEviH100, NearusBrc300],
                }
                group PtzOpticsG2 {
                    doc: "PtzOptics G2/G3/30X cameras using raw VISCA with PTZOptics timing limits.",
                    display: "PtzOptics Series",
                    profiles: [PtzOpticsG2, PtzOpticsG3, PtzOptics30X],
                }
                group SonyProfessional {
                    doc: "Sony professional cameras using Sony encapsulated VISCA.",
                    display: "Sony Professional",
                    profiles: [SonyFr7, SonyBrcH900],
                }
            }
            profiles {
                profile PtzOpticsG2 {
                    doc: "PtzOptics G2-series camera profile.",
                    id: PtzOpticsG2,
                    id_doc: "PtzOptics G2 series cameras",
                    id_attrs: [],
                    vendor: "PtzOptics",
                    description: "G2-series PTZ camera with 12x, 20x, and 30x optical variants and 128 presets",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: { default_port: 5678 },
                        udp: { default_port: 1259 },
                        serial: supported,
                    },
                    metadata: {
                        model_name: "PtzOptics G2",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2448, 2448),
                        tilt_range: range!(i32, -432, 1296),
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // 12x, 20x and 30x lenses share this profile (§4.1): a lens fact.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x0000,
                        far_limit: 0xFFFF,
                        auto_focus: true,
                        one_push: false,
                        focus_zone_inquiry: true,
                        zones: profile_constants::PTZ_OPTICS_G2_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(domain!(u8, 0, 17)),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: Some(range!(u8, 0, 14)),
                        sharpness_range: Some(range!(u8, 0, 15)),
                        saturation_range: Some(range!(u8, 0, 14)),
                        flip: true,
                        mirror: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        nr_2d: true,
                        nr_3d: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 127,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    usb_audio: { supported: true },
                    typed_support: [
                        PtzOpticsAntiFlicker,
                        PtzOpticsSettingsSave,
                        PtzOpticsPresetRecallSpeed,
                        PtzOpticsMulticastStreaming,
                        PtzOpticsNdiQuality,
                        ExposureMode,
                        ExposureCompensation,
                        BrightnessControl,
                        FocusLock,
                        DirectZoom,
                        IrisControl,
                        FocusZone,
                        FocusZoneInquiry,
                        UsbAudio,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        ColorTemperatureInquiry,
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
                        NoiseReduction2D,
                        NoiseReduction2DMode,
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    withheld: [],
                    evidence: [
                        ("command_cancel", "On the PTZOptics G2 bench (PT30X-NDI G2 firmware ARM 6.3.51THI and PT12X-NDI G2 firmware ARM 6.4.18SHI; 2026-10-05), a socket cancel `81 2y FF` sent while a slow absolute pan/tilt move ran on socket y (ACK `90 4y`) was answered `90 60 02 FF` (syntax error) and the move continued to its target and completed (`90 5y`): the G2 family does not implement socket cancel."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795), `81 09 00 02 FF` returns the 2-byte payload `00 52`, not the 7-byte Sony layout, and neither R1 nor R14 documents the reply. Keep the typed version inquiry absent; send the raw inquiry instead."),
                        ("digital_zoom", "No model- and firmware-identified source-backed evidence establishes VISCA digital-zoom control for G2; keep it unavailable pending documented validation."),
                        ("iris", "R1 and the current PTZOptics G2/G3 Developer Portal (R14 in docs/visca_reference.md) document G2 `04 39`, iris reset/up/down, direct `04 4B`, and `09 04 4B` position inquiry. They omit the distinct `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the G2 family."),
                        ("focus_zones", "R14/R20 document `CAM_AFZone` Top `00`, Center `01` and Bottom `02`. On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795) the cameras also report `03`, accept `81 01 04 AA 03 FF` with ACK and completion, and read `03` back, so `FocusZone::Zone03` is admitted for this family."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the 81 2A 02 A0 04 USB-audio command and matching inquiry for G2 models."),
                        ("noise_reduction", "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain."),
                    ],
                }

                profile PtzOpticsG3 {
                    doc: "PtzOptics G3 camera profile.",
                    id: PtzOpticsG3,
                    id_doc: "PtzOptics G3 series cameras",
                    id_attrs: [],
                    vendor: "PtzOptics",
                    description: "Latest generation PTZ camera using the conservative raw VISCA preset range",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: { default_port: 5678 },
                        udp: { default_port: 1259 },
                        serial: supported,
                    },
                    metadata: {
                        model_name: "PtzOptics G3",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2448, 2448),
                        tilt_range: range!(i32, -432, 1296),
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x0000,
                        far_limit: 0xFFFF,
                        auto_focus: true,
                        one_push: false,
                        focus_zone_inquiry: false,
                        zones: profile_constants::DOCUMENTED_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(domain!(u8, 0, 17)),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: Some(range!(u8, 0, 14)),
                        sharpness_range: Some(range!(u8, 0, 15)),
                        saturation_range: Some(range!(u8, 0, 14)),
                        flip: true,
                        mirror: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        nr_2d: true,
                        nr_3d: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 127,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    usb_audio: { supported: false },
                    typed_support: [
                        PtzOpticsAntiFlicker,
                        PtzOpticsSettingsSave,
                        PtzOpticsPresetRecallSpeed,
                        PtzOpticsMulticastStreaming,
                        PtzOpticsNdiQuality,
                        ExposureMode,
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
                        NoiseReduction2D,
                        NoiseReduction2DMode,
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    withheld: [],
                    evidence: [
                        ("command_cancel", "No PTZOptics source (R1, R10, R14) documents the standard `8x 2p FF` socket-cancel command and G3 was not on the bench, so it stays unavailable under the source rule. The G2 family, which shares this command set, rejects it on the bench."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "R10 and R14 do not document a G3 `09 00 02` reply layout, and the G2 family replies with an unsourced 2-byte payload. G3 was not bench-tested; the typed Sony-format version inquiry stays absent under the source-evidence rule until a source or G3 hardware evidence establishes the reply."),
                        ("focus_zones", "The R14 Focus page (R20), which PTZOptics describes as the full G2/G3 VISCA list, documents only the `CAM_AFZone` values Top `00`, Center `01` and Bottom `02`. `FocusZone::Zone03` was observed only on the PTZOptics G2 bench (#795); G3 was not bench-tested, so the typed setter refuses it before any I/O."),
                        ("digital_zoom", "PTZOptics built-ins keep VISCA digital zoom unavailable until model-specific evidence exists."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                        ("iris", "R10 and the current PTZOptics G2/G3 Developer Portal (R14 in docs/visca_reference.md) independently document G3 `04 39`, iris reset/up/down, direct `04 4B`, and `09 04 4B` position inquiry. They omit the distinct `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("vendor_controls", "The PTZOptics Move 4K G3 command manual (R10 in docs/visca_reference.md) documents the anti-flicker, settings-save, preset-recall-speed, multicast-streaming, and NDI-quality command families retained for G3."),
                        ("focus_zone_inquiry", "The G3 command list establishes focus-zone selection, but its query table does not establish the matching 81 09 04 AA response; keep the inquiry untyped."),
                        ("usb_audio", "The UAC command/query table is source-backed for the Gen-2 entries, not G3; keep USB audio conservative pending a G3-specific source."),
                        ("picture_effect", "PTZOptics' official Developer Portal identifies its current VISCA list for G2 and G3 and documents the 04 63 picture-effect command and inquiry there."),
                        ("noise_reduction", "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain."),
                    ],
                }

                profile PtzOptics30X {
                    doc: "PtzOptics legacy PT30X SDI/NDI G2 raw-VISCA camera profile.",
                    id: PtzOptics30X,
                    id_doc: "PTZOptics legacy PT30X SDI/NDI G2 (Gen-2 raw-VISCA) camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "ptzoptics-30x"))]],
                    vendor: "PtzOptics",
                    description: "Legacy PT30X SDI/NDI G2 raw-VISCA profile with 30x optical zoom",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: { default_port: 5678 },
                        udp: { default_port: 1259 },
                        serial: supported,
                    },
                    metadata: {
                        model_name: "PtzOptics 30X",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 150,
                        min_command_spacing_ms: 100,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2448, 2448),
                        tilt_range: range!(i32, -432, 1296),
                        max_pan_speed: 24,
                        max_tilt_speed: 20,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 14.4,
                        tilt_degrees_to_units: 14.4,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // PT30X-NDI: 30x optical, f4.42–132.6 mm (R3; docs/visca_reference.md §4.1).
                        optical_zoom_ratio: Some(30.0),
                    },
                    focus: {
                        near_limit: 0x0000,
                        far_limit: 0xFFFF,
                        auto_focus: true,
                        one_push: false,
                        focus_zone_inquiry: true,
                        zones: profile_constants::PTZ_OPTICS_G2_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(domain!(u8, 0, 17)),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: Some(range!(u8, 0, 14)),
                        sharpness_range: Some(range!(u8, 0, 15)),
                        saturation_range: Some(range!(u8, 0, 14)),
                        flip: true,
                        mirror: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        nr_2d: true,
                        nr_3d: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 127,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    usb_audio: { supported: true },
                    typed_support: [
                        PtzOpticsAntiFlicker,
                        PtzOpticsSettingsSave,
                        PtzOpticsPresetRecallSpeed,
                        PtzOpticsMulticastStreaming,
                        PtzOpticsNdiQuality,
                        ExposureMode,
                        ExposureCompensation,
                        BrightnessControl,
                        FocusLock,
                        DirectZoom,
                        IrisControl,
                        FocusZone,
                        FocusZoneInquiry,
                        UsbAudio,
                        BacklightCompensation,
                        WideDynamicRange,
                        ColorTemperature,
                        ColorTemperatureInquiry,
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
                        NoiseReduction2D,
                        NoiseReduction2DMode,
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    withheld: [],
                    evidence: [
                        ("command_cancel", "On the PTZOptics G2 bench (PT30X-NDI G2 firmware ARM 6.3.51THI and PT12X-NDI G2 firmware ARM 6.4.18SHI; 2026-10-05), a socket cancel `81 2y FF` sent while a slow absolute pan/tilt move ran on socket y (ACK `90 4y`) was answered `90 60 02 FF` (syntax error) and the move continued to its target and completed (`90 5y`): the G2 family does not implement socket cancel."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795), the legacy G2 models return the 2-byte payload `00 52` for `81 09 00 02 FF`, not the 7-byte Sony layout, and no PTZOptics source documents the reply. Keep the typed version inquiry absent."),
                        ("digital_zoom", "The Axis 0x7AC0 digital endpoint is not applied to PTZOptics; the 30X raw VISCA profile keeps the standard 0x4000 optical endpoint and no typed digital zoom."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                        ("iris", "The shared PTZOptics Gen-2 source documents iris priority, relative controls, direct `04 4B`, and matching `09 04 4B` position inquiry for the raw 30X profile. It does not establish a model-specific `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the raw 30X model."),
                        ("focus_zones", "R14/R20 document `CAM_AFZone` Top `00`, Center `01` and Bottom `02`. On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795) the cameras also report `03`, accept `81 01 04 AA 03 FF` with ACK and completion, and read `03` back, so `FocusZone::Zone03` is admitted for this family."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the USB-audio command and matching inquiry for the raw 30X model."),
                        ("noise_reduction", "R1 supplies legacy PT30X SDI/NDI G2 09 04 50/53/54 inquiries, including 3D 0..8, but no setters. R14 supplies 04 50/53/54 controls only because R15 explicitly narrows this profile to that legacy PT30X SDI/NDI G2 raw-VISCA family. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by query output domains; do not generalize this evidence to newer 30X products."),
                    ],
                }

                profile SonyFR7 {
                    doc: "Sony FR7 camera profile.",
                    id: SonyFr7,
                    id_doc: "Sony FR7 camera",
                    id_attrs: [],
                    vendor: "Sony",
                    description: "Professional cinema camera with variable ND filter and model-specific exposure controls",
                    envelope: $crate::transport::SonyEncapsulated,
                    transport: {
                        tcp: none,
                        udp: { default_port: 52381 },
                        serial: none,
                    },
                    metadata: {
                        model_name: "Sony FR7",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: profile_registry::command_timeouts_ms(8000, 30000, 60000, 300000, 8000),
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: false,
                            nd_filter: true,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 35,
                        min_command_spacing_ms: 35,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2700, 2700),
                        tilt_range: range!(i32, -300, 1200),
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 15.88,
                        tilt_degrees_to_units: 15.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7000),
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // Interchangeable-lens body: the ratio belongs to the mounted lens.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::NO_SHARED_EXPOSURE_MODES,
                        iris_range: None,
                        shutter_speeds: profile_constants::NO_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 15),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_FR7_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -7, 7)),
                        bg_tuning_range: Some(range!(i8, -7, 7)),
                        color_temp_range: None,
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: Some(range!(u8, 0, 14)),
                        sharpness_range: Some(range!(u8, 0, 14)),
                        saturation_range: Some(range!(u8, 0, 14)),
                        flip: true,
                        mirror: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        nr_2d: false,
                        nr_3d: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 255,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::Variable, steps: None },
                    variable_speed: { supported: true },
                    typed_support: [
                        VersionInquiry,
                        SonySpotlight,
                        ExposureCompensation,
                        PushAutoFocus,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
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
                        Tally,
                        DirectMenu,
                        NdFilter,
                        VariableSpeed,
                    ],
                    withheld: [],
                    evidence: [
                        ("shutter", "The FR7 command list (R7) could not be obtained to read its shutter table (direct downloads are refused), so the profile advertises no shutter codes: typed shutter positions are refused rather than sent with another model's codes. Use the raw command path."),
                        ("transport", "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header."),
                        ("version_inquiry", "The FR7 VISCA command list (R7) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (051E ILME-FR7/FR7K), ROM revision and maximum socket (02)."),
                        ("sony_spotlight", "The FR7 command list (R7 in docs/visca_reference.md) documents the fixed 04 3A spotlight commands, but not the fixed 04 5A auto slow-shutter commands."),
                        ("tally", "Sony professional profile metadata and typed controls expose tally for FR7."),
                        ("color_temperature", "FR7 uses ATW/manual WB surfaces; built-in typed color-temperature control remains unavailable."),
                        ("iris", "The FR7 command list documents vendor-relative iris Up/Down commands under 7E 04 4B and an Auto Iris inquiry under 05 34, but not the shared 04 39 AE-mode commands/inquiry, standard absolute iris-direct command, or 09 04 4B position inquiry; keep those shared typed surfaces absent until the distinct protocol families have their own models."),
                        ("nd_filter", "The FR7 registry is the only built-in entry with variable-ND metadata, typed ND controls, and the exact 0x64 position inquiry documented in the Sony command table."),
                        ("brightness", "The FR7 model command list does not establish the exposure-brightness control or inquiry; retain no brightness range or typed marker."),
                        ("focus_zone", "The FR7 model command list does not establish focus-zone selection or its inquiry; keep both typed surfaces absent."),
                        ("af_sensitivity", "The FR7 command list R7 documents Push AF/MF under `7E 04 58`, but not the shared `04 58` autofocus-sensitivity command or inquiry; keep the shared metadata and typed surface absent."),
                        ("variable_speed", "The FR7 command list R7 documents the `06 45` normal/extended pan/tilt speed-step range. Its separate `7E 04 1B` family selects preset-speed behavior and is not used by this typed surface."),
                        ("noise_reduction", "The FR7 command list R7 does not establish the shared `04 50`/`04 53`/`04 54` noise-reduction family; keep the shared metadata and typed surfaces absent."),
                        ("picture_effect", "The FR7 model command list does not establish picture-effect control or inquiry; leave the typed surface unavailable."),
                    ],
                }

                profile SonyBRCH900 {
                    doc: "Sony BRC-H900 camera profile.",
                    id: SonyBrcH900,
                    id_doc: "Sony BRC-H900 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-brch900"))]],
                    vendor: "Sony",
                    description: "Professional PTZ camera with advanced image processing and 100 presets",
                    envelope: $crate::transport::SonyEncapsulated,
                    transport: {
                        tcp: none,
                        udp: { default_port: 52381 },
                        serial: none,
                    },
                    metadata: {
                        model_name: "Sony BRC-H900",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: profile_registry::command_timeouts_ms(6000, 30000, 60000, 300000, 6000),
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 35,
                        min_command_spacing_ms: 35,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2700, 2700),
                        tilt_range: range!(i32, -300, 1200),
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 15.88,
                        tilt_degrees_to_units: 15.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7000),
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::BRC_H900_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x1E)),
                        shutter_speeds: profile_constants::NO_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 15),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp_range: None,
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -7, 7)),
                        bg_tuning_range: Some(range!(i8, -7, 7)),
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        base_support: true,
                        contrast_range: Some(range!(u8, 0, 14)),
                        sharpness_range: Some(range!(u8, 0, 14)),
                        saturation_range: Some(range!(u8, 0, 14)),
                        flip: true,
                        mirror: true,
                        hue_range: None,
                        nr_2d: false,
                        nr_3d: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 100,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    tally: { supported: false },
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        VersionInquiry,
                        SonySpotlight,
                        ExposureMode,
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
                    ],
                    withheld: [],
                    evidence: [
                        ("shutter", "The BRC-H900 command list (R11) could not be obtained to read its shutter table (direct downloads are refused), so the profile advertises no shutter codes: typed shutter positions are refused rather than sent with another model's codes. Use the raw command path."),
                        ("transport", "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header."),
                        ("version_inquiry", "The BRC-H900 command list (R11) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (050B BRC-H900), ROM revision and maximum socket (02)."),
                        ("sony_spotlight", "The BRC-H900 command list (R11 in docs/visca_reference.md) documents the fixed 04 3A spotlight commands, but not the fixed 04 5A auto slow-shutter commands."),
                        ("tally", "The BRC-H900 source-backed command list does not establish the FR7 red/green tally family; typed tally remains unavailable."),
                        ("exposure_mode", "The BRC-H900 command list R11 lines 706-717 and 1003 documents the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri commands and `09 04 39` inquiry. Bright mode is not listed and is therefore absent from this profile's inventory."),
                        ("iris", "The BRC-H900 command list R11 lines 706-717 and 1012 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry. The distinct `09 04 2B` status inquiry remains unavailable."),
                        ("brightness", "The BRC-H900 model command list does not establish the exposure-brightness control or inquiry; retain no brightness range or typed marker."),
                        ("noise_reduction", "The BRC-H900 model command list R11 does not establish the shared `04 50`/`04 53`/`04 54` noise-reduction family, so both discovery metadata and typed surfaces remain unavailable."),
                        ("picture_effect", "The BRC-H900 model command list does not establish picture-effect control or inquiry; leave the typed surface unavailable."),
                    ],
                }

                profile SonyEVIH100 {
                    doc: "Sony EVI-H100 camera profile.",
                    id: SonyEviH100,
                    id_doc: "Sony EVI-H100 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-evih100"))]],
                    vendor: "Sony",
                    description: "Compact HD PTZ camera with basic feature set",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: none,
                        udp: none,
                        serial: supported,
                    },
                    metadata: {
                        model_name: "Sony EVI-H100",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 240,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -0x1E1B, 0x1E1B),
                        tilt_range: range!(i32, -0x038B, 0x0FF0),
                        max_pan_speed: 0x18,
                        max_tilt_speed: 0x17,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 7707.0 / 170.0,
                        tilt_degrees_to_units: 7707.0 / 170.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7AC0),
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: true,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x11, gaps: [0x01, 0x02, 0x03, 0x04])),
                        shutter_speeds: profile_constants::SONY_EVI_H100_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0x00, 0x0F),
                        brightness_range: Some(domain!(u8, 0x00, 0x1F, gaps: [0x01, 0x02, 0x03, 0x04])),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: true,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp_range: None,
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: None,
                        // R8 Aperture Level `00`..`0F` and Color Gain/Hue
                        // `0h`..`Eh`.
                        sharpness_range: Some(range!(u8, 0x00, 0x0F)),
                        saturation_range: Some(range!(u8, 0x00, 0x0E)),
                        flip: false,
                        mirror: false,
                        hue_range: Some(range!(u8, 0x00, 0x0E)),
                        nr_2d: true,
                        nr_3d: false,
                        picture_effect: true,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        highest: 5,
                        speed_range: None,
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        VersionInquiry,
                        SonyAutoSlowShutter,
                        ExposureMode,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
                        OnePushFocus,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BrightnessControl,
                        ExposureCompensation,
                        BacklightCompensation,
                        RgbGain,
                        OnePushWhiteBalance,
                        SaturationControl,
                        HueControl,
                        GammaControl,
                        NoiseReduction2D,
                        NoiseReduction2DControl,
                    ],
                    withheld: [WideDynamicRange, SharpnessControl, PictureEffect],
                    evidence: [
                        ("transport", "The EVI-H100S/H100V technical manual (R8) documents VISCA over RS-232C and RS-422 at 9,600 or 38,400 bps; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The EVI-H100S/H100V technical manual (R8) shutter table (VISCA Command Setting Values, exposure control 1/2), 60/30 mode: `00` 1/1 s through `15` 1/10000 s. The 50/25 mode assigns several codes other times (for example `13` 1/3500 s); the profile uses the 60/30 column."),
                        ("one_push_focus", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF`."),
                        ("digital_zoom", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF`, inquiry list (1/3) `CAM_DZoomModeInq` `8x 09 04 06 FF`, and the zoom position table (p. 44): optical `0000`..`4000` (x1..x20) and digital `4000`..`7AC0` (x1..x12), all reached with `CAM_Zoom` Direct `04 47`."),
                        ("af_sensitivity", "The EVI-H100S/H100V technical manual (R8) documents AF Sensitivity Normal `8x 01 04 58 02 FF` and Low `03` only; the typed surface also sends High `01`, which R8 does not list, so the metadata and typed surface stay unavailable."),
                        ("presets", "The EVI-H100S/H100V technical manual (R8) `CAM_Memory` reset/set/recall with memory number p = 0 to 5; it documents no preset recall speed."),
                        ("pan_tilt", "The EVI-H100S/H100V technical manual (R8) command list (4/4) and position table (p. 45): pan `E1E5`..`1E1B` (-170..+170 degrees), tilt `FC75`..`0FF0` (-20..+90 degrees, the range used here) with IMAGE FLIP off (`F010`..`038B` with it on), pan speed `01`..`18`, tilt speed `01`..`17`. One scale, 7707 units per 170 degrees, maps both axes; tilt `FC75` is then -20.0 and `0FF0` +90.0 degrees to within 0.01 degree."),
                        ("gain", "The EVI-H100S/H100V technical manual (R8) gain table (exposure control 1/2): positions `00` (-3 dB) through `0F` (+28 dB)."),
                        ("exposure_compensation", "The EVI-H100S/H100V technical manual (R8) command list (2/4) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`; inquiry list (1/3) `09 04 3E` and `09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The EVI-H100S/H100V technical manual (R8) command list (2/4) `CAM_Bright` up/down `04 0D` and direct `04 4D 00 00 0p 0q`; inquiry `09 04 4D`. Its Bright table (p. 45) lists `00` and `05`..`1F`; positions `01`..`04` are not listed, so the domain excludes them."),
                        ("rgb_gain", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with R/B gain `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("wide_dynamic_range", "The EVI-H100S/H100V technical manual (R8) documents Wide-D as `CAM_WD` `8x 01 04 3D` with inquiry `09 04 3D`, so the discovery metadata reports it; the typed `WideDynamicRange` surface encodes the different `04 25` dynamic-range command, which R8 does not list, so the typed surface is withheld."),
                        ("exposure_mode", "The EVI-H100S/H100V technical manual (R8) command list (2/4) documents `CAM_AE` Full Auto/Manual/Shutter Priority/Iris Priority/Bright `8x 01 04 39 00/03/0A/0B/0D FF` and inquiry list (1/3) `CAM_AEModeInq` `8x 09 04 39 FF`."),
                        ("iris", "The EVI-H100S/H100V technical manual (R8) command list (2/4) documents `CAM_Iris` reset/up/down `8x 01 04 0B 00/02/03 FF` and direct `8x 01 04 4B 00 00 0p 0q FF`, inquiry list (1/3) `CAM_IrisPosInq` `8x 09 04 4B FF`, and the iris table (p. 44) `00` CLOSE and `05`..`11` (F14..F1.6); positions `01`..`04` are not listed, so the domain excludes them. The distinct `09 04 2B` status inquiry is not listed."),
                        ("busy_timeout", "The EVI-H100S/H100V technical manual (R8) command list note 4: after the completion of a `CAM_Memory` recall, a following command may be answered `Command not executable` for at most 240 ms; the camera asks for it to be sent again."),
                        ("white_balance", "The EVI-H100S/H100V technical manual (R8) `CAM_WB` lists Auto, Indoor, Outdoor, One Push and Manual with the `04 10 05` one-push trigger; it has no color-temperature mode or `04 20` color-temperature command, and its `04 43`/`04 44` rows are absolute R/B gain `00..FF`, not the signed red/blue tuning surface."),
                        ("image_flip", "The EVI-H100S/H100V technical manual (R8) sets image flip with the rear IMAGE FLIP switch only; it lists no `04 61`/`04 66` VISCA flip or mirror command."),
                        ("version_inquiry", "The EVI-H100S/H100V technical manual (R8) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (050E EVI-H100V, 050F EVI-H100S), ROM revision and maximum socket (02)."),
                        ("sony_auto_slow_shutter", "The EVI-H100 technical manual (R8 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("noise_reduction", "The EVI-H100S/H100V technical manual (R8) command list (3/4) `CAM_NR` `8x 01 04 53 0p FF` (p: 0 off, levels 1 to 5) and inquiry list `CAM_NRInq` `8x 09 04 53 FF` -> `y0 50 0p FF`: exactly the 2D noise-reduction level control and inquiry. R8 lists no `04 50` auto/manual mode and no `04 54` 3D level, so those surfaces stay unavailable."),
                        ("saturation_control", "The EVI-H100S/H100V technical manual (R8) `CAM_ColorGain` Direct `8x 01 04 49 00 00 00 0p FF` and `CAM_ColorGainInq` `8x 09 04 49 FF` -> `y0 50 00 00 00 0p FF`, p `0h` (60%) to `Eh` (200%): the saturation control and inquiry byte for byte."),
                        ("hue_control", "The EVI-H100S/H100V technical manual (R8) `CAM_ColorHue` Direct `8x 01 04 4F 00 00 00 0p FF` and `CAM_ColorHueInq` `8x 09 04 4F FF` -> `y0 50 00 00 00 0p FF`, p `0h` (-14 degrees) to `Eh` (+14 degrees): the hue control and inquiry byte for byte."),
                        ("sharpness_control", "The EVI-H100S/H100V technical manual (R8) `CAM_Aperture` reset/up/down `8x 01 04 02 00/02/03 FF`, direct `8x 01 04 42 00 00 0p 0q FF` and `CAM_ApertureInq` `09 04 42`, with Aperture Level `00`..`0F`, so the discovery metadata reports it. The typed surface also sends the `04 05` sharpness auto/manual mode and its `09 04 05` inquiry, which R8 does not list, so the typed surface is withheld."),
                        ("picture_effect", "The EVI-H100S/H100V technical manual (R8) `CAM_PictureEffect` Off/Neg.Art/B&W `8x 01 04 63 00/02/04 FF`, so the discovery metadata reports it. Its `CAM_PictureEffectModeInq` reports Off as `00` and Neg.Art as `02`, while the typed inquiry decodes `02` as Off (the PTZOptics layout) and has no Neg.Art mode, so the typed surface is withheld."),
                    ],
                }

                profile SonyBRC300 {
                    doc: "Sony BRC-300 camera profile.",
                    id: SonyBrc300,
                    id_doc: "Sony BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-brc300"))]],
                    vendor: "Sony",
                    description: "Legacy PTZ camera with signed-centered 20-bit pan and 16-bit tilt coordinates",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: none,
                        udp: none,
                        serial: supported,
                    },
                    metadata: {
                        model_name: "Sony BRC-300",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        // Sony BRC-300 technical manual, pp. 12 and 22:
                        // signed 20-bit pan (`08A58` left / `F75A8` right)
                        // and signed 16-bit tilt (`493D` up / `E796` down).
                        // The library's degree convention is positive pan right
                        // and positive tilt up, so pan is opposite the raw
                        // axis and tilt follows it.
                        pan_range: range!(i32, -0x08A58, 0x08A58),
                        tilt_range: range!(i32, -0x186A, 0x493D),
                        max_pan_speed: 0x18,
                        // BRC-300 position frames have one speed byte. The
                        // profile-aware coarse-speed lowering mirrors it into
                        // both public wrappers; explicit paired speeds must
                        // agree and use this same documented maximum.
                        max_tilt_speed: 0x18,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        // The manual approximates one degree as `0xD0`; the
                        // signs follow the documented raw-axis polarity.
                        pan_degrees_to_units: -208.0,
                        tilt_degrees_to_units: 208.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7F00),
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xC000,
                        auto_focus: true,
                        one_push: true,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x11)),
                        shutter_speeds: profile_constants::SONY_BRC300_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0x00, 0x07),
                        brightness_range: Some(domain!(u8, 0x00, 0x17)),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: true,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp_range: None,
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        // Backlight inquiry/control is part of the canonical
                        // `image()` noun, so its source-backed typed marker
                        // requires the base accessor as well.
                        base_support: true,
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: true,
                        mirror: false,
                        hue_range: None,
                        nr_2d: false,
                        nr_3d: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: None,
                    },
                    presets: {
                        // Sony BRC-300 technical manual, pp. 11–14:
                        // CAM_Memory accepts p = 0..=5, and Cmd_PT_M_Speed
                        // documents q = 1..=24. These discovery facts do not
                        // establish compatibility with the PTZOptics typed
                        // preset-recall-speed command family.
                        highest: 5,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        VersionInquiry,
                        SonyAutoSlowShutter,
                        ExposureMode,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
                        OnePushFocus,
                        IrisControl,
                        BrightnessControl,
                        ExposureCompensation,
                        BacklightCompensation,
                        RgbGain,
                        ImageFlip,
                        OnePushWhiteBalance,
                    ],
                    withheld: [],
                    evidence: [
                        ("zoom", "The BRC-300 technical manual (R12) `CAM_Zoom` Direct `8x 01 04 47 0p 0q 0r 0s FF` and its zoom position table (x12 lens): optical `0000` (x1) through `4000` (x12), digital `4000` (x1) through `7F00` (x4)."),
                        ("digital_zoom", "The BRC-300 technical manual (R12) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF` and `CAM_DZoomModeInq` `8x 09 04 06 FF` (`y0 50 02/03 FF`); combined optical-plus-digital positions `4000`..`7F00` are reached with `CAM_Zoom` Direct `04 47` in Combine mode (`04 36 00`) with the x4 E-Zoom limit (`7E 01 19 01`); with the x2 limit the camera stops at `6A00`."),
                        ("one_push_focus", "The BRC-300 technical manual (R12) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF` (One Push AF Trigger)."),
                        ("transport", "The BRC-300 technical manual (R12) documents VISCA over RS-232C and RS-422; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The BRC-300 technical manual (R12) shutter table (VISCA command setting values, exposure control 1/2), BRC-300 column: `02` 1/4 s through `15` 1/10000 s."),
                        ("image_flip", "The BRC-300 technical manual (R12) `CAM_ImgFlip` on/off `8x 01 04 66 02/03 FF` and `CAM_ImgFlipInq` `8x 09 04 66 FF`. It lists no `04 61` mirror command, so mirror stays unavailable."),
                        ("gain", "The BRC-300 technical manual (R12) gain table (exposure control 1/2): positions `0` (-3 dB) through `7` (18 dB)."),
                        ("exposure_compensation", "The BRC-300 technical manual (R12) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`, inquiries `09 04 3E`/`09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The BRC-300 technical manual (R12) `CAM_Bright` reset/up/down `04 0D` and direct `04 4D 00 00 0p 0q`, inquiry `09 04 4D`. Its Bright table lists `00`..`17`."),
                        ("rgb_gain", "The BRC-300 technical manual (R12) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("one_push_white_balance", "The BRC-300 technical manual (R12) `CAM_WB` lists One Push WB `8x 01 04 35 03 FF` and the One Push trigger `8x 01 04 10 05 FF` (command list 1/4), and `CAM_WBModeInq` reports `y0 50 03 FF` One Push."),
                        ("focus_near_limit", "The BRC-300 technical manual (R12) lists no `04 28` focus near-limit command or `09 04 28` inquiry, so the typed near-limit surface is unavailable."),
                        ("version_inquiry", "The BRC-300 technical manual R12 documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (040F BRC-300), ROM revision and maximum socket (02)."),
                        ("sony_auto_slow_shutter", "The BRC-300 technical manual (R12 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("exposure_mode", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri/Bright commands and `09 04 39` inquiry."),
                        ("iris", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry, and its iris table (exposure control 1/2) lists `00` CLOSE through `11` F1.6. The distinct `09 04 2B` status inquiry remains unavailable."),
                        ("backlight", "The BRC-300 technical manual R12 pp. 11 and 14 (text rows 404-405 and 539-540 in docs/visca_reference.md's linked source) documents `CAM_BackLight` on/off as `8x 01 04 33 02/03 FF` and `CAM_BackLightModeInq` as `8x 09 04 33 FF` with `y0 50 02/03 FF` replies. Those paired rows support the `image()` backlight surface, not a broad image-processing grant."),
                    ],
                }

                profile NearusBRC300 {
                    doc: "Nearus BRC-300 camera profile.",
                    id: NearusBrc300,
                    id_doc: "Nearus BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "nearus-brc300"))]],
                    vendor: "Nearus",
                    description: "Nearus BRC-300 PTZ camera with the BRC-300 signed-centered 20-bit pan and 16-bit tilt framing",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: none,
                        udp: none,
                        serial: supported,
                    },
                    metadata: {
                        model_name: "Nearus BRC-300",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: $crate::CommandTimeouts::DEFAULT,
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        // R21 command list (3/4, 4/4): signed 20-bit pan
                        // (`08A58` left end / `F75A8` right end) and signed
                        // 16-bit tilt (`493D` up / `E796` down) in the
                        // one-speed, five-pan-nibble position frame. The
                        // library's degree convention is positive pan right
                        // and positive tilt up, so pan is opposite the raw
                        // axis and tilt follows it.
                        pan_range: range!(i32, -0x08A58, 0x08A58),
                        tilt_range: range!(i32, -0x186A, 0x493D),
                        max_pan_speed: 0x18,
                        // The position frame has one speed byte; explicit
                        // paired speeds must agree and use this same maximum.
                        max_tilt_speed: 0x18,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        // R21's position table approximates one degree as
                        // `0xD0`; the signs follow the documented raw-axis
                        // polarity.
                        pan_degrees_to_units: -208.0,
                        tilt_degrees_to_units: 208.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: Some(0x7F00),
                        speed_range: range!(u8, 0, 7),
                        supports_direct: true,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xC000,
                        auto_focus: true,
                        one_push: true,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x11)),
                        shutter_speeds: profile_constants::SONY_BRC300_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0x00, 0x07),
                        brightness_range: Some(domain!(u8, 0x00, 0x17)),
                        backlight_comp: true,
                        exposure_comp_range: Some(range!(i8, -7, 7)),
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: true,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp_range: None,
                        red_gain_range: Some(range!(u8, 0x00, 0xFF)),
                        blue_gain_range: Some(range!(u8, 0x00, 0xFF)),
                    },
                    image: {
                        base_support: true,
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: true,
                        mirror: false,
                        hue_range: None,
                        nr_2d: false,
                        nr_3d: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: None,
                    },
                    presets: {
                        highest: 5,
                        speed_range: Some(range!(u8, 1, 24)),
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        VersionInquiry,
                        SonyAutoSlowShutter,
                        ExposureMode,
                        DirectZoom,
                        DigitalZoomToggle,
                        DigitalZoomRange,
                        OnePushFocus,
                        IrisControl,
                        BrightnessControl,
                        ExposureCompensation,
                        BacklightCompensation,
                        RgbGain,
                        ImageFlip,
                        OnePushWhiteBalance,
                    ],
                    withheld: [],
                    evidence: [
                        ("zoom", "The Nearus command list (R21) `CAM_Zoom` Direct `8x 01 04 47 0p 0q 0r 0s FF` and its zoom position table (x12 lens): optical `0000` (x1) through `4000` (x12), digital `4000` (x1) through `7F00` (x4)."),
                        ("digital_zoom", "The Nearus command list (R21) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF` and `CAM_DZoomModeInq` `8x 09 04 06 FF` (`y0 50 02/03 FF`); combined optical-plus-digital positions `4000`..`7F00` are reached with `CAM_Zoom` Direct `04 47` in Combine mode (`04 36 00`) with the x4 E-Zoom limit (`7E 01 19 01`); with the x2 limit the camera stops at `6A00`."),
                        ("one_push_focus", "The Nearus command list (R21) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF` (One Push AF Trigger)."),
                        ("pan_tilt", "The Nearus command list (R21) `Pan-tiltDrive` speeds `01`..`18h` on both axes; `AbsolutePosition`/`RelativePosition` `8x 01 06 02/03 VV 00 0Y 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF` with one speed byte and a five-nibble pan; `Pan-tiltLimitSet` limits pan `0x08A58` (left) / `0xF75A8` (right) and tilt `0x493D` (up) / `0xE796` (down); one degree is about `0xD0`."),
                        ("transport", "The Nearus command list (R21) documents VISCA over RS-232C and RS-422; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The Nearus command list (R21) shutter table, BRC-300 column: `02` 1/4 s through `15` 1/10000 s."),
                        ("presets", "The Nearus command list (R21) `CAM_Memory` memory number p = 0 to 5 and `Cmd_PT_M_Speed` preset speed 1 to 24."),
                        ("image_flip", "The Nearus command list (R21) `CAM_ImgFlip` on/off `8x 01 04 66 02/03 FF` and `CAM_ImgFlipInq` `8x 09 04 66 FF`. It lists no `04 61` mirror command, so mirror stays unavailable."),
                        ("gain", "The Nearus command list (R21) gain table (exposure control 1/2): positions `0` (-3 dB) through `7` (18 dB)."),
                        ("exposure_compensation", "The Nearus command list (R21) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`, inquiries `09 04 3E`/`09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The Nearus command list (R21) `CAM_Bright` reset/up/down `04 0D` and direct `04 4D 00 00 0p 0q`, inquiry `09 04 4D`. Its Bright table lists `00`..`17`."),
                        ("rgb_gain", "The Nearus command list (R21) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("sony_vendor_exposure", "The Nearus command list (R21, the BRC-300/300P command list text) documents the fixed `04 5A` auto slow-shutter commands and `09 04 5A` inquiry, but not the fixed `04 3A` spotlight commands."),
                        ("exposure_mode", "The Nearus command list (R21, the BRC-300/300P command list text) documents `CAM_AE` Full Auto/Manual/Shutter Priority/Iris Priority/Bright `8x 01 04 39 00/03/0A/0B/0D FF` and `CAM_AEModeInq` `8x 09 04 39 FF`."),
                        ("iris", "The Nearus command list (R21, the BRC-300/300P command list text) documents `CAM_Iris` reset/up/down `8x 01 04 0B 00/02/03 FF`, direct `8x 01 04 4B 00 00 0p 0q FF`, `CAM_IrisPosInq` `8x 09 04 4B FF`, and the iris table `00` CLOSE through `11` F1.6. The distinct `09 04 2B` status inquiry is not listed."),
                        ("one_push_white_balance", "The Nearus command list (R21, the BRC-300/300P command list text) `CAM_WB` lists One Push WB `8x 01 04 35 03 FF` and the One Push trigger `8x 01 04 10 05 FF`."),
                        ("image_controls", "The Nearus command list (R21, the BRC-300/300P command list text) lists no `04 49` saturation (color gain) command and no `04 28` focus near-limit command or inquiry, so neither typed surface is available."),
                        ("version_inquiry", "The Nearus \"VISCA Protocol via Sony\" document (R21) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (040F BRC-300/P, 0410 BRU-300/P), ROM revision and maximum socket (02); its command list repeats the row as `y0 50 00 01 mn pq rs tu vw FF` (model code 04xx, socket 02)."),
                        ],
                }

                profile GenericVisca {
                    doc: "Generic VISCA camera profile.",
                    id: GenericVisca,
                    id_doc: "Generic VISCA-compatible camera",
                    id_attrs: [#[default]],
                    vendor: "Generic",
                    description: "Conservative profile for unknown VISCA-compatible cameras",
                    envelope: $crate::transport::RawVisca,
                    transport: {
                        tcp: { default_port: 5678 },
                        udp: { default_port: 1259 },
                        serial: supported,
                    },
                    metadata: {
                        model_name: "Generic VISCA Camera",
                        default_camera_id: 1,
                        ack_timeout_ms: profile_registry::INTERIM_ACK_TIMEOUT_MS,
                        command_timeouts: profile_registry::command_timeouts_ms(10000, 30000, 60000, 300000, 10000),
                        inquiry_timeout_ms: profile_registry::INTERIM_INQUIRY_TIMEOUT_MS,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Partial,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        maximum_command_sockets: 2,
                        position_inquiries: {
                            pan_tilt: true,
                            zoom: true,
                            focus: true,
                            iris: true,
                            nd_filter: false,
                        },
                        preset_recall_axes: $crate::AffectedAxes::PAN_TILT
                            .union($crate::AffectedAxes::ZOOM)
                            .union($crate::AffectedAxes::FOCUS),
                        min_inquiry_spacing_ms: 0,
                        min_command_spacing_ms: 0,
                    },
                    pan_tilt: {
                        pan_range: range!(i32, -2880, 2880),
                        tilt_range: range!(i32, -1440, 1440),
                        max_pan_speed: 24,
                        max_tilt_speed: 24,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 16.0,
                        tilt_degrees_to_units: 16.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0xFFFF,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
                        supports_direct: false,
                        supports_variable: true,
                        // No cited source states this model's optical ratio.
                        optical_zoom_ratio: None,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xE000,
                        auto_focus: true,
                        one_push: true,
                        focus_zone_inquiry: false,
                        zones: profile_constants::NO_FOCUS_ZONES,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(domain!(u16, 0x00, 0x11, gaps: [0x01, 0x02, 0x03, 0x04])),
                        shutter_speeds: profile_constants::SONY_BRC300_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: None,
                        backlight_comp: false,
                        exposure_comp_range: None,
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::STANDARD_WB_MODES,
                        one_push: true,
                        rg_tuning_range: None,
                        bg_tuning_range: None,
                        color_temp_range: None,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        base_support: false,
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: None,
                        flip: false,
                        mirror: false,
                        hue_range: None,
                        nr_2d: false,
                        nr_3d: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma_range: None,
                    },
                    presets: {
                        highest: 5,
                        speed_range: None,
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
                    motion_sync: { speed_range: None },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
                        VersionInquiry,
                        ExposureMode,
                        IrisControl,
                        OnePushFocus,
                        OnePushWhiteBalance,
                    ],
                    withheld: [],
                    evidence: [
                        ("shutter", "Generic VISCA uses the shutter codes R8, R12 and R21 all list with the same exposure time: `02` 1/4 s through `15` 1/10000 s (R8 also lists `00` 1/1 s and `01` 1/2 s)."),
                        ("presets", "R8, R12 and R21 all document `CAM_Memory` memory numbers 0 to 5; R8 documents no preset recall speed, so Generic VISCA has none."),
                        ("exposure_mode", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): the `04 39` Full Auto/Manual/Shutter Priority/Iris Priority/Bright commands and the `09 04 39` inquiry."),
                        ("iris", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): `04 0B` reset/up/down, direct `04 4B`, the `09 04 4B` position inquiry, and the iris positions all three list: `00` CLOSE and `05`..`11` (R12 and R21 also list `01`..`04`, R8 does not). None documents the distinct `09 04 2B` status inquiry."),
                        ("one_push_white_balance", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): the `04 35 03` One Push WB mode and the `04 10 05` trigger."),
                        ("focus_near_limit", "R12 and R21 list no `04 28` focus near-limit command or `09 04 28` inquiry, so the Sony-standard family does not include it."),
                        ("version_inquiry", "The generic profile models the Sony VISCA baseline, whose `CAM_VersionInq` reply is the 7-byte vendor/model/ROM/socket layout documented in R12; cameras that reply in another layout fail decode with an invalid-length error rather than a mislabelled value."),
                        ("one_push_focus", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF`."),
                        ("direct_zoom", "Generic profile keeps absolute zoom positioning unavailable despite baseline zoom movement."),
                        ("digital_zoom", "R8, R12 and R21 all document `CAM_DZoom` `04 06`, but their digital position tables differ (R8 `4000`..`7AC0`, R12/R21 `4000`..`7F00`) and Generic VISCA has no direct zoom, so it keeps digital zoom unavailable."),
                        ],
                }
            }
        }
    };
}
pub(crate) use builtin_profile_registry;

/// Re-exports every built-in profile type from `camera::profiles`.
macro_rules! reexport_builtin_profiles {
    (
        groups { $($groups:tt)* }
        profiles { $( profile $profile:ident { $($row:tt)* } )* }
    ) => {
        pub use $crate::camera::profiles::{ $($profile),* };
    };
}
pub(crate) use reexport_builtin_profiles;
