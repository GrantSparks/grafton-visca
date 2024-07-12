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
    pub fn to_byte(self) -> u8 {
        match self {
            PanTiltDirection::Up => 0x03,
            PanTiltDirection::Down => 0x04,
            PanTiltDirection::Left => 0x01,
            PanTiltDirection::Right => 0x02,
            PanTiltDirection::UpLeft => 0x01,
            PanTiltDirection::UpRight => 0x02,
            PanTiltDirection::DownLeft => 0x01,
            PanTiltDirection::DownRight => 0x02,
            PanTiltDirection::Stop => 0x03,
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
    PanTiltDrive(PanTiltDirection, u8, u8),
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
}

impl ViscaCommand {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
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
                let direction_byte = dir.to_byte();
                vec![
                    0x81,
                    0x01,
                    0x06,
                    0x01,
                    *pan_speed,
                    *tilt_speed,
                    direction_byte,
                    0x03,
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
        }
    }
}
