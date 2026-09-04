use std::time::Duration;

use grafton_visca::{
    capabilities::{
        exposure::ShutterSpeed, CapabilityRange, Exposure, Focus, HasDirectZoom, HasMotionSync,
        ImageProcessing, InquirySupport, MenuCapability, MotionSyncMetadata, NdFilterMetadata,
        PanTilt, Power, Presets, ProfileMetadata, ProfileTypedSupport, Tally, TypedSupportSet,
        TypedSupportSurface, VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    command::ExposureMode,
    transport::RawVisca,
    AffectedAxes, CommandTimeouts, CompileTimeProfile, PositionInquirySupport,
    TransportCompatibility, WhiteBalanceMode,
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

macro_rules! synthetic_command_timeouts {
    () => {
        CommandTimeouts::new(
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        )
    };
    ($command_timeouts:expr) => {
        $command_timeouts
    };
}

macro_rules! synthetic_profile_impl {
    ($profile:ident, $model:literal, $typed_support:expr, $default:ident, $ambiguity_timeout:expr $(, $command_timeouts:expr)?) => {
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy)]
        pub struct $profile;

        synthetic_profile_default!($default, $profile);

        impl ProfileMetadata for $profile {
            const MODEL_NAME: &'static str = $model;
            const DEFAULT_CAMERA_ID: u8 = 1;
            type Envelope = RawVisca;
            const ACK_TIMEOUT: Duration = Duration::from_millis(100);
            const COMMAND_TIMEOUTS: CommandTimeouts = synthetic_command_timeouts!($($command_timeouts)?);
            const INQUIRY_SUPPORT: InquirySupport = InquirySupport::Full;
        }

        impl PanTilt for $profile {
            const PAN_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-1700, 1700);
            const TILT_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-300, 900);
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
            const AMBIGUITY_TIMEOUT: Duration = $ambiguity_timeout;
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
        synthetic_profile_impl!(
            $profile,
            $model,
            $typed_support,
            default,
            Duration::from_secs(1)
        );
    };
    ($profile:ident, $model:literal, $typed_support:expr, no_default) => {
        synthetic_profile_impl!(
            $profile,
            $model,
            $typed_support,
            no_default,
            Duration::from_secs(1)
        );
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

// Motion-owner tests intentionally chain successful raw position inquiries
// under exact one-second observer deadlines. Keep the production-style
// one-second pre-ACK ambiguity fact: matched inquiries no longer misuse it as
// a post-success dispatch hold (#712).
synthetic_profile_impl!(
    MotionOwnerCompileTimeProfile,
    "Motion Owner Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default,
    Duration::from_secs(1)
);

// Socket-reuse integration tests need a real owner timeout to install an
// exact-socket quarantine without making the test wait for a production-scale
// movement deadline. The distinct fixture keeps that timing fact local to
// those tests.
synthetic_profile_impl!(
    QuarantinedSocketCompileTimeProfile,
    "Quarantined Socket Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default,
    Duration::from_millis(250),
    CommandTimeouts::new(
        Duration::from_millis(100),
        Duration::from_millis(100),
        Duration::from_millis(100),
        Duration::from_millis(100),
        Duration::from_millis(100),
    )
);

// No built-in profile declares motion sync (see the profile registry's
// `MotionSync` note), so the typed accessor is only reachable from a profile
// that opts in — which is what makes a behavioural test of the helper possible
// at all.
synthetic_profile!(
    MotionSyncTypedSupport,
    "Motion Sync Typed Support Only",
    TypedSupportSet::from_surface(TypedSupportSurface::MotionSync)
);

impl MotionSyncMetadata for MetadataEnabledNoTypedSupport {}
impl MotionSyncMetadata for DirectZoomOnlyTypedSupport {}
impl MotionSyncMetadata for NonDefaultCompileTimeProfile {}
impl MotionSyncMetadata for MotionOwnerCompileTimeProfile {}
impl MotionSyncMetadata for QuarantinedSocketCompileTimeProfile {}

/// The one fixture that documents the physical capability, so the typed
/// `MotionSync` surface above has something real to gate.
impl MotionSyncMetadata for MotionSyncTypedSupport {
    const SUPPORTS_MOTION_SYNC: bool = true;
}

impl HasDirectZoom for DirectZoomOnlyTypedSupport {}

impl HasMotionSync for MotionSyncTypedSupport {}
