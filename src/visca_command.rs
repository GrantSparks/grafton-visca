use nom::{
    bytes::complete::tag,
    combinator::{map, map_res},
    number::complete::{be_u16, be_u32, u8},
    sequence::{preceded, tuple},
    IResult,
};

use crate::error::ViscaError;

#[derive(Debug, Copy, Clone)]
pub enum Power {
    On = 0x02,
    Standby = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub enum Flip {
    On = 0x02,
    Off = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}

#[derive(Debug, Copy, Clone)]
pub enum WhiteBalanceMode {
    Auto = 0x00,
    Indoor = 0x01,
    Outdoor = 0x02,
    OnePush = 0x03,
    Manual = 0x05,
    ColorTemperature = 0x20,
}

#[derive(Debug, Copy, Clone)]
pub enum FocusMode {
    Auto = 0x02,
    Manual = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub enum CommandStatus {
    Ack = 0x04,
    Completion = 0x05,
    SyntaxError = 0x60,
    CommandBufferFull = 0x61,
    CommandCanceled = 0x63,
    NoSocket = 0x65,
    CommandNotExecutable = 0x41,
}

impl CommandStatus {
    pub fn from_u8(value: u8) -> Option<CommandStatus> {
        match value {
            0x04 => Some(CommandStatus::Ack),
            0x05 => Some(CommandStatus::Completion),
            0x60 => Some(CommandStatus::SyntaxError),
            0x61 => Some(CommandStatus::CommandBufferFull),
            0x63 => Some(CommandStatus::CommandCanceled),
            0x65 => Some(CommandStatus::NoSocket),
            0x41 => Some(CommandStatus::CommandNotExecutable),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PanSpeed(u8);

impl PanSpeed {
    pub const STOP: PanSpeed = PanSpeed(0x00);
    pub const LOW_SPEED: PanSpeed = PanSpeed(0x01);
    pub const HIGH_SPEED: PanSpeed = PanSpeed(0x18);

    // Constructor to ensure the value is either STOP or within the range 0x01 (low speed) ~ 0x18 (high speed)
    pub fn new(value: u8) -> Result<Self, &'static str> {
        if Self::is_valid_value(value) {
            Ok(PanSpeed(value))
        } else {
            Err("Value must be in the range 0x00..=0x18")
        }
    }

    // Internal function to check if the value is valid
    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    // Getter method to access the value
    pub fn get_value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    pub const STOP: TiltSpeed = TiltSpeed(0x00);
    pub const LOW_SPEED: TiltSpeed = TiltSpeed(0x01);
    pub const HIGH_SPEED: TiltSpeed = TiltSpeed(0x14);

    // Constructor to ensure the value is either STOP or within the range 0x01 (low speed) ~ 0x14 (high speed)
    pub fn new(value: u8) -> Result<Self, &'static str> {
        if Self::is_valid_value(value) {
            Ok(TiltSpeed(value))
        } else {
            Err("Value must be in the range 0x00..=0x18")
        }
    }

    // Internal function to check if the value is valid
    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    // Getter method to access the value
    pub fn get_value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PanTiltDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Stop,
}

impl PanTiltDirection {
    pub fn to_bytes(self) -> (u8, u8) {
        match self {
            PanTiltDirection::Up => (0x03, 0x01),
            PanTiltDirection::Down => (0x03, 0x02),
            PanTiltDirection::Left => (0x01, 0x03),
            PanTiltDirection::Right => (0x02, 0x03),
            PanTiltDirection::UpLeft => (0x01, 0x01),
            PanTiltDirection::UpRight => (0x02, 0x01),
            PanTiltDirection::DownLeft => (0x01, 0x02),
            PanTiltDirection::DownRight => (0x02, 0x02),
            PanTiltDirection::Stop => (0x03, 0x03),
        }
    }
}

#[derive(Debug)]
pub enum ViscaCommand {
    // Image Commands
    LuminanceDirect(u8),
    ContrastDirect(u8),
    SharpnessMode(u8),
    SharpnessReset,
    SharpnessUp,
    SharpnessDown,
    SharpnessDirect(u8, u8),
    HorizontalFlip(Flip),
    VerticalFlip(Flip),
    ImageFlip(u8),
    BlackWhiteDirect(u8),

    // Exposure Commands
    ExposureMode(ExposureMode),
    ExposureCompensationOnOff(Flip),
    ExposureCompensationReset,
    ExposureCompensationUp,
    ExposureCompensationDown,
    ExposureCompensationDirect(u8, u8),
    DynamicRangeControlDirect(u8),
    BacklightOnOff(Flip),

    // Iris Commands
    IrisReset,
    IrisUp,
    IrisDown,
    IrisDirect(u8),

    // Shutter Commands
    ShutterReset,
    ShutterUp,
    ShutterDown,
    ShutterDirect(u8, u8),

    // Bright Commands
    BrightReset,
    BrightUp,
    BrightDown,
    BrightDirect(u8, u8),

    // Wide Dynamic Range Commands
    WideDynamicRangeReset,
    WideDynamicRangeUp,
    WideDynamicRangeDown,
    WideDynamicRangeDirect(u8, u8),

    // Gain Commands
    GainReset,
    GainUp,
    GainDown,
    GainDirect(u8, u8),
    GainLimitDirect(u8),

    // Anti-Flicker Commands
    AntiFlickerDirect(u8),

    // Color Commands
    WhiteBalanceMode(WhiteBalanceMode),
    OnePushTriggerAction,
    RedTuningDirect(u8, u8),
    BlueTuningDirect(u8, u8),
    SaturationDirect(u8),
    HueDirect(u8),

    // Pan Tilt Commands
    PanTiltDrive(PanTiltDirection, PanSpeed, TiltSpeed),
    PanTiltAbsolutePosition(u8, u8, u8, u8, u8, u8, u8, u8),
    PanTiltRelativePosition(u8, u8, u8, u8, u8, u8, u8, u8),
    PanTiltHome,
    PanTiltReset,
    PanTiltLimitSet(u8, u8, u8, u8, u8, u8, u8, u8),
    PanTiltLimitClear(u8, u8, u8, u8, u8, u8, u8, u8),

    // Zoom Commands
    ZoomStop,
    ZoomTeleStandard,
    ZoomWideStandard,
    ZoomTeleAdjustableSpeed(u8),
    ZoomWideAdjustableSpeed(u8),
    ZoomDirect(u8, u8, u8, u8),

    // Focus Commands
    FocusMode(FocusMode),
    FocusToggle,
    FocusStop,
    FocusFarStandardSpeed,
    FocusNearStandardSpeed,
    FocusFarAdjustableSpeed(u8),
    FocusNearAdjustableSpeed(u8),
    FocusDirect(u8, u8, u8, u8),
    FocusOnePushAutoFocus,
    FocusLock(u8),
    FocusZone(u8),
    FocusRange(u8, u8, u8),
    FocusRecalibrate,

    // Preset Commands
    PresetReset(u8),
    PresetSet(u8),
    PresetRecall(u8),
    PresetSpeedDirect(u8),

    // Motion Sync Commands
    MotionSyncOnOff(Flip),
    MotionSyncMaxSpeedLimit(u8),

    // Camera Menu Commands
    CameraMenuOpenClose,
    CameraMenuClose,
    CameraMenuNavigateUp,
    CameraMenuNavigateDown,
    CameraMenuNavigateLeft,
    CameraMenuNavigateRight,
    CameraMenuEnter,
    CameraMenuReturn,

    // System Commands
    SystemPower(Power),
    SystemIfClear,
    SystemVideoTemplate(u8),
    SystemLensType(u8),
    SystemMulticastOnOff(Flip),
    SystemRtmpStream(u8, Flip),
    SystemStandbyLight(u8),
    SystemUsbAudio(Flip),
    SystemPauseVideo(Flip),
    SystemSave,

    // Inquiry Commands
    InquiryLuminance,
    InquiryContrast,
    InquirySharpnessMode,
    InquirySharpnessPosition,
    InquiryHorizontalFlip,
    InquiryVerticalFlip,
    InquiryImageFlip,
    InquiryBlackWhiteMode,
    InquiryExposureCompensationMode,
    InquiryExposureCompensationPosition,
    InquiryBacklight,
    InquiryIris,
    InquiryShutter,
    InquiryGainLimit,
    InquiryAntiFlicker,
    InquiryWhiteBalanceMode,
    InquiryRedTuning,
    InquiryBlueTuning,
    InquirySaturation,
    InquiryHue,
    InquiryRedGain,
    InquiryBlueGain,
    InquiryColorTemperature,
    InquiryAutoWhiteBalanceSensitivity,
    InquiryThreeDNoiseReduction,
    InquiryTwoDNoiseReduction,
    InquiryPanTiltPosition,
    InquiryZoomPosition,
    InquiryMotionSyncSpeed,
    InquiryFocusPosition,
    InquiryFocusZone,
    InquiryAutoFocusSensitivity,
    InquiryFocusRange,
    InquiryMenuOpenClose,
    InquiryUsbAudio,
    InquiryRtmp,
    InquiryBlockLens,
    InquiryBlockColorExposure,
    InquiryBlockPowerImageEffect,
    InquiryBlockImage,
}

impl ViscaCommand {
    pub fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let bytes = match self {
            // Image Commands
            ViscaCommand::LuminanceDirect(p) => {
                vec![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, *p, 0xFF]
            }
            ViscaCommand::ContrastDirect(p) => {
                vec![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, *p, 0xFF]
            }
            ViscaCommand::SharpnessMode(p) => vec![0x81, 0x01, 0x04, 0x05, *p, 0xFF],
            ViscaCommand::SharpnessReset => vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF],
            ViscaCommand::SharpnessUp => vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF],
            ViscaCommand::SharpnessDown => vec![0x81, 0x01, 0x04, 0x02, 0x03, 0xFF],
            ViscaCommand::SharpnessDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x42, 0x00, 0x00, *p, *q, 0xFF]
            }
            ViscaCommand::HorizontalFlip(flip) => vec![0x81, 0x01, 0x04, 0x61, *flip as u8, 0xFF],
            ViscaCommand::VerticalFlip(flip) => vec![0x81, 0x01, 0x04, 0x66, *flip as u8, 0xFF],
            ViscaCommand::ImageFlip(p) => vec![0x81, 0x01, 0x04, 0xA4, *p, 0xFF],
            ViscaCommand::BlackWhiteDirect(p) => vec![0x81, 0x01, 0x04, 0x63, *p, 0xFF],

            // Exposure Commands
            ViscaCommand::ExposureMode(mode) => vec![0x81, 0x01, 0x04, 0x39, *mode as u8, 0xFF],
            ViscaCommand::ExposureCompensationOnOff(flip) => {
                vec![0x81, 0x01, 0x04, 0x3E, *flip as u8, 0xFF]
            }
            ViscaCommand::ExposureCompensationReset => vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF],
            ViscaCommand::ExposureCompensationUp => vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF],
            ViscaCommand::ExposureCompensationDown => vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF],
            ViscaCommand::ExposureCompensationDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, *p, *q, 0xFF]
            }
            ViscaCommand::DynamicRangeControlDirect(p) => {
                vec![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, *p, 0xFF]
            }
            ViscaCommand::BacklightOnOff(flip) => vec![0x81, 0x01, 0x04, 0x33, *flip as u8, 0xFF],

            // Iris Commands
            ViscaCommand::IrisReset => vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            ViscaCommand::IrisUp => vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            ViscaCommand::IrisDown => vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF],
            ViscaCommand::IrisDirect(p) => vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, *p, 0xFF],

            // Shutter Commands
            ViscaCommand::ShutterReset => vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            ViscaCommand::ShutterUp => vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF],
            ViscaCommand::ShutterDown => vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF],
            ViscaCommand::ShutterDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, *p, *q, 0xFF]
            }

            // Bright Commands
            ViscaCommand::BrightReset => vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF],
            ViscaCommand::BrightUp => vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF],
            ViscaCommand::BrightDown => vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF],
            ViscaCommand::BrightDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, *p, *q, 0xFF]
            }

            // Wide Dynamic Range Commands
            ViscaCommand::WideDynamicRangeReset => vec![0x81, 0x01, 0x04, 0x21, 0x00, 0xFF],
            ViscaCommand::WideDynamicRangeUp => vec![0x81, 0x01, 0x04, 0x21, 0x02, 0xFF],
            ViscaCommand::WideDynamicRangeDown => vec![0x81, 0x01, 0x04, 0x21, 0x03, 0xFF],
            ViscaCommand::WideDynamicRangeDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x51, 0x00, 0x00, *p, *q, 0xFF]
            }

            // Gain Commands
            ViscaCommand::GainReset => vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF],
            ViscaCommand::GainUp => vec![0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF],
            ViscaCommand::GainDown => vec![0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF],
            ViscaCommand::GainDirect(p, q) => {
                vec![0x81, 0x01, 0x04, 0x4C, 0x00, 0x00, *p, *q, 0xFF]
            }
            ViscaCommand::GainLimitDirect(p) => vec![0x81, 0x01, 0x04, 0x2C, *p, 0xFF],

            // Anti-Flicker Commands
            ViscaCommand::AntiFlickerDirect(p) => vec![0x81, 0x01, 0x04, 0x23, *p, 0xFF],

            // Color Commands
            ViscaCommand::WhiteBalanceMode(mode) => vec![0x81, 0x01, 0x04, 0x35, *mode as u8, 0xFF],
            ViscaCommand::OnePushTriggerAction => vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF],
            ViscaCommand::RedTuningDirect(p, q) => vec![0x81, 0x0A, 0x01, 0x12, *p, *q, 0xFF],
            ViscaCommand::BlueTuningDirect(p, q) => vec![0x81, 0x0A, 0x01, 0x13, *p, *q, 0xFF],
            ViscaCommand::SaturationDirect(p) => {
                vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, *p, 0xFF]
            }
            ViscaCommand::HueDirect(p) => vec![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, *p, 0xFF],

            // Pan Tilt Commands
            ViscaCommand::PanTiltDrive(dir, pan_speed, tilt_speed) => {
                let (dir_byte1, dir_byte2) = dir.to_bytes();
                vec![
                    0x81,
                    0x01,
                    0x06,
                    0x01,
                    pan_speed.get_value(),
                    tilt_speed.get_value(),
                    dir_byte1,
                    dir_byte2,
                    0xFF,
                ]
            }
            ViscaCommand::PanTiltAbsolutePosition(
                pan_speed,
                tilt_speed,
                y1,
                y2,
                y3,
                y4,
                z1,
                z2,
            ) => vec![
                0x81,
                0x01,
                0x06,
                0x02,
                *pan_speed,
                *tilt_speed,
                *y1,
                *y2,
                *y3,
                *y4,
                *z1,
                *z2,
                0xFF,
            ],
            ViscaCommand::PanTiltRelativePosition(
                pan_speed,
                tilt_speed,
                y1,
                y2,
                y3,
                y4,
                z1,
                z2,
            ) => vec![
                0x81,
                0x01,
                0x06,
                0x03,
                *pan_speed,
                *tilt_speed,
                *y1,
                *y2,
                *y3,
                *y4,
                *z1,
                *z2,
                0xFF,
            ],
            ViscaCommand::PanTiltHome => vec![0x81, 0x01, 0x06, 0x04, 0xFF],
            ViscaCommand::PanTiltReset => vec![0x81, 0x01, 0x06, 0x05, 0xFF],
            ViscaCommand::PanTiltLimitSet(w, y1, y2, y3, y4, z1, z2, z3) => vec![
                0x81, 0x01, 0x06, 0x07, 0x00, *w, *y1, *y2, *y3, *y4, *z1, *z2, *z3, 0xFF,
            ],
            ViscaCommand::PanTiltLimitClear(w, y1, y2, y3, y4, z1, z2, z3) => vec![
                0x81, 0x01, 0x06, 0x07, 0x01, *w, *y1, *y2, *y3, *y4, *z1, *z2, *z3, 0xFF,
            ],

            // Zoom Commands
            ViscaCommand::ZoomStop => vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF],
            ViscaCommand::ZoomTeleStandard => vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF],
            ViscaCommand::ZoomWideStandard => vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF],
            ViscaCommand::ZoomTeleAdjustableSpeed(p) => {
                vec![0x81, 0x01, 0x04, 0x07, 0x20 + *p, 0xFF]
            }
            ViscaCommand::ZoomWideAdjustableSpeed(p) => {
                vec![0x81, 0x01, 0x04, 0x07, 0x30 + *p, 0xFF]
            }
            ViscaCommand::ZoomDirect(p, q, r, s) => {
                vec![0x81, 0x01, 0x04, 0x47, *p, *q, *r, *s, 0xFF]
            }

            // Focus Commands
            ViscaCommand::FocusMode(mode) => vec![0x81, 0x01, 0x04, 0x38, *mode as u8, 0xFF],
            ViscaCommand::FocusToggle => vec![0x81, 0x01, 0x04, 0x38, 0x10, 0xFF],
            ViscaCommand::FocusStop => vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF],
            ViscaCommand::FocusFarStandardSpeed => vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF],
            ViscaCommand::FocusNearStandardSpeed => vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF],
            ViscaCommand::FocusFarAdjustableSpeed(p) => {
                vec![0x81, 0x01, 0x04, 0x08, 0x20 + *p, 0xFF]
            }
            ViscaCommand::FocusNearAdjustableSpeed(p) => {
                vec![0x81, 0x01, 0x04, 0x08, 0x30 + *p, 0xFF]
            }
            ViscaCommand::FocusDirect(p, q, r, s) => {
                vec![0x81, 0x01, 0x04, 0x48, *p, *q, *r, *s, 0xFF]
            }
            ViscaCommand::FocusOnePushAutoFocus => vec![0x81, 0x01, 0x04, 0x38, 0x04, 0xFF],
            ViscaCommand::FocusLock(p) => vec![0x81, 0x0A, 0x04, 0x68, *p, 0xFF],
            ViscaCommand::FocusZone(p) => vec![0x81, 0x01, 0x04, 0xAA, *p, 0xFF],
            ViscaCommand::FocusRange(p, rs, tv) => vec![0x81, 0x0A, 0x11, 0x42, *p, *rs, *tv, 0xFF],
            ViscaCommand::FocusRecalibrate => vec![0x81, 0x0A, 0x01, 0x03, 0x12, 0xFF],

            // Preset Commands
            ViscaCommand::PresetReset(pp) => vec![0x81, 0x01, 0x04, 0x3F, 0x00, *pp, 0xFF],
            ViscaCommand::PresetSet(pp) => vec![0x81, 0x01, 0x04, 0x3F, 0x01, *pp, 0xFF],
            ViscaCommand::PresetRecall(pp) => vec![0x81, 0x01, 0x04, 0x3F, 0x02, *pp, 0xFF],
            ViscaCommand::PresetSpeedDirect(pp) => vec![0x81, 0x01, 0x06, 0x01, *pp, 0xFF],

            // Motion Sync Commands
            ViscaCommand::MotionSyncOnOff(flip) => vec![0x81, 0x0A, 0x11, 0x13, *flip as u8, 0xFF],
            ViscaCommand::MotionSyncMaxSpeedLimit(p) => vec![0x81, 0x0A, 0x11, 0x14, *p, 0xFF],

            // Camera Menu Commands
            ViscaCommand::CameraMenuOpenClose => vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x5F, 0xFF],
            ViscaCommand::CameraMenuClose => vec![0x81, 0x01, 0x06, 0x06, 0x03, 0xFF],
            ViscaCommand::CameraMenuNavigateUp => {
                vec![0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x03, 0x01, 0xFF]
            }
            ViscaCommand::CameraMenuNavigateDown => {
                vec![0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x03, 0x02, 0xFF]
            }
            ViscaCommand::CameraMenuNavigateLeft => {
                vec![0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x01, 0x03, 0xFF]
            }
            ViscaCommand::CameraMenuNavigateRight => {
                vec![0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x02, 0x03, 0xFF]
            }
            ViscaCommand::CameraMenuEnter => vec![0x81, 0x01, 0x06, 0x06, 0x05, 0xFF],
            ViscaCommand::CameraMenuReturn => vec![0x81, 0x01, 0x06, 0x06, 0x04, 0xFF],

            // System Commands
            ViscaCommand::SystemPower(power) => vec![0x81, 0x01, 0x04, 0x00, *power as u8, 0xFF],
            ViscaCommand::SystemIfClear => vec![0x81, 0x01, 0x00, 0x01, 0xFF],
            ViscaCommand::SystemVideoTemplate(p) => vec![0x81, 0x0B, 0x01, 0x0D, *p, 0xFF],
            ViscaCommand::SystemLensType(p) => vec![0x81, 0x0A, 0x01, 0x04, 0x1B, *p, 0xFF],
            ViscaCommand::SystemMulticastOnOff(flip) => {
                vec![0x81, 0x0B, 0x01, 0x23, *flip as u8, 0xFF]
            }
            ViscaCommand::SystemRtmpStream(s, flip) => {
                vec![0x81, 0x0A, 0x11, 0xA8, *s, *flip as u8, 0xFF]
            }
            ViscaCommand::SystemStandbyLight(p) => vec![0x81, 0x0A, 0x02, 0x02, *p, 0xFF],
            ViscaCommand::SystemUsbAudio(flip) => {
                vec![0x81, 0x2A, 0x02, 0xA0, 0x04, *flip as u8, 0xFF]
            }
            ViscaCommand::SystemPauseVideo(flip) => vec![0x81, 0x01, 0x04, 0x62, *flip as u8, 0xFF],
            ViscaCommand::SystemSave => vec![0x81, 0x01, 0x04, 0xA5, 0x10, 0xFF],

            // Inquiry Commands
            ViscaCommand::InquiryLuminance => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            ViscaCommand::InquiryContrast => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            ViscaCommand::InquirySharpnessMode => vec![0x81, 0x09, 0x04, 0x49, 0xFF],
            ViscaCommand::InquirySharpnessPosition => vec![0x81, 0x09, 0x04, 0x4A, 0xFF],
            ViscaCommand::InquiryHorizontalFlip => vec![0x81, 0x09, 0x04, 0x66, 0xFF],
            ViscaCommand::InquiryVerticalFlip => vec![0x81, 0x09, 0x04, 0x67, 0xFF],
            ViscaCommand::InquiryImageFlip => vec![0x81, 0x09, 0x04, 0x68, 0xFF],
            ViscaCommand::InquiryBlackWhiteMode => vec![0x81, 0x09, 0x04, 0x53, 0xFF],
            ViscaCommand::InquiryExposureCompensationMode => vec![0x81, 0x09, 0x04, 0x3E, 0xFF],
            ViscaCommand::InquiryExposureCompensationPosition => vec![0x81, 0x09, 0x04, 0x4E, 0xFF],
            ViscaCommand::InquiryBacklight => vec![0x81, 0x09, 0x04, 0x33, 0xFF],
            ViscaCommand::InquiryIris => vec![0x81, 0x09, 0x04, 0x4B, 0xFF],
            ViscaCommand::InquiryShutter => vec![0x81, 0x09, 0x04, 0x4A, 0xFF],
            ViscaCommand::InquiryGainLimit => vec![0x81, 0x09, 0x04, 0x2C, 0xFF],
            ViscaCommand::InquiryAntiFlicker => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            ViscaCommand::InquiryWhiteBalanceMode => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            ViscaCommand::InquiryRedTuning => vec![0x81, 0x09, 0x04, 0x42, 0xFF],
            ViscaCommand::InquiryBlueTuning => vec![0x81, 0x09, 0x04, 0x43, 0xFF],
            ViscaCommand::InquirySaturation => vec![0x81, 0x09, 0x04, 0x49, 0xFF],
            ViscaCommand::InquiryHue => vec![0x81, 0x09, 0x04, 0x4F, 0xFF],
            ViscaCommand::InquiryRedGain => vec![0x81, 0x09, 0x04, 0x44, 0xFF],
            ViscaCommand::InquiryBlueGain => vec![0x81, 0x09, 0x04, 0x45, 0xFF],
            ViscaCommand::InquiryColorTemperature => vec![0x81, 0x09, 0x04, 0x43, 0xFF],
            ViscaCommand::InquiryAutoWhiteBalanceSensitivity => {
                vec![0x81, 0x09, 0x04, 0x38, 0xFF]
            }
            ViscaCommand::InquiryThreeDNoiseReduction => vec![0x81, 0x09, 0x04, 0x52, 0xFF],
            ViscaCommand::InquiryTwoDNoiseReduction => vec![0x81, 0x09, 0x04, 0x53, 0xFF],
            ViscaCommand::InquiryPanTiltPosition => vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            ViscaCommand::InquiryZoomPosition => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            ViscaCommand::InquiryMotionSyncSpeed => vec![0x81, 0x09, 0x04, 0x38, 0xFF],
            ViscaCommand::InquiryFocusPosition => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            ViscaCommand::InquiryFocusZone => vec![0x81, 0x09, 0x04, 0x4B, 0xFF],
            ViscaCommand::InquiryAutoFocusSensitivity => {
                vec![0x81, 0x09, 0x04, 0x38, 0xFF]
            }
            ViscaCommand::InquiryFocusRange => vec![0x81, 0x09, 0x04, 0x4C, 0xFF],
            ViscaCommand::InquiryMenuOpenClose => vec![0x81, 0x09, 0x04, 0x62, 0xFF],
            ViscaCommand::InquiryUsbAudio => vec![0x81, 0x09, 0x04, 0x61, 0xFF],
            ViscaCommand::InquiryRtmp => vec![0x81, 0x09, 0x11, 0x53, 0xFF],
            ViscaCommand::InquiryBlockLens => vec![0x81, 0x09, 0x7E, 0x7E, 0x00, 0xFF],
            ViscaCommand::InquiryBlockColorExposure => vec![0x81, 0x09, 0x7E, 0x7E, 0x01, 0xFF],
            ViscaCommand::InquiryBlockPowerImageEffect => {
                vec![0x81, 0x09, 0x7E, 0x7E, 0x02, 0xFF]
            }
            ViscaCommand::InquiryBlockImage => vec![0x81, 0x09, 0x7E, 0x7E, 0x03, 0xFF],
        };

        Ok(bytes)
    }

    pub fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            ViscaCommand::InquiryLuminance => Some(ViscaResponseType::Luminance),
            ViscaCommand::InquiryContrast => Some(ViscaResponseType::Contrast),
            ViscaCommand::InquirySharpnessMode => Some(ViscaResponseType::SharpnessMode),
            ViscaCommand::InquirySharpnessPosition => Some(ViscaResponseType::SharpnessPosition),
            ViscaCommand::InquiryHorizontalFlip => Some(ViscaResponseType::HorizontalFlip),
            ViscaCommand::InquiryVerticalFlip => Some(ViscaResponseType::VerticalFlip),
            ViscaCommand::InquiryImageFlip => Some(ViscaResponseType::ImageFlip),
            ViscaCommand::InquiryBlackWhiteMode => Some(ViscaResponseType::BlackWhiteMode),
            ViscaCommand::InquiryExposureCompensationMode => {
                Some(ViscaResponseType::ExposureCompensationMode)
            }
            ViscaCommand::InquiryExposureCompensationPosition => {
                Some(ViscaResponseType::ExposureCompensationPosition)
            }
            ViscaCommand::InquiryBacklight => Some(ViscaResponseType::Backlight),
            ViscaCommand::InquiryIris => Some(ViscaResponseType::Iris),
            ViscaCommand::InquiryShutter => Some(ViscaResponseType::Shutter),
            ViscaCommand::InquiryGainLimit => Some(ViscaResponseType::GainLimit),
            ViscaCommand::InquiryAntiFlicker => Some(ViscaResponseType::AntiFlicker),
            ViscaCommand::InquiryWhiteBalanceMode => Some(ViscaResponseType::WhiteBalanceMode),
            ViscaCommand::InquiryRedTuning => Some(ViscaResponseType::RedTuning),
            ViscaCommand::InquiryBlueTuning => Some(ViscaResponseType::BlueTuning),
            ViscaCommand::InquirySaturation => Some(ViscaResponseType::Saturation),
            ViscaCommand::InquiryHue => Some(ViscaResponseType::Hue),
            ViscaCommand::InquiryRedGain => Some(ViscaResponseType::RedGain),
            ViscaCommand::InquiryBlueGain => Some(ViscaResponseType::BlueGain),
            ViscaCommand::InquiryColorTemperature => Some(ViscaResponseType::ColorTemperature),
            ViscaCommand::InquiryAutoWhiteBalanceSensitivity => {
                Some(ViscaResponseType::AutoWhiteBalanceSensitivity)
            }
            ViscaCommand::InquiryThreeDNoiseReduction => {
                Some(ViscaResponseType::ThreeDNoiseReduction)
            }
            ViscaCommand::InquiryTwoDNoiseReduction => Some(ViscaResponseType::TwoDNoiseReduction),
            ViscaCommand::InquiryPanTiltPosition => Some(ViscaResponseType::PanTiltPosition),
            ViscaCommand::InquiryZoomPosition => Some(ViscaResponseType::ZoomPosition),
            ViscaCommand::InquiryMotionSyncSpeed => Some(ViscaResponseType::MotionSyncSpeed),
            ViscaCommand::InquiryFocusPosition => Some(ViscaResponseType::FocusPosition),
            ViscaCommand::InquiryFocusZone => Some(ViscaResponseType::FocusZone),
            ViscaCommand::InquiryAutoFocusSensitivity => {
                Some(ViscaResponseType::AutoFocusSensitivity)
            }
            ViscaCommand::InquiryFocusRange => Some(ViscaResponseType::FocusRange),
            ViscaCommand::InquiryMenuOpenClose => Some(ViscaResponseType::MenuOpenClose),
            ViscaCommand::InquiryUsbAudio => Some(ViscaResponseType::UsbAudio),
            ViscaCommand::InquiryRtmp => Some(ViscaResponseType::Rtmp),
            ViscaCommand::InquiryBlockLens => Some(ViscaResponseType::BlockLens),
            ViscaCommand::InquiryBlockColorExposure => Some(ViscaResponseType::BlockColorExposure),
            ViscaCommand::InquiryBlockPowerImageEffect => {
                Some(ViscaResponseType::BlockPowerImageEffect)
            }
            ViscaCommand::InquiryBlockImage => Some(ViscaResponseType::BlockImage),
            ViscaCommand::ZoomWideStandard => Some(ViscaResponseType::ZoomWideStandard),
            ViscaCommand::ZoomTeleStandard => Some(ViscaResponseType::ZoomTeleStandard),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscaResponseType {
    Luminance,
    Contrast,
    SharpnessMode,
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    ImageFlip,
    BlackWhiteMode,
    ExposureMode,
    ExposureCompensationMode,
    ExposureCompensationPosition,
    Backlight,
    Iris,
    Shutter,
    GainLimit,
    AntiFlicker,
    WhiteBalanceMode,
    RedTuning,
    BlueTuning,
    Saturation,
    Hue,
    RedGain,
    BlueGain,
    ColorTemperature,
    AutoWhiteBalanceSensitivity,
    ThreeDNoiseReduction,
    TwoDNoiseReduction,
    PanTiltPosition,
    ZoomPosition,
    MotionSyncMode,
    MotionSyncSpeed,
    FocusMode,
    FocusPosition,
    FocusZone,
    AutoFocusSensitivity,
    FocusRange,
    MenuOpenClose,
    UsbAudio,
    Rtmp,
    BlockLens,
    BlockColorExposure,
    BlockPowerImageEffect,
    BlockImage,
    ZoomWideStandard,
    ZoomTeleStandard,
}

// Define ViscaInquiryResponse to capture various inquiry responses.
#[derive(Debug)]
pub enum ViscaInquiryResponse {
    PanTiltPosition { pan: u32, tilt: u32 },
    Luminance(u8),
    Contrast(u8),
    ZoomPosition { position: u32 },
    FocusPosition { position: u32 },
    Gain { gain: u8 },
    WhiteBalance { mode: u8 },
    ExposureCompensation { value: u8 },
    Backlight { status: bool },
    ColorTemperature { temperature: u16 },
    Hue { hue: u8 },
    // Add other specific inquiry responses as needed.
}

#[derive(Debug)]
pub enum ViscaResponse {
    Ack,
    Completion,
    Error(ViscaError),
    InquiryResponse(ViscaInquiryResponse),
    Unknown(Vec<u8>),
}

impl ViscaResponse {
    pub fn from_bytes(
        response: &[u8],
        response_type: &ViscaResponseType,
    ) -> Result<Self, ViscaError> {
        // Define parsers
        fn parse_header(input: &[u8]) -> IResult<&[u8], ()> {
            map(tag(&[0x90]), |_| ())(input)
        }

        fn parse_footer(input: &[u8]) -> IResult<&[u8], ()> {
            map(tag(&[0xFF]), |_| ())(input)
        }

        fn parse_short_message(input: &[u8]) -> IResult<&[u8], ViscaResponse> {
            map_res(
                tuple((parse_header, u8, parse_footer)),
                |(_, code, _)| match code {
                    0x40..=0x4F => Ok(ViscaResponse::Ack),
                    0x50..=0x5F => Ok(ViscaResponse::Completion),
                    0x60..=0x6F => Err(ViscaError::from_code(code)),
                    _ => Err(ViscaError::Unknown(code)),
                },
            )(input)
        }

        fn parse_inquiry_response<'a>(
            input: &'a [u8],
            response_type: &'a ViscaResponseType,
        ) -> IResult<&'a [u8], ViscaResponse> {
            let (input, _) = parse_header(input)?;
            let (input, _) = tag(&[0x50])(input)?;
            let (input, result) = match response_type {
                ViscaResponseType::PanTiltPosition => {
                    let (input, (pan, tilt)) = tuple((be_u32, be_u32))(input)?;
                    (input, ViscaInquiryResponse::PanTiltPosition { pan, tilt })
                }
                ViscaResponseType::ZoomPosition | ViscaResponseType::FocusPosition => {
                    let (input, position) = be_u32(input)?;
                    (
                        input,
                        match response_type {
                            ViscaResponseType::ZoomPosition => {
                                ViscaInquiryResponse::ZoomPosition { position }
                            }
                            ViscaResponseType::FocusPosition => {
                                ViscaInquiryResponse::FocusPosition { position }
                            }
                            _ => unreachable!(),
                        },
                    )
                }
                ViscaResponseType::Luminance | ViscaResponseType::Contrast => {
                    let (input, value) = preceded(tag(&[0x00, 0x00, 0x00]), u8)(input)?;
                    (
                        input,
                        match response_type {
                            ViscaResponseType::Luminance => ViscaInquiryResponse::Luminance(value),
                            ViscaResponseType::Contrast => ViscaInquiryResponse::Contrast(value),
                            _ => unreachable!(),
                        },
                    )
                }
                ViscaResponseType::GainLimit
                | ViscaResponseType::WhiteBalanceMode
                | ViscaResponseType::ExposureCompensationMode
                | ViscaResponseType::Backlight
                | ViscaResponseType::Hue => {
                    let (input, value) = preceded(tag(&[0x00, 0x00]), u8)(input)?;
                    (
                        input,
                        match response_type {
                            ViscaResponseType::GainLimit => {
                                ViscaInquiryResponse::Gain { gain: value }
                            }
                            ViscaResponseType::WhiteBalanceMode => {
                                ViscaInquiryResponse::WhiteBalance { mode: value }
                            }
                            ViscaResponseType::ExposureCompensationMode => {
                                ViscaInquiryResponse::ExposureCompensation { value }
                            }
                            ViscaResponseType::Backlight => {
                                ViscaInquiryResponse::Backlight { status: value != 0 }
                            }
                            ViscaResponseType::Hue => ViscaInquiryResponse::Hue { hue: value },
                            _ => unreachable!(),
                        },
                    )
                }
                ViscaResponseType::ColorTemperature => {
                    let (input, temperature) = preceded(tag(&[0x00]), be_u16)(input)?;
                    (
                        input,
                        ViscaInquiryResponse::ColorTemperature { temperature },
                    )
                }
                _ => {
                    return Err(nom::Err::Failure(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Switch,
                    )))
                }
            };
            let (input, _) = parse_footer(input)?;
            Ok((input, ViscaResponse::InquiryResponse(result)))
        }

        // Try parsing as a short message first, then as an inquiry response
        match parse_short_message(response) {
            Ok((_, result)) => Ok(result),
            Err(_) => match parse_inquiry_response(response, response_type) {
                Ok((_, result)) => Ok(result),
                Err(_) => Err(ViscaError::InvalidResponseFormat),
            },
        }
    }
}
