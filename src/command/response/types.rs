//! Core response types for VISCA commands.

use std::borrow::Cow;

use crate::{command::InquiryResponse, error::Error, ViscaSocket};

/// ViscaResponse from a VISCA command.
///
/// Represents all possible responses from the camera including acknowledgments,
/// completions, errors, and inquiry data.
#[derive(Debug)]
pub enum ViscaResponse {
    /// Acknowledgment that the command was received and is being processed
    CmdAck {
        /// Socket that acknowledged, if available
        socket: Option<ViscaSocket>,
    },
    /// Command completed successfully (no data returned)
    Completion {
        /// Socket that completed, if available
        socket: Option<ViscaSocket>,
    },
    /// Command failed with an error
    Error(Error),
    /// Inquiry command response containing requested data
    Inquiry(InquiryResponse),
    /// Unknown response format with type information and raw data
    Unknown {
        /// The response type that could not be parsed
        response_type: Option<ViscaResponseType>,
        /// Raw response data for debugging
        data: Vec<u8>,
    },
}

impl ViscaResponse {
    /// Convert response to a Result, treating Completion as Ok and Error as Err.
    ///
    /// Note: ACK responses are treated as an error because they only indicate
    /// the command was queued, not completed. Callers should wait for the
    /// subsequent Completion response.
    pub fn into_result(self) -> Result<(), Error> {
        match self {
            ViscaResponse::Completion { .. } => Ok(()),
            ViscaResponse::CmdAck { .. } => Err(Error::CommandPending), // ACK means command is queued, not completed
            ViscaResponse::Error(e) => Err(e),
            ViscaResponse::Inquiry(_) => Ok(()), // Inquiry responses are success
            ViscaResponse::Unknown { data, .. } => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Known response type"),
                actual: data,
            }),
        }
    }
}

/// Type of expected response for inquiry commands.
///
/// Used to indicate what kind of data parser should expect in the response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViscaResponseType {
    /// Power state inquiry response (On/Off).
    Power,
    /// Pan and tilt position inquiry response.
    PanTiltPosition,
    /// Zoom position inquiry response.
    ZoomPosition,
    /// Focus position inquiry response.
    FocusPosition,
    /// Focus near limit inquiry response.
    FocusNearLimit,
    /// Focus zone inquiry response.
    FocusZone,
    /// Auto focus sensitivity inquiry response.
    AutoFocusSensitivity,
    /// Exposure mode inquiry response.
    ExposureMode,
    /// Exposure compensation mode inquiry response.
    ExposureCompensationMode,
    /// Exposure compensation value inquiry response.
    ExposureCompensation,
    /// Iris position inquiry response.
    Iris,
    /// Shutter speed inquiry response.
    Shutter,
    /// Brightness inquiry response.
    Bright,
    /// Gain inquiry response.
    Gain,
    /// Gain limit inquiry response.
    GainLimit,
    /// Backlight compensation inquiry response.
    Backlight,
    /// Dynamic range inquiry response.
    DynamicRange,
    /// White balance mode inquiry response.
    WhiteBalanceMode,
    /// Color temperature inquiry response.
    ColorTemperature,
    /// Red gain inquiry response.
    RedChannel,
    /// Blue gain inquiry response.
    BlueChannel,
    /// Luminance inquiry response.
    Luminance,
    /// Contrast inquiry response.
    Contrast,
    /// Sharpness value inquiry response.
    Sharpness,
    /// Sharpness mode inquiry response.
    SharpnessMode,
    /// Saturation inquiry response.
    Saturation,
    /// Hue inquiry response.
    Hue,
    /// 2D noise reduction inquiry response.
    NoiseReduction2D,
    /// 3D noise reduction inquiry response.
    NoiseReduction3D,
    /// Image flip inquiry response.
    ImageFlip,
    /// Black and white mode inquiry response.
    BlackWhite,
    /// Picture effect inquiry response.
    PictureEffect,
    /// System version inquiry response.
    Version,
    /// Red tally light state inquiry response.
    TallyRed,
    /// Green tally light state inquiry response.
    TallyGreen,

    // Additional response types for completeness
    /// Sharpness position inquiry response.
    SharpnessPosition,
    /// Black and white mode state inquiry response.
    BlackWhiteMode,
    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition,
    /// Red tuning level inquiry response.
    RedTuning,
    /// Blue tuning level inquiry response.
    BlueTuning,
    /// Gamma curve setting inquiry response.
    Gamma,
    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity,
    /// Motion sync mode inquiry response.
    MotionSyncMode,
    /// Motion sync speed inquiry response.
    MotionSyncPreset,
    /// Focus mode inquiry response.
    FocusMode,
    /// Focus range inquiry response.
    FocusRange,
    /// Menu open/close state inquiry response.
    MenuOpenClose,
    /// USB audio state inquiry response.
    UsbAudio,
    /// RTMP state inquiry response.
    Rtmp,
    /// Auto focus state inquiry response.
    AutoFocus,
    /// Focus unlock state inquiry response.
    FocusUnlock,
    /// Zoom out state inquiry response.
    ZoomOut,
    /// Zoom in state inquiry response.
    ZoomIn,
    /// Iris up state inquiry response.
    IrisUp,
    /// Iris down state inquiry response.
    IrisDown,
    /// Night/day mode inquiry response.
    NightDayMode,
    /// Night/day position inquiry response.
    NightDayPosition,
    /// Auto trace state inquiry response.
    AutoTrace,
    /// Two tone mode inquiry response.
    TwoToneMode,
    /// Defog mode inquiry response.
    DefogMode,
    /// Noise reduction level inquiry response.
    NrLevel,
    /// Noise reduction mode inquiry response.
    NrMode,
    /// Noise reduction speed inquiry response.
    NrSpeed,
    /// Broadcast domain inquiry response.
    BroadcastDomain,
    /// Resolution inquiry response.
    Resolution,
    /// ND filter state inquiry response.
    NdFilter,
    /// ND filter preset inquiry response.
    NdFilterPreset,
    /// Focus near/far state inquiry response.
    FocusNearFar,
    /// Zoom tele/wide state inquiry response.
    ZoomTeleWide,
    /// Standby state inquiry response.
    Standby,
    /// Digital Ptz state inquiry response.
    DigitalPtz,
    /// Digital mode inquiry response.
    Digital,
    /// Iris control inquiry response.
    IrisControl,
    /// Defog level inquiry response.
    DefogLevel,
    /// Night/day switch inquiry response.
    NightDaySwitch,
    /// Flip mode inquiry response.
    FlipMode,
    /// Tally status inquiry response.
    TallyStatus,
    /// Tally auto adjust inquiry response.
    TallyAutoAdjust,
}
