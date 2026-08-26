use std::time::Duration;

use grafton_visca::{
    capabilities::{
        exposure::ShutterSpeed, CapabilityRange, Exposure, Focus, HasDirectZoom, ImageProcessing,
        InquirySupport, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt, Power,
        Presets, ProfileMetadata, ProfileTypedSupport, Tally, TypedSupportSet, TypedSupportSurface,
        VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    command::ExposureMode,
    transport::RawVisca,
    AffectedAxes, CompileTimeProfile, PositionInquirySupport, TransportCompatibility,
    WhiteBalanceMode,
};

const EXPOSURE_MODES: &[ExposureMode] = &[
    ExposureMode::Auto,
    ExposureMode::Manual,
    ExposureMode::Shutter,
    ExposureMode::Iris,
    ExposureMode::Bright,
];
const SHUTTER_SPEEDS: &[ShutterSpeed] = &[ShutterSpeed::new("1/60", 0x01)];
const WB_MODES: &[WhiteBalanceMode] = &[WhiteBalanceMode::Auto, WhiteBalanceMode::Manual];

macro_rules! synthetic_profile_default {
    (default, $profile:ident) => {
        impl Default for $profile {
            fn default() -> Self {
                Self
            }
        }
    };
    (no_default, $profile:ident) => {};
}

macro_rules! synthetic_profile_impl {
    ($profile:ident, $model:literal, $typed_support:expr, $default:ident) => {
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy)]
        pub struct $profile;

        synthetic_profile_default!($default, $profile);

        impl ProfileMetadata for $profile {
            const MODEL_NAME: &'static str = $model;
            const DEFAULT_CAMERA_ID: u8 = 1;
            type Envelope = RawVisca;
            const ACK_TIMEOUT: Duration = Duration::from_millis(100);
            const COMPLETION_TIMEOUT: Duration = Duration::from_millis(1_000);
            const INQUIRY_SUPPORT: InquirySupport = InquirySupport::Full;
        }

        impl PanTilt for $profile {
            const PAN_RANGE: CapabilityRange<i16> = CapabilityRange::<i16>::new(-1700, 1700);
            const TILT_RANGE: CapabilityRange<i16> = CapabilityRange::<i16>::new(-300, 900);
            const MAX_PAN_SPEED: u8 = 24;
            const MAX_TILT_SPEED: u8 = 20;
            const PAN_DEGREES_TO_UNITS: f32 = 10.0;
            const TILT_DEGREES_TO_UNITS: f32 = 10.0;
        }

        impl Zoom for $profile {
            const OPTICAL_ZOOM_MAX: u16 = 0x4000;
            const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
            const ZOOM_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 7);
            const SUPPORTS_DIRECT_ZOOM: bool = true;
            const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
        }

        impl Focus for $profile {
            const FOCUS_NEAR_LIMIT: u16 = 0x1000;
            const FOCUS_FAR_LIMIT: u16 = 0xF000;
            const SUPPORTS_AUTO_FOCUS: bool = true;
            const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
            const SUPPORTS_FOCUS_ZONE: bool = true;
            const SUPPORTS_AF_SENSITIVITY: bool = true;
        }

        impl Exposure for $profile {
            const EXPOSURE_MODES: &'static [ExposureMode] = EXPOSURE_MODES;
            const IRIS_RANGE: Option<CapabilityRange<u16>> = None;
            const SHUTTER_SPEEDS: &'static [ShutterSpeed] = SHUTTER_SPEEDS;
            const GAIN_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 15);
            const SUPPORTS_BACKLIGHT_COMP: bool = false;
        }

        impl WhiteBalance for $profile {
            const WB_MODES: &'static [WhiteBalanceMode] = WB_MODES;
            const SUPPORTS_ONE_PUSH_WB: bool = false;
            const RG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
            const BG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
        }

        impl ImageProcessing for $profile {
            const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
            const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
            const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
            const SUPPORTS_FLIP: bool = false;
            const SUPPORTS_MIRROR: bool = false;
        }

        impl Presets for $profile {
            const MAX_PRESETS: u8 = 6;
            const PRESET_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(1, 23);
            const SUPPORTS_PRESET_TOUR: bool = false;
        }

        impl Power for $profile {
            const POWER_ON_TIME: Duration = Duration::from_secs(5);
            const SUPPORTS_STANDBY: bool = false;
        }

        impl MenuCapability for $profile {}
        impl Tally for $profile {}
        impl MotionSyncMetadata for $profile {}
        impl NdFilterMetadata for $profile {}
        impl VariableSpeedMetadata for $profile {}

        impl ProfileTypedSupport for $profile {
            const TYPED_SUPPORT: TypedSupportSet = $typed_support;
        }

        impl CompileTimeProfile for $profile {
            const TRANSPORTS: TransportCompatibility =
                TransportCompatibility::new(Some(5678), Some(1259), true);
            const INQUIRY_TIMEOUT: Duration = Duration::from_secs(1);
            const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(1);
            const AMBIGUITY_TIMEOUT: Duration = Duration::from_secs(1);
            const MAXIMUM_COMMAND_SOCKETS: u8 = 2;
            const PRESET_RECALL_AXES: Option<AffectedAxes> = Some(
                AffectedAxes::PAN_TILT
                    .union(AffectedAxes::ZOOM)
                    .union(AffectedAxes::FOCUS),
            );
            const POSITION_INQUIRIES: PositionInquirySupport =
                PositionInquirySupport::new(true, true, true);
        }
    };
}

macro_rules! synthetic_profile {
    ($profile:ident, $model:literal, $typed_support:expr) => {
        synthetic_profile_impl!($profile, $model, $typed_support, default);
    };
    ($profile:ident, $model:literal, $typed_support:expr, no_default) => {
        synthetic_profile_impl!($profile, $model, $typed_support, no_default);
    };
}

synthetic_profile!(
    MetadataEnabledNoTypedSupport,
    "Metadata Enabled Without Typed Support",
    TypedSupportSet::EMPTY
);

synthetic_profile!(
    DirectZoomOnlyTypedSupport,
    "Direct Zoom Typed Support Only",
    TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom)
);

synthetic_profile!(
    NonDefaultCompileTimeProfile,
    "Non-Default Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default
);

impl HasDirectZoom for DirectZoomOnlyTypedSupport {}
