//! Downstream derive/profile smoke test with a renamed crate dependency.

use std::time::Duration;

use visca_renamed::{
    capabilities::{
        self, Exposure, Focus, ImageProcessing, MenuCapability, MotionSyncMetadata,
        NdFilterMetadata, PanTilt, Power, Presets, ProfileMetadata, ProfileTypedSupport, Tally,
        VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    command::{RawInquiryPayload, Response, ResponseParser},
    request, AffectedAxes, CameraId, CompileTimeProfile, Inquiry, PositionInquirySupport,
    ProfileSpec, Request, TransportCompatibility, ViscaEnum, ViscaInquiry, ViscaValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "1", max = "4")]
struct RenamedValue(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
enum RenamedEnum {
    First = 1,
    Second = 2,
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x7e, response = Raw)]
struct RenamedRawInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
struct RenamedResponseInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, typed_response = bool, typed_field = on)]
struct RenamedTypedInquiry;

fn assert_inquiry_contracts() {
    fn raw<T>()
    where
        T: Request<Class = request::Inquiry> + Inquiry<Response = RawInquiryPayload>,
    {
    }
    fn generic<T>()
    where
        T: Request<Class = request::Inquiry> + Inquiry<Response = Response>,
    {
    }
    fn typed<T>()
    where
        T: Request<Class = request::Inquiry>
            + Inquiry<Response = bool>
            + ResponseParser<Response = bool>,
    {
    }

    raw::<RenamedRawInquiry>();
    generic::<RenamedResponseInquiry>();
    typed::<RenamedTypedInquiry>();
}

/// A downstream static profile exercises the public profile contract without
/// importing any implementation-only camera/runtime types.
#[derive(Debug, Clone, Copy)]
struct DownstreamProfile;

type Base = visca_renamed::profiles::GenericVisca;

impl ProfileMetadata for DownstreamProfile {
    const MODEL_NAME: &'static str = "Downstream renamed profile";
    const DEFAULT_CAMERA_ID: u8 = 3;
    type Envelope = visca_renamed::transport::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(120);
    const COMMAND_TIMEOUTS: visca_renamed::CommandTimeouts = visca_renamed::CommandTimeouts::new(
        Duration::from_secs(6),
        Duration::from_secs(30),
        Duration::from_secs(60),
        Duration::from_secs(300),
        Duration::from_secs(6),
    );
}

impl PanTilt for DownstreamProfile {
    const PAN_RANGE: capabilities::CapabilityRange<i16> = <Base as PanTilt>::PAN_RANGE;
    const TILT_RANGE: capabilities::CapabilityRange<i16> = <Base as PanTilt>::TILT_RANGE;
    const MAX_PAN_SPEED: u8 = 9;
    const MAX_TILT_SPEED: u8 = 7;
    const PAN_DEGREES_TO_UNITS: f32 = 10.0;
    const TILT_DEGREES_TO_UNITS: f32 = 15.0;
}

impl Zoom for DownstreamProfile {
    const OPTICAL_ZOOM_MAX: u16 = <Base as Zoom>::OPTICAL_ZOOM_MAX;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: capabilities::CapabilityRange<u8> = <Base as Zoom>::ZOOM_SPEED_RANGE;
    const SUPPORTS_DIRECT_ZOOM: bool = false;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = <Base as Zoom>::ZOOM_MAGNIFICATION_TO_UNITS;
}

impl Focus for DownstreamProfile {
    const FOCUS_NEAR_LIMIT: u16 = <Base as Focus>::FOCUS_NEAR_LIMIT;
    const FOCUS_FAR_LIMIT: u16 = <Base as Focus>::FOCUS_FAR_LIMIT;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}

impl Exposure for DownstreamProfile {
    const EXPOSURE_MODES: &'static [visca_renamed::ExposureMode] =
        <Base as Exposure>::EXPOSURE_MODES;
    const IRIS_RANGE: Option<capabilities::CapabilityRange<u16>> = <Base as Exposure>::IRIS_RANGE;
    const SHUTTER_SPEEDS: &'static [capabilities::ShutterSpeed] =
        <Base as Exposure>::SHUTTER_SPEEDS;
    const GAIN_RANGE: capabilities::CapabilityRange<u8> = <Base as Exposure>::GAIN_RANGE;
    const BRIGHTNESS_RANGE: Option<capabilities::CapabilityRange<u16>> =
        <Base as Exposure>::BRIGHTNESS_RANGE;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
}

impl WhiteBalance for DownstreamProfile {
    const WB_MODES: &'static [visca_renamed::WhiteBalanceMode] = <Base as WhiteBalance>::WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = false;
    const RG_TUNING_RANGE: Option<capabilities::CapabilityRange<i8>> =
        <Base as WhiteBalance>::RG_TUNING_RANGE;
    const BG_TUNING_RANGE: Option<capabilities::CapabilityRange<i8>> =
        <Base as WhiteBalance>::BG_TUNING_RANGE;
}

impl ImageProcessing for DownstreamProfile {
    const CONTRAST_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::CONTRAST_RANGE;
    const SHARPNESS_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::SHARPNESS_RANGE;
    const SATURATION_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::SATURATION_RANGE;
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_HUE: bool = true;
    const HUE_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::HUE_RANGE;
}

impl Presets for DownstreamProfile {
    const MAX_PRESETS: u8 = 8;
    const PRESET_SPEED_RANGE: capabilities::CapabilityRange<u8> =
        <Base as Presets>::PRESET_SPEED_RANGE;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for DownstreamProfile {
    const POWER_ON_TIME: Duration = Duration::from_millis(4_250);
    const SUPPORTS_STANDBY: bool = true;
}

impl MenuCapability for DownstreamProfile {}
impl Tally for DownstreamProfile {}
impl MotionSyncMetadata for DownstreamProfile {}
impl NdFilterMetadata for DownstreamProfile {}
impl VariableSpeedMetadata for DownstreamProfile {}
impl ProfileTypedSupport for DownstreamProfile {
    const TYPED_SUPPORT: capabilities::TypedSupportSet = capabilities::TypedSupportSet::empty();
}

impl CompileTimeProfile for DownstreamProfile {
    const TRANSPORTS: TransportCompatibility = TransportCompatibility::new(Some(9876), None, false);
    const INQUIRY_TIMEOUT: Duration = Duration::from_millis(900);
    const CANCELLATION_TIMEOUT: Duration = Duration::from_millis(800);
    const AMBIGUITY_TIMEOUT: Duration = Duration::from_millis(700);
    const MAXIMUM_COMMAND_SOCKETS: u8 = 1;
    const PRESET_RECALL_AXES: Option<AffectedAxes> = Some(AffectedAxes::PAN_TILT);
    const POSITION_INQUIRIES: PositionInquirySupport =
        PositionInquirySupport::new(true, true, true);
}

fn main() -> visca_renamed::Result<()> {
    assert_inquiry_contracts();

    let _ = RenamedValue::new(2)?;
    let _ = RenamedEnum::try_from(1)?;

    let inquiry = RenamedRawInquiry;
    let mut buffer = [0; <RenamedRawInquiry as Request>::MAX_SIZE];
    assert_eq!(inquiry.write_into(CameraId::CAMERA_1, &mut buffer)?, 5);
    assert_eq!(
        inquiry.decoder().decode(&[0x12, 0x34])?.as_slice(),
        &[0x12, 0x34]
    );

    let profile = ProfileSpec::from_compile_time::<DownstreamProfile>()?;
    assert_eq!(
        profile.capabilities().model_name,
        "Downstream renamed profile"
    );
    assert_eq!(profile.transports().tcp_port(), Some(9876));
    Ok(())
}
