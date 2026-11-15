//@ build
#![cfg(feature = "blocking")]
//! A static ND marker without its runtime bit fails at monomorphization.

use std::time::Duration;

use grafton_visca::{
    camera::CameraConfig,
    capabilities::{
        exposure::ShutterSpeedEntry, CapabilityDomain, CapabilityRange, Exposure, Focus,
        ImageProcessing, HasNdFilter, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt, Power,
        Presets, ProfileMetadata, ProfileTypedSupport, SupportsTcp, SupportsUdp, Tally,
        TypedSupportSet, VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    profiles::GenericVisca as Base,
    transport::RawVisca,
    AffectedAxes, CommandTimeouts, CompileTimeProfile, PositionInquirySupport,
    TransportCompatibility, WhiteBalanceMode,
};

#[derive(Debug, Default, Clone, Copy)]
struct SerialOnly;

impl ProfileMetadata for SerialOnly {
    const MODEL_NAME: &'static str = "SerialOnly";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = <Base as ProfileMetadata>::ACK_TIMEOUT;
    const COMMAND_TIMEOUTS: CommandTimeouts = CommandTimeouts::DEFAULT;
}
impl PanTilt for SerialOnly {
    const PAN_RANGE: CapabilityRange<i32> = <Base as PanTilt>::PAN_RANGE;
    const TILT_RANGE: CapabilityRange<i32> = <Base as PanTilt>::TILT_RANGE;
    const MAX_PAN_SPEED: u8 = <Base as PanTilt>::MAX_PAN_SPEED;
    const MAX_TILT_SPEED: u8 = <Base as PanTilt>::MAX_TILT_SPEED;
    const PAN_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::PAN_DEGREES_TO_UNITS;
    const TILT_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::TILT_DEGREES_TO_UNITS;
}
impl Zoom for SerialOnly {
    const OPTICAL_ZOOM_MAX: u16 = <Base as Zoom>::OPTICAL_ZOOM_MAX;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: CapabilityRange<u8> = <Base as Zoom>::ZOOM_SPEED_RANGE;
    const OPTICAL_ZOOM_RATIO: Option<f32> = None;
}
impl Focus for SerialOnly {
    const FOCUS_NEAR_LIMIT: u16 = <Base as Focus>::FOCUS_NEAR_LIMIT;
    const FOCUS_FAR_LIMIT: u16 = <Base as Focus>::FOCUS_FAR_LIMIT;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}
impl Exposure for SerialOnly {
    const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
    const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = <Base as Exposure>::SHUTTER_SPEEDS;
    const GAIN_RANGE: CapabilityRange<u8> = <Base as Exposure>::GAIN_RANGE;
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
impl NdFilterMetadata for SerialOnly {
    const ND_MODE: grafton_visca::capabilities::NdFilterMode = grafton_visca::capabilities::NdFilterMode::Variable;
}
impl HasNdFilter for SerialOnly {}
impl VariableSpeedMetadata for SerialOnly {}
impl ProfileTypedSupport for SerialOnly {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
}
impl CompileTimeProfile for SerialOnly {
    const TRANSPORTS: TransportCompatibility = TransportCompatibility::new(None, None, true);
    const INQUIRY_TIMEOUT: Duration = Duration::from_secs(1);
    const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(1);
    const AMBIGUITY_TIMEOUT: Duration = Duration::from_secs(1);
    const MAXIMUM_COMMAND_SOCKETS: u8 = 2;
    const PRESET_RECALL_AXES: Option<AffectedAxes> =
        Some(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM));
    const POSITION_INQUIRIES: PositionInquirySupport =
        PositionInquirySupport::new(true, true, true);
}

fn main() {
    let camera: Option<grafton_visca::blocking::Camera<SerialOnly>> = None;
    let _ = camera.unwrap().nd_filter().set_value(grafton_visca::command::NdFilterValue::new(0).unwrap());
}

//~ E0080
//~ "profile marker `HasNdFilter` requires its typed-support bit"
