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
    pub(crate) position_inquiries: crate::profile::PositionInquirySupport,
    pub(crate) image_base_support: bool,
    pub(crate) focus_zone_inquiry: bool,
    pub(crate) usb_audio: bool,
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
                        // Acknowledgement deadline. Every built-in profile
                        // uses 500 ms, restored to the 1.x `TimeoutConfig`
                        // default as an interim value (issue #689); the final
                        // per-profile numbers are to come from the hardware
                        // pass. `validate_tuning` lets an override only widen
                        // this, never undercut the profile floor.
                        ack_timeout_ms: $ack_timeout_ms:expr,
                        command_timeouts_ms: {
                            quick: $quick_timeout_ms:expr,
                            movement: $movement_timeout_ms:expr,
                            preset: $preset_timeout_ms:expr,
                            long_running: $long_running_timeout_ms:expr,
                            network: $network_timeout_ms:expr,
                        },
                        // Inquiry-response deadline. Every built-in profile
                        // uses 1000 ms: an intentional interim change from 1.x,
                        // where inquiries had no dedicated deadline and used the
                        // 5 s `Quick` category budget (issue #689, parity waiver
                        // `inquiry-deadline-interim-default`). Hardware pass to
                        // finalize per profile.
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
                        magnification_to_units: $zoom_magnification_to_units:expr,
                    },
                    focus: {
                        near_limit: $focus_near_limit:expr,
                        far_limit: $focus_far_limit:expr,
                        auto_focus: $supports_auto_focus:expr,
                        one_push: $supports_one_push_focus:expr,
                        focus_zone: $supports_focus_zone:expr,
                        focus_zone_inquiry: $supports_focus_zone_inquiry:expr,
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
                        // Source-backed permission for the base `image()` noun.
                        // This is distinct from the metadata facts below: every
                        // profile implements `ImageProcessing` for discovery.
                        base_support: $supports_image_processing:tt,
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
                    $(usb_audio: {
                        supported: $supports_usb_audio:expr $(,)?
                    },)?
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
                const COMMAND_TIMEOUTS: $crate::CommandTimeouts = $crate::CommandTimeouts::new(
                    std::time::Duration::from_millis($quick_timeout_ms),
                    std::time::Duration::from_millis($movement_timeout_ms),
                    std::time::Duration::from_millis($preset_timeout_ms),
                    std::time::Duration::from_millis($long_running_timeout_ms),
                    std::time::Duration::from_millis($network_timeout_ms),
                );
                const BUSY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis($busy_timeout_ms);
                const INQUIRY_SUPPORT: $crate::capabilities::InquirySupport = $inquiry_support;
                const POSITION_INQUIRY_SUPPORT: $crate::profile::PositionInquirySupport =
                    $crate::profile::PositionInquirySupport::new_with_iris_nd(
                        $pan_tilt_position_inquiry,
                        $zoom_position_inquiry,
                        $focus_position_inquiry,
                        $iris_position_inquiry,
                        $nd_filter_position_inquiry,
                    );
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
                const ZOOM_MAGNIFICATION_TO_UNITS: f32 = $zoom_magnification_to_units;
            }

            impl $crate::capabilities::Focus for $profile {
                const FOCUS_NEAR_LIMIT: u16 = $focus_near_limit;
                const FOCUS_FAR_LIMIT: u16 = $focus_far_limit;
                const SUPPORTS_AUTO_FOCUS: bool = $supports_auto_focus;
                const SUPPORTS_ONE_PUSH_FOCUS: bool = $supports_one_push_focus;
                const SUPPORTS_FOCUS_ZONE: bool = $supports_focus_zone;
                const SUPPORTS_FOCUS_ZONE_INQUIRY: bool = $supports_focus_zone_inquiry;
                const MAX_FOCUS_SPEED: u8 = $max_focus_speed;
                const SUPPORTS_AF_SENSITIVITY: bool = $supports_af_sensitivity;
                const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool =
                    $supports_focus_near_limit_inquiry;
            }

            impl $crate::capabilities::Exposure for $profile {
                const EXPOSURE_MODES: &'static [$crate::command::exposure::ExposureMode] =
                    $exposure_modes;
                const IRIS_RANGE: Option<$crate::capabilities::CapabilityRange<u16>> = $iris_range;
                const SHUTTER_SPEEDS: &'static [$crate::capabilities::ShutterSpeed] =
                    $shutter_speeds;
                const GAIN_RANGE: $crate::capabilities::CapabilityRange<u8> = $gain_range;
                const BRIGHTNESS_RANGE: Option<$crate::capabilities::CapabilityRange<u16>> = $brightness_range;
                const SUPPORTS_BACKLIGHT_COMP: bool = $supports_backlight_comp;
                const SUPPORTS_EXPOSURE_COMP: bool = $supports_exposure_comp;
                const EXPOSURE_COMP_RANGE: $crate::capabilities::CapabilityRange<i8> = $exposure_comp_range;
                const SUPPORTS_WDR: bool = $supports_wdr;
            }

            impl $crate::capabilities::WhiteBalance for $profile {
                const WB_MODES: &'static [$crate::WhiteBalanceMode] = $wb_modes;
                const SUPPORTS_ONE_PUSH_WB: bool = $supports_one_push_wb;
                const RG_TUNING_RANGE: Option<$crate::capabilities::CapabilityRange<i8>> = $rg_tuning_range;
                const BG_TUNING_RANGE: Option<$crate::capabilities::CapabilityRange<i8>> = $bg_tuning_range;
                const SUPPORTS_COLOR_TEMP: bool = $supports_color_temp;
                const COLOR_TEMP_RANGE: Option<$crate::capabilities::CapabilityRange<u16>> = $color_temp_range;
                const SUPPORTS_RGB_GAIN: bool = $supports_rgb_gain;
                const RED_GAIN_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $red_gain_range;
                const BLUE_GAIN_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $blue_gain_range;
            }

            impl $crate::capabilities::ImageProcessing for $profile {
                const CONTRAST_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $contrast_range;
                const SHARPNESS_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $sharpness_range;
                const SATURATION_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $saturation_range;
                const SUPPORTS_FLIP: bool = $supports_flip;
                const SUPPORTS_MIRROR: bool = $supports_mirror;
                const SUPPORTS_HUE: bool = $supports_hue;
                const HUE_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $hue_range;
                const SUPPORTS_NOISE_REDUCTION: bool = $supports_noise_reduction;
                const SUPPORTS_2D_NR: bool = $supports_2d_nr;
                const SUPPORTS_3D_NR: bool = $supports_3d_nr;
                const SUPPORTS_LUMINANCE: bool = $supports_luminance;
                const SUPPORTS_PICTURE_EFFECT: bool = $supports_picture_effect;
                const LUMINANCE_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $luminance_range;
                const USES_COMBINED_FLIP_COMMAND: bool = $uses_combined_flip_command;
                const REQUIRES_SETTINGS_SAVE_FOR_FLIP: bool = $requires_settings_save_for_flip;
                const SUPPORTS_GAMMA: bool = $supports_gamma;
                const GAMMA_RANGE: Option<$crate::capabilities::CapabilityRange<u8>> = $gamma_range;
                const SUPPORTS_IMAGE_PROCESSING: bool = $supports_image_processing;
            }

            impl $crate::capabilities::Presets for $profile {
                const MAX_PRESETS: u8 = $max_presets;
                const PRESET_SPEED_RANGE: $crate::capabilities::CapabilityRange<u8> = $preset_speed_range;
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
                    position_inquiries: <$profile as $crate::capabilities::ProfileMetadata>::POSITION_INQUIRY_SUPPORT,
                    image_base_support: $supports_image_processing,
                    focus_zone_inquiry: $supports_focus_zone_inquiry,
                    usb_audio: __optional_support_bool!($($supports_usb_audio)?),
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
            /// Returns whether the runtime profile carries this built-in
            /// profile's complete protocol identity.
            ///
            /// `profile_id` is public so downstream callers can mutate a
            /// discovered inventory.  It is therefore an identity claim,
            /// not an authority token; vendor-specific request validation
            /// must only rely on it after every protocol identity fact has
            /// been checked against the generated profile row.
            pub(crate) fn matches_profile_spec(
                &self,
                profile: &$crate::ProfileSpec,
            ) -> bool {
                match self {
                    $(ProfileId::$id => profile.matches_compile_time_profile::<$profile>(),)*
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
                assert_eq!(facts.tcp.map(|facts| facts.default_port), id.default_tcp_port());
                assert_eq!(facts.udp.map(|facts| facts.default_port), id.default_udp_port());
                assert_eq!(facts.tcp.is_some(), id.supports_tcp());
                assert_eq!(facts.udp.is_some(), id.supports_udp());
                assert_eq!(facts.serial.is_some(), id.supports_serial());
                assert_eq!(facts.default_camera_id, P::DEFAULT_CAMERA_ID);
                assert_eq!(facts.default_camera_id, id.default_camera_id());
                assert_eq!(facts.inquiry_support, P::INQUIRY_SUPPORT);
                assert_eq!(facts.inquiry_support, id.inquiry_support());
                assert_eq!(facts.position_inquiries, P::POSITION_INQUIRY_SUPPORT);
                assert_eq!(spec.position_inquiries(), P::POSITION_INQUIRY_SUPPORT);
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
                assert_eq!(
                    facts.envelope == profile_registry::EnvelopeKind::SonyEncapsulated,
                    id.uses_sony_encapsulation()
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
                assert!(
                    P::EXPOSURE_COMP_RANGE.min() <= P::EXPOSURE_COMP_RANGE.max(),
                    "{id:?} exposure-compensation metadata must be non-empty"
                );
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
                assert!(
                    P::PRESET_SPEED_RANGE.min() <= P::PRESET_SPEED_RANGE.max(),
                    "{id:?} preset-speed metadata must be non-empty"
                );
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
                if facts.has_typed_support($crate::capabilities::TypedSupportSurface::IrisControl) {
                    assert!(
                        P::IRIS_RANGE.is_some(),
                        "{id:?} iris typed support requires an iris metadata range"
                    );
                }
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
                assert_eq!(caps.pan_range, P::PAN_RANGE.as_inclusive());
                assert_eq!(caps.tilt_range, P::TILT_RANGE.as_inclusive());
                assert_eq!(caps.pan_tilt_simultaneous, P::PAN_TILT_SIMULTANEOUS);
                assert_eq!(caps.preset_recovery_time, P::PRESET_RECOVERY_TIME);
                assert_eq!(caps.has_digital_zoom, P::DIGITAL_ZOOM_MAX.is_some());
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
                assert_eq!(caps.has_focus_zone, P::SUPPORTS_FOCUS_ZONE);
                assert_eq!(
                    caps.has_focus_zone_inquiry,
                    P::SUPPORTS_FOCUS_ZONE_INQUIRY
                );
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
                    caps.exposure_comp_profile_range,
                    P::EXPOSURE_COMP_RANGE.as_inclusive()
                );
                assert_eq!(
                    caps.iris_range,
                    P::IRIS_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(caps.gain_range, P::GAIN_RANGE.as_inclusive());
                assert_eq!(caps.shutter_speeds.len(), P::SHUTTER_SPEEDS.len());
                for (runtime, static_speed) in caps.shutter_speeds.iter().zip(P::SHUTTER_SPEEDS) {
                    assert_eq!(runtime.label, static_speed.label);
                    assert_eq!(runtime.value, static_speed.value);
                }
                assert_eq!(
                    caps.exposure_brightness_range,
                    P::BRIGHTNESS_RANGE.map(|range| range.as_inclusive())
                );
                assert_eq!(
                    caps.exposure_comp_range,
                    P::SUPPORTS_EXPOSURE_COMP.then(|| P::EXPOSURE_COMP_RANGE.as_inclusive())
                );
                assert_eq!(caps.has_one_push_wb, P::SUPPORTS_ONE_PUSH_WB);
                assert_eq!(caps.has_color_temp, P::SUPPORTS_COLOR_TEMP);
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
                assert_eq!(caps.has_rgb_gain, P::SUPPORTS_RGB_GAIN);
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
                assert_eq!(caps.supports_hue, P::SUPPORTS_HUE);
                assert_eq!(caps.uses_combined_flip_command, P::USES_COMBINED_FLIP_COMMAND);
                assert_eq!(caps.requires_settings_save_for_flip, P::REQUIRES_SETTINGS_SAVE_FOR_FLIP);
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
                    P::PRESET_SPEED_RANGE.as_inclusive()
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
                assert_eq!(caps.has_motion_sync, P::SUPPORTS_MOTION_SYNC);
                assert_eq!(caps.max_motion_sync_speed_profile, P::MAX_MOTION_SYNC_SPEED);
                assert_eq!(caps.max_motion_sync_speed, P::SUPPORTS_MOTION_SYNC.then_some(P::MAX_MOTION_SYNC_SPEED));
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

            /// Pins the per-profile protocol *defaults* the parity row
            /// `timeout-category-defaults-selection` depends on. Every built-in
            /// profile's acknowledgement deadline must equal the 1.x
            /// `TimeoutConfig` default of 500 ms, restored as the interim value
            /// (issue #689); its inquiry-response deadline must be the interim
            /// 1000 ms. The mapped selection test installs explicit tuning, so
            /// it cannot see these defaults drift — this sweep can, and covers
            /// every profile automatically so a newly added one cannot slip in
            /// a tighter default unnoticed.
            #[test]
            fn builtin_profile_default_deadlines_match_the_1x_ack_and_interim_inquiry() {
                // 1.x `TimeoutConfig::default().ack_timeout` — the deadline every
                // 1.x profile actually scheduled under (the per-profile
                // `ACK_TIMEOUT` const was defined but never read).
                const ONE_X_ACK_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis(500);
                // Interim inquiry-response deadline. 1.x inquiries carried no
                // dedicated deadline and used the 5 s `Quick` category budget;
                // v2 holds this at 1000 ms pending the hardware pass (waiver
                // `inquiry-deadline-interim-default`).
                const INTERIM_INQUIRY_TIMEOUT: std::time::Duration =
                    std::time::Duration::from_millis(1000);
                $(
                    let timing = $crate::ProfileSpec::from_compile_time::<$profile>()
                        .unwrap_or_else(|error| {
                            panic!("{} ProfileSpec failed: {error}", stringify!($profile))
                        })
                        .timing();
                    assert_eq!(
                        timing.ack_timeout(),
                        ONE_X_ACK_TIMEOUT,
                        "{} ACK deadline must match the restored 1.x 500 ms default",
                        stringify!($profile)
                    );
                    assert_eq!(
                        timing.inquiry_timeout(),
                        INTERIM_INQUIRY_TIMEOUT,
                        "{} inquiry deadline must be the interim 1000 ms value",
                        stringify!($profile)
                    );
                )*
            }

            #[test]
            fn scalar_position_inquiry_matrix_is_conservative_and_explicit() {
                let expected = [
                    (ProfileId::PtzOpticsG2, true, false),
                    (ProfileId::PtzOpticsG3, true, false),
                    (ProfileId::PtzOptics30X, true, false),
                    (ProfileId::SonyFr7, false, true),
                    (ProfileId::SonyBrcH900, true, false),
                    (ProfileId::SonyEviH100, true, false),
                    (ProfileId::SonyBrc300, true, false),
                    (ProfileId::NearusBrc300, true, false),
                    (ProfileId::GenericVisca, true, false),
                ];
                for (id, iris, nd_filter) in expected {
                    let facts = id.registry_facts();
                    assert_eq!(facts.position_inquiries.iris(), iris, "{id:?} iris inquiry");
                    assert_eq!(
                        facts.position_inquiries.nd_filter(),
                        nd_filter,
                        "{id:?} ND-filter inquiry"
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
                fn assert_shared_ae_mode_profile<P>()
                where
                    P: $crate::capabilities::Profile + $crate::capabilities::HasExposureMode,
                {
                    assert!(
                        !P::EXPOSURE_MODES.is_empty(),
                        "shared exposure-mode marker requires a source-backed mode inventory"
                    );
                    assert!(
                        <P as $crate::capabilities::ProfileTypedSupport>::TYPED_SUPPORT.contains(
                            $crate::capabilities::TypedSupportSurface::ExposureMode
                        ),
                        "shared exposure-mode marker and typed support must come from the same registry row"
                    );
                }

                assert_shared_ae_mode_profile::<PtzOpticsG2>();
                assert_shared_ae_mode_profile::<PtzOpticsG3>();
                assert_shared_ae_mode_profile::<PtzOptics30X>();
                assert_shared_ae_mode_profile::<SonyBRCH900>();
                assert_shared_ae_mode_profile::<SonyEVIH100>();
                assert_shared_ae_mode_profile::<SonyBRC300>();
                assert_shared_ae_mode_profile::<NearusBRC300>();
                assert_shared_ae_mode_profile::<GenericVisca>();

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
                    assert!(P::SUPPORTS_FOCUS_ZONE);
                    assert!(!P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY);
                    assert_eq!(P::MAX_PRESETS, 127);
                    assert!(P::SUPPORTS_PICTURE_EFFECT);
                    assert!(!P::SUPPORTS_PRESET_TOUR);
                }

                assert_ptzoptics_raw_profile::<PtzOpticsG2>();
                assert_ptzoptics_raw_profile::<PtzOpticsG3>();
                assert_ptzoptics_raw_profile::<PtzOptics30X>();

                assert!(
                    !<PtzOpticsG2 as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                assert!(
                    <PtzOpticsG3 as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                assert!(
                    <PtzOptics30X as $crate::capabilities::ProfileMetadata>::SUPPORTS_COMMAND_CANCEL
                );
                assert!(ProfileId::PtzOpticsG2
                    .registry_facts()
                    .evidence
                    .iter()
                    .any(|item| item.key == "command_cancel"));

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
                let nr_profiles = "`PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`";
                for surface in [
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3DControl,
                ] {
                    assert_eq!(registry_profile_type_names_for(surface), nr_profiles, "{surface:?}");
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
                        $crate::capabilities::TypedSupportSurface::OnePushFocus
                            | $crate::capabilities::TypedSupportSurface::PtzOpticsSnapFocus
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
                    "2D mode and 2D/3D noise-reduction level inquiries",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
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
                    "2D mode and 2D/3D noise-reduction level controls",
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl,
                );
                assert_eq!(
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2DControl
                    ),
                    registry_profile_type_names_for(
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3DControl
                    )
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
                    doc: "PtzOptics G2-series camera profile.",
                    id: PtzOpticsG2,
                    id_doc: "PtzOptics G2 series cameras",
                    id_attrs: [],
                    group: PtzOpticsG2,
                    vendor: "PtzOptics",
                    description: "G2-series PTZ camera with 12x, 20x, and 30x optical variants and 128 presets",
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        focus_zone_inquiry: true,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(range!(u16, 0, 17)),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp: true,
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        rgb_gain: true,
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
                        hue: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 127,
                        speed_range: range!(u8, 1, 24),
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
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    evidence: [
                        ("command_cancel", "No model- and firmware-identified source-backed evidence establishes standard VISCA socket-cancel support for G2; keep it unavailable pending documented validation."),
                        ("digital_zoom", "No model- and firmware-identified source-backed evidence establishes VISCA digital-zoom control for G2; keep it unavailable pending documented validation."),
                        ("iris", "R1 and the current PTZOptics G2/G3 Developer Portal (R14 in docs/visca_reference.md) document G2 `04 39`, iris reset/up/down, direct `04 4B`, and `09 04 4B` position inquiry. They omit the distinct `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the G2 family."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the 81 2A 02 A0 04 USB-audio command and matching inquiry for G2 models."),
                        ("noise_reduction", "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain."),
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
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
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(range!(u16, 0, 17)),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp: true,
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        rgb_gain: true,
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
                        hue: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 127,
                        speed_range: range!(u8, 1, 24),
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
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    evidence: [
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
                    group: PtzOpticsG2,
                    vendor: "PtzOptics",
                    description: "Legacy PT30X SDI/NDI G2 raw-VISCA profile with 30x optical zoom",
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 0,
                        inquiry_support: $crate::capabilities::InquirySupport::Full,
                        supports_operation_complete: true,
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
                        magnification_to_units: 565.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: true,
                        focus_zone_inquiry: true,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: false,
                    },
                    exposure: {
                        modes: profile_constants::PTZ_OPTICS_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x0C)),
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: Some(range!(u16, 0, 17)),
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::PTZ_OPTICS_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -10, 10)),
                        bg_tuning_range: Some(range!(i8, -10, 10)),
                        color_temp: true,
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        rgb_gain: true,
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
                        hue: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: true,
                        picture_effect: true,
                        luminance_range: Some(range!(u8, 0, 14)),
                        combined_flip: true,
                        save_after_flip: true,
                        gamma: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 127,
                        speed_range: range!(u8, 1, 24),
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
                        NoiseReduction3D,
                        NoiseReduction2DControl,
                        NoiseReduction3DControl,
                        PictureEffect,
                    ],
                    evidence: [
                        ("digital_zoom", "The Axis 0x7AC0 digital endpoint is not applied to PTZOptics; the 30X raw VISCA profile keeps the standard 0x4000 optical endpoint and no typed digital zoom."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                        ("iris", "The shared PTZOptics Gen-2 source documents iris priority, relative controls, direct `04 4B`, and matching `09 04 4B` position inquiry for the raw 30X profile. It does not establish a model-specific `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the raw 30X model."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the USB-audio command and matching inquiry for the raw 30X model."),
                        ("noise_reduction", "R1 supplies legacy PT30X SDI/NDI G2 09 04 50/53/54 inquiries, including 3D 0..8, but no setters. R14 supplies 04 50/53/54 controls only because R15 explicitly narrows this profile to that legacy PT30X SDI/NDI G2 raw-VISCA family. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by query output domains; do not generalize this evidence to newer 30X products."),
                    ],
                }

                profile SonyFR7 {
                    doc: "Sony FR7 camera profile.",
                    id: SonyFr7,
                    id_doc: "Sony FR7 camera",
                    id_attrs: [],
                    group: SonyProfessional,
                    vendor: "Sony",
                    description: "Professional cinema camera with variable ND filter and model-specific exposure controls",
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 8000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 8000,
                        },
                        inquiry_timeout_ms: 1000,
                        cancellation_timeout_ms: 1000,
                        ambiguity_timeout_ms: 1000,
                        busy_timeout_ms: 240,
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
                        magnification_to_units: 1000.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::NO_SHARED_EXPOSURE_MODES,
                        iris_range: None,
                        shutter_speeds: profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 15),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: true,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_FR7_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -7, 7)),
                        bg_tuning_range: Some(range!(i8, -7, 7)),
                        color_temp: false,
                        color_temp_range: None,
                        rgb_gain: true,
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
                        hue: true,
                        hue_range: Some(range!(u8, 0, 14)),
                        noise_reduction: false,
                        nr_2d: false,
                        nr_3d: false,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 255,
                        speed_range: range!(u8, 1, 24),
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
                    evidence: [
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 6000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 6000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        magnification_to_units: 862.3,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xF000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::BRC_H900_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x1E)),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 15),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: true,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -7, 7)),
                        bg_tuning_range: Some(range!(i8, -7, 7)),
                        color_temp: true,
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        rgb_gain: false,
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
                        hue: false,
                        hue_range: None,
                        noise_reduction: true,
                        nr_2d: true,
                        nr_3d: true,
                        luminance: false,
                        picture_effect: false,
                        luminance_range: None,
                        combined_flip: false,
                        save_after_flip: false,
                        gamma: true,
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 100,
                        speed_range: range!(u8, 1, 24),
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
                    motion_sync: { supported: false, max_speed: 24 },
                    nd_filter: { mode: $crate::capabilities::NdFilterMode::None, steps: None },
                    variable_speed: { supported: false },
                    typed_support: [
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
                    evidence: [
                        ("sony_spotlight", "The BRC-H900 command list (R11 in docs/visca_reference.md) documents the fixed 04 3A spotlight commands, but not the fixed 04 5A auto slow-shutter commands."),
                        ("tally", "The BRC-H900 source-backed command list does not establish the FR7 red/green tally family; typed tally remains unavailable."),
                        ("exposure_mode", "The BRC-H900 command list R11 lines 706-717 and 1003 documents the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri commands and `09 04 39` inquiry. Bright mode is not listed and is therefore absent from this profile's inventory."),
                        ("iris", "The BRC-H900 command list R11 lines 706-717 and 1012 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry. The distinct `09 04 2B` status inquiry remains unavailable."),
                        ("brightness", "The BRC-H900 model command list does not establish the exposure-brightness control or inquiry; retain no brightness range or typed marker."),
                        ("picture_effect", "The BRC-H900 model command list does not establish picture-effect control or inquiry; leave the typed surface unavailable."),
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        pan_range: range!(i32, -1440, 1440),
                        tilt_range: range!(i32, -480, 480),
                        max_pan_speed: 18,
                        max_tilt_speed: 18,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 16.0,
                        tilt_degrees_to_units: 16.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x4000,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
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
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x1B)),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: range!(i8, -7, 7),
                        wdr: false,
                    },
                    white_balance: {
                        modes: profile_constants::SONY_COLOR_TEMP_WB_MODES,
                        one_push: true,
                        rg_tuning_range: Some(range!(i8, -7, 7)),
                        bg_tuning_range: Some(range!(i8, -7, 7)),
                        color_temp: true,
                        color_temp_range: Some(range!(u16, 2500, 8000)),
                        rgb_gain: false,
                        red_gain_range: None,
                        blue_gain_range: None,
                    },
                    image: {
                        base_support: true,
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
                        gamma_range: Some(range!(u8, 0, 4)),
                    },
                    presets: {
                        max: 6,
                        speed_range: range!(u8, 1, 19),
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
                        SonyAutoSlowShutter,
                        ExposureMode,
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
                    ],
                    evidence: [
                        ("sony_auto_slow_shutter", "The EVI-H100 technical manual (R8 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("exposure_mode", "Decision D4 in #716 retains the EVI-H100 1.2 compatibility breadth for the standard `04 39` family pending a direct line-item audit of the model authority R8; do not remove it without a contradictory model-specific citation."),
                        ("iris", "Decision D4 in #716 retains the EVI-H100 1.2 compatibility breadth for standard iris reset/up/down, direct `04 4B`, and the position inquiry pending a direct R8 line-item audit. The distinct `09 04 2B` status inquiry remains unavailable."),
                    ],
                }

                profile SonyBRC300 {
                    doc: "Sony BRC-300 camera profile.",
                    id: SonyBrc300,
                    id_doc: "Sony BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "sony-brc300"))]],
                    group: GenericVisca,
                    vendor: "Sony",
                    description: "Legacy PTZ camera with signed-centered 20-bit pan and 16-bit tilt coordinates",
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        // The library's positive-pan/right and negative-tilt/up
                        // degree convention is therefore opposite both raw axes.
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
                        // negative sign is the documented raw-axis polarity.
                        pan_degrees_to_units: -208.0,
                        tilt_degrees_to_units: -208.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                    },
                    zoom: {
                        optical_max: 0x1068,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
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
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x10)),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 6),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: range!(i8, -7, 7),
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
                        base_support: false,
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
                        // Sony BRC-300 technical manual, pp. 11–14:
                        // CAM_Memory accepts p = 0..=5, and Cmd_PT_M_Speed
                        // documents q = 1..=24. These discovery facts do not
                        // establish compatibility with the PTZOptics typed
                        // preset-recall-speed command family.
                        max: 5,
                        speed_range: range!(u8, 1, 24),
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
                        SonyAutoSlowShutter,
                        ExposureMode,
                        DirectZoom,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                    ],
                    evidence: [
                        ("sony_auto_slow_shutter", "The BRC-300 technical manual (R12 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("exposure_mode", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri/Bright commands and `09 04 39` inquiry."),
                        ("iris", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry. The distinct `09 04 2B` status inquiry remains unavailable."),
                    ],
                }

                profile NearusBRC300 {
                    doc: "Nearus BRC-300 camera profile.",
                    id: NearusBrc300,
                    id_doc: "Nearus BRC-300 camera",
                    id_attrs: [#[cfg_attr(feature = "serde", serde(rename = "nearus-brc300"))]],
                    group: GenericVisca,
                    vendor: "Nearus",
                    description: "Nearus PTZ camera profile with conservatively modeled standard VISCA pan/tilt framing",
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 5000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 5000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        pan_range: range!(i32, -1170, 1170),
                        tilt_range: range!(i32, -390, 390),
                        max_pan_speed: 18,
                        max_tilt_speed: 17,
                        simultaneous: true,
                        preset_recovery_ms: 0,
                        pan_degrees_to_units: 13.0,
                        tilt_degrees_to_units: 13.0,
                        coordinate_system: $crate::capabilities::CoordinateSystem::UnsignedCentered,
                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                    },
                    zoom: {
                        optical_max: 0x1068,
                        digital_max: None,
                        speed_range: range!(u8, 0, 7),
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
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x10)),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 6),
                        brightness_range: None,
                        backlight_comp: true,
                        exposure_comp: false,
                        exposure_comp_range: range!(i8, -7, 7),
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
                        base_support: true,
                        contrast_range: None,
                        sharpness_range: None,
                        saturation_range: Some(range!(u8, 0, 15)),
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
                        speed_range: range!(u8, 1, 17),
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
                        ExposureMode,
                        DirectZoom,
                        IrisControl,
                        FocusNearLimitInquiry,
                        BacklightCompensation,
                        SaturationControl,
                    ],
                    evidence: [
                        ("pan_tilt_wire", "No independent Nearus model source establishes Sony BRC-300's one-speed, five-pan-nibble position frame. The profile therefore exposes conservative standard 4+4 VISCA pan/tilt framing pending model-specific validation."),
                        ("sony_vendor_exposure", "No independent Nearus BRC-300 source establishes the fixed 04 3A spotlight or 04 5A auto slow-shutter command family, so both typed markers remain unavailable."),
                        ("exposure_mode", "As the BRC-300 compatibility profile, Nearus BRC-300 follows the standard shared `04 39` family documented by Sony R12 while model-specific vendor exposure extensions remain withheld."),
                        ("iris", "As the BRC-300 compatibility profile, Nearus BRC-300 follows the standard iris `04 0B`/`04 4B` controls and `09 04 4B` position inquiry documented by Sony R12. The distinct status inquiry remains unavailable."),
                    ],
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
                        ack_timeout_ms: 500,
                        command_timeouts_ms: {
                            quick: 10000,
                            movement: 30000,
                            preset: 60000,
                            long_running: 300000,
                            network: 10000,
                        },
                        inquiry_timeout_ms: 1000,
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
                        magnification_to_units: 1000.0,
                    },
                    focus: {
                        near_limit: 0x1000,
                        far_limit: 0xE000,
                        auto_focus: true,
                        one_push: false,
                        focus_zone: false,
                        focus_zone_inquiry: false,
                        max_speed: 7,
                        af_sensitivity: false,
                        near_limit_inquiry: true,
                    },
                    exposure: {
                        modes: profile_constants::STANDARD_EXPOSURE_MODES,
                        iris_range: Some(range!(u16, 0x00, 0x1B)),
                        shutter_speeds: profile_constants::GENERIC_VISCA_SHUTTER_SPEEDS,
                        gain_range: range!(u8, 0, 7),
                        brightness_range: None,
                        backlight_comp: false,
                        exposure_comp: false,
                        exposure_comp_range: range!(i8, -7, 7),
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
                        base_support: false,
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
                        speed_range: range!(u8, 1, 23),
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
                        ExposureMode,
                        IrisControl,
                        FocusNearLimitInquiry,
                        OnePushWhiteBalance,
                    ],
                    evidence: [
                        ("direct_zoom", "Generic profile keeps absolute zoom positioning unavailable despite baseline zoom movement."),
                        ("exposure_mode", "Generic VISCA deliberately assumes the standard Sony `04 39` exposure-mode family documented by R11/R12, matching its 1.2 compatibility contract."),
                        ("iris", "Generic VISCA deliberately assumes the standard Sony `04 0B`/`04 4B` iris controls and `09 04 4B` position inquiry documented by R11/R12, matching its 1.2 compatibility contract."),
                    ],
                }
            }
        }
    };
}
