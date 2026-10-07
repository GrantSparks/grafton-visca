//! Synthetic compile-time profiles and the session configurations built on
//! them, shared by the integration suite; each test binary uses a different
//! subset.

use std::time::Duration;

use grafton_visca::{
    capabilities::{
        self, exposure::ShutterSpeedEntry, CapabilityDomain, CapabilityRange, Exposure, Focus,
        HasDirectZoom, HasMotionSync, ImageProcessing, InquirySupport, MenuCapability,
        MotionSyncMetadata, NdFilterMetadata, PanTilt, Power, Presets, ProfileMetadata,
        ProfileTypedSupport, Tally, TypedSupportSet, TypedSupportSurface, VariableSpeedMetadata,
        WhiteBalance, Zoom,
    },
    command::{ExposureMode, FocusZone},
    transport::RawVisca,
    AffectedAxes, CameraId, CommandTimeouts, CompileTimeProfile, PositionInquirySupport,
    ProfileSpec, SessionConfig, TransportCompatibility, WhiteBalanceMode,
};

const EXPOSURE_MODES: &[ExposureMode] = &[
    ExposureMode::Auto,
    ExposureMode::Manual,
    ExposureMode::Shutter,
    ExposureMode::Iris,
    ExposureMode::Bright,
];
const SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[ShutterSpeedEntry::new(
    match grafton_visca::units::Fraction::new(1, 60) {
        Some(exposure) => exposure,
        None => panic!("nonzero denominator"),
    },
    0x01,
)];
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
            const OPTICAL_ZOOM_RATIO: Option<f32> = None;
        }

        impl Focus for $profile {
            const FOCUS_NEAR_LIMIT: u16 = 0x1000;
            const FOCUS_FAR_LIMIT: u16 = 0xF000;
            const SUPPORTS_AUTO_FOCUS: bool = true;
            const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
            const FOCUS_ZONES: &'static [FocusZone] =
                &[FocusZone::Top, FocusZone::Center, FocusZone::Bottom];
            const SUPPORTS_AF_SENSITIVITY: bool = true;
        }

        impl Exposure for $profile {
            const EXPOSURE_MODES: &'static [ExposureMode] = EXPOSURE_MODES;
            const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
            const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = SHUTTER_SPEEDS;
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
            const HIGHEST_PRESET: u8 = 6;
            const PRESET_SPEED_RANGE: Option<CapabilityRange<u8>> =
                Some(CapabilityRange::<u8>::new(1, 23));
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

// The generic noun contracts in the suite bound a profile on many typed
// surfaces at once, and no built-in profile documents all of them (none
// documents motion sync at all). This fixture declares every surface those
// contracts name, so each contract is instantiated rather than only
// type-checked in the abstract.
synthetic_profile!(
    OptionalSurfacesTypedSupport,
    "Optional Surfaces Typed Support",
    TypedSupportSet::from_surfaces(&[
        TypedSupportSurface::AutoFocusSensitivity,
        TypedSupportSurface::AutoWhiteBalanceSensitivity,
        TypedSupportSurface::BacklightCompensation,
        TypedSupportSurface::BrightnessControl,
        TypedSupportSurface::ColorTemperature,
        TypedSupportSurface::ColorTemperatureInquiry,
        TypedSupportSurface::ContrastControl,
        TypedSupportSurface::ExposureCompensation,
        TypedSupportSurface::FocusNearLimitInquiry,
        TypedSupportSurface::FocusZoneInquiry,
        TypedSupportSurface::GammaControl,
        TypedSupportSurface::HueControl,
        TypedSupportSurface::ImageFlip,
        TypedSupportSurface::IrisControl,
        TypedSupportSurface::IrisControlInquiry,
        TypedSupportSurface::LuminanceControl,
        TypedSupportSurface::MotionSync,
        TypedSupportSurface::NdFilter,
        TypedSupportSurface::NoiseReduction2D,
        TypedSupportSurface::NoiseReduction2DMode,
        TypedSupportSurface::NoiseReduction3D,
        TypedSupportSurface::PictureEffect,
        TypedSupportSurface::RgbGain,
        TypedSupportSurface::RgbTuning,
        TypedSupportSurface::SaturationControl,
        TypedSupportSurface::SharpnessControl,
        TypedSupportSurface::Tally,
        TypedSupportSurface::WideDynamicRange,
    ])
);

impl MotionSyncMetadata for DirectZoomOnlyTypedSupport {}
impl MotionSyncMetadata for NonDefaultCompileTimeProfile {}
impl MotionSyncMetadata for MotionOwnerCompileTimeProfile {}
impl MotionSyncMetadata for QuarantinedSocketCompileTimeProfile {}

impl MotionSyncMetadata for OptionalSurfacesTypedSupport {
    const MOTION_SYNC_SPEED_RANGE: Option<CapabilityRange<u8>> =
        Some(CapabilityRange::<u8>::new(1, 24));
}

/// The one fixture that documents the physical capability, so the typed
/// `MotionSync` surface above has something real to gate.
impl MotionSyncMetadata for MotionSyncTypedSupport {
    const MOTION_SYNC_SPEED_RANGE: Option<CapabilityRange<u8>> =
        Some(CapabilityRange::<u8>::new(1, 24));
}

impl HasDirectZoom for DirectZoomOnlyTypedSupport {}

impl HasMotionSync for MotionSyncTypedSupport {}

impl capabilities::HasAutoFocusSensitivity for OptionalSurfacesTypedSupport {}
impl capabilities::HasAutoWhiteBalanceSensitivity for OptionalSurfacesTypedSupport {}
impl capabilities::HasBacklightCompensation for OptionalSurfacesTypedSupport {}
impl capabilities::HasBrightnessControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasColorTemperature for OptionalSurfacesTypedSupport {}
impl capabilities::HasColorTemperatureInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasContrastControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasExposureCompensation for OptionalSurfacesTypedSupport {}
impl capabilities::HasFocusNearLimitInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasFocusZoneInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasGammaControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasHueControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasImageFlip for OptionalSurfacesTypedSupport {}
impl capabilities::HasIrisControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasIrisControlInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasLuminanceControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasMotionSync for OptionalSurfacesTypedSupport {}
impl capabilities::HasNdFilter for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction2D for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction2DMode for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction3D for OptionalSurfacesTypedSupport {}
impl capabilities::HasPictureEffect for OptionalSurfacesTypedSupport {}
impl capabilities::HasRgbGain for OptionalSurfacesTypedSupport {}
impl capabilities::HasRgbTuning for OptionalSurfacesTypedSupport {}
impl capabilities::HasSaturationControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasSharpnessControl for OptionalSurfacesTypedSupport {}
impl capabilities::HasTally for OptionalSurfacesTypedSupport {}
impl capabilities::HasWideDynamicRange for OptionalSurfacesTypedSupport {}
impl capabilities::HasImageProcessing for OptionalSurfacesTypedSupport {}

/// A session on camera 1 with [`NonDefaultCompileTimeProfile`], the
/// two-socket raw profile.
pub fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw profile"),
    )
}

/// [`session_config`] with camera 2 registered on the same two-socket raw
/// profile.
pub fn two_camera_session_config() -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket raw profile"),
        )
        .expect("second target");
    config
}

/// A session on camera 1 with the built-in Sony FR7 profile (Sony envelope).
pub fn sony_session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyFR7>()
            .expect("Sony FR7 profile"),
    )
}
