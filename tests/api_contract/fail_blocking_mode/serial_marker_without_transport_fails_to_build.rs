//@ build
#![cfg(feature = "blocking")]
//! A serial marker without serial transport facts fails at construction.

use std::time::Duration;

use grafton_visca::{
    camera::CameraConfig,
    capabilities::{
        exposure::ShutterSpeedEntry, CapabilityDomain, CapabilityRange, Exposure, Focus,
        ImageProcessing, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt, Power,
        Presets, ProfileMetadata, ProfileTypedSupport, SupportsSerial, Tally, TypedSupportSet,
        VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    profiles::GenericVisca as Base,
    transport::RawVisca,
    AffectedAxes, CommandTimeouts, CompileTimeProfile, PositionInquirySupport,
    TransportCompatibility, WhiteBalanceMode,
};

#[derive(Debug, Default, Clone, Copy)]
struct SerialOnly;

impl ProfileMetadata for SerialOnly {
    const INQUIRY_SUPPORT: grafton_visca::capabilities::InquirySupport =
        grafton_visca::capabilities::InquirySupport::Full;
    const SUPPORTS_COMMAND_CANCEL: bool = true;

    const MODEL_NAME: &'static str = "SerialOnly";
    const DEFAULT_CAMERA_ID: grafton_visca::IndividualCameraId =
        grafton_visca::IndividualCameraId::new(1).expect("individual profile default");
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = <Base as ProfileMetadata>::ACK_TIMEOUT;
    const COMMAND_TIMEOUTS: CommandTimeouts = CommandTimeouts::DEFAULT;
}
impl PanTilt for SerialOnly {
    const PAN_RANGE: CapabilityRange<i32> = <Base as PanTilt>::PAN_RANGE;
    const TILT_RANGES: grafton_visca::capabilities::TiltRanges =
        grafton_visca::capabilities::TiltRanges::Single(<Base as PanTilt>::TILT_RANGES.range(None));
    const MAX_PAN_SPEED: u8 = <Base as PanTilt>::MAX_PAN_SPEED;
    const MAX_TILT_SPEED: u8 = <Base as PanTilt>::MAX_TILT_SPEED;
    const PAN_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::PAN_DEGREES_TO_UNITS;
    const TILT_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::TILT_DEGREES_TO_UNITS;
}
impl Zoom for SerialOnly {
    const SUPPORTS_DIRECT_ZOOM: bool = true;
    const SUPPORTS_VARIABLE_ZOOM: bool = true;

    const OPTICAL_ZOOM_MAX: u16 = <Base as Zoom>::OPTICAL_ZOOM_MAX;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: CapabilityRange<u8> = <Base as Zoom>::ZOOM_SPEED_RANGE;
    const OPTICAL_ZOOM_RATIO: Option<f32> = None;
}
impl Focus for SerialOnly {
    const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = true;

    const MAX_FOCUS_SPEED: u8 = 7;

    const FOCUS_RANGE: grafton_visca::capabilities::CapabilityRange<u16> =
        grafton_visca::capabilities::CapabilityRange::<u16>::new(
            <Base as Focus>::FOCUS_RANGE.min(),
            <Base as Focus>::FOCUS_RANGE.max(),
        );
    const FOCUS_NEAR_END: grafton_visca::capabilities::FocusNearEnd =
        grafton_visca::capabilities::FocusNearEnd::Minimum;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}
impl Exposure for SerialOnly {
    const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
    const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = <Base as Exposure>::SHUTTER_SPEEDS;
    const GAIN_RANGE: Option<grafton_visca::capabilities::CapabilityRange<u8>> =
        <Base as Exposure>::GAIN_RANGE;
    const SUPPORTS_BACKLIGHT_COMP: bool = false;
}
impl WhiteBalance for SerialOnly {
    const WB_MODES: &'static [WhiteBalanceMode] = &[WhiteBalanceMode::Auto];
    const SUPPORTS_ONE_PUSH_WB: bool = false;
    const RG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
    const BG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
}
impl ImageProcessing for SerialOnly {
    const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
    const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
    const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
    const SUPPORTS_FLIP: bool = false;
    const SUPPORTS_MIRROR: bool = false;
}
impl Presets for SerialOnly {
    const HIGHEST_PRESET: u8 = 5;
    const PRESET_SPEED_RANGE: Option<CapabilityRange<u8>> = None;
    const SUPPORTS_PRESET_TOUR: bool = false;
}
impl Power for SerialOnly {
    const POWER_ON_TIME: Duration = Duration::from_secs(5);
    const SUPPORTS_STANDBY: bool = false;
}
impl MenuCapability for SerialOnly {}
impl Tally for SerialOnly {}
impl MotionSyncMetadata for SerialOnly {}
impl NdFilterMetadata for SerialOnly {}
impl VariableSpeedMetadata for SerialOnly {}
impl ProfileTypedSupport for SerialOnly {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
}
impl CompileTimeProfile for SerialOnly {
    const TRANSPORTS: TransportCompatibility = TransportCompatibility::new(None, Some(1259), false);
    const INQUIRY_TIMEOUT: Duration = Duration::from_secs(1);
    const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(1);
    const AMBIGUITY_TIMEOUT: Duration = Duration::from_secs(1);
    const MAXIMUM_COMMAND_SOCKETS: u8 = 2;
    const PRESET_RECALL_AXES: Option<AffectedAxes> =
        Some(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM));
    const POSITION_INQUIRIES: PositionInquirySupport =
        PositionInquirySupport::new(true, true, true);
}

impl SupportsSerial for SerialOnly {}
fn main() {
    let _ = CameraConfig::<SerialOnly>::serial("/never-opened", 9600);
}

//~ E0080
//~ "a profile implementing `SupportsSerial` must declare serial support in `TRANSPORTS`"
