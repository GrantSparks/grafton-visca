use log::error;

use super::{ExposureMode, ViscaInquiryResponse, WhiteBalanceMode};
use crate::error::ViscaError;

#[derive(Debug)]
pub enum ViscaResponse {
    Ack,
    Completion,
    Error(ViscaError),
    InquiryResponse(ViscaInquiryResponse),
    Unknown(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscaResponseType {
    PanTiltPosition,
    ZoomPosition,
    FocusPosition,
    ExposureMode,
    WhiteBalanceMode,
    Luminance,
    Contrast,
    SharpnessMode,
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    ImageFlip,
    BlackWhiteMode,
    ExposureCompensationMode,
    ExposureCompensationPosition,
    Backlight,
    Iris,
    Shutter,
    GainLimit,
    AntiFlicker,
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
    MotionSyncMode,
    MotionSyncSpeed,
    FocusMode,
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

pub fn parse_visca_response(
    response: &[u8],
    response_type: &ViscaResponseType,
) -> Result<ViscaResponse, ViscaError> {
    if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
        return Err(ViscaError::InvalidResponseFormat);
    }

    match response[1] {
        0x40..=0x4F => Ok(ViscaResponse::Ack),
        0x50..=0x5F => {
            if response.len() == 3 {
                return Ok(ViscaResponse::Completion);
            }

            match response_type {
                ViscaResponseType::PanTiltPosition => {
                    if response.len() != 11 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let pan =
                        u32::from_be_bytes([response[2], response[3], response[4], response[5]]);
                    let tilt =
                        u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::PanTiltPosition { pan, tilt },
                    ))
                }
                ViscaResponseType::ZoomPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = u32::from_be_bytes([0, response[2], response[3], response[4]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ZoomPosition { position },
                    ))
                }
                ViscaResponseType::FocusPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = u32::from_be_bytes([0, response[2], response[3], response[4]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::FocusPosition { position },
                    ))
                }
                ViscaResponseType::ExposureMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let mode = ExposureMode::try_from(response[2])
                        .map_err(|_| ViscaError::UnexpectedResponseType)?;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ExposureMode { mode },
                    ))
                }
                ViscaResponseType::WhiteBalanceMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let mode = WhiteBalanceMode::try_from(response[2])
                        .map_err(|_| ViscaError::UnexpectedResponseType)?;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::WhiteBalance { mode },
                    ))
                }
                _ => Ok(ViscaResponse::Completion),
            }
        }
        0x60..=0x6F => Err(ViscaError::from_code(response[1])),
        _ => {
            error!("Unknown response: {:02X?}", response);
            Ok(ViscaResponse::Unknown(response.to_vec()))
        }
    }
}
