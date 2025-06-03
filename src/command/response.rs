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
    Sharpness,
    SharpnessMode,
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    ImageFlip,
    BlackWhiteMode,
    ExposureCompensation,
    ExposureCompensationMode,
    ExposureCompensationPosition,
    Backlight,
    Iris,
    Shutter,
    Bright,
    Gain,
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
    NoiseReduction2D,
    NoiseReduction3D,
    BlackWhite,
    AFSensitivity,
    FocusNearLimit,
    DynamicRange,
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

                    let mut pan = (response[2] as i16) << 12;
                    pan |= (response[3] as i16) << 8;
                    pan |= (response[4] as i16) << 4;
                    pan |= response[5] as i16;

                    let mut tilt = (response[6] as i16) << 12;
                    tilt |= (response[7] as i16) << 8;
                    tilt |= (response[8] as i16) << 4;
                    tilt |= response[9] as i16;

                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::PanTiltPosition { pan, tilt },
                    ))
                }
                ViscaResponseType::ZoomPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }

                    let mut position = (response[2] as u16) << 12;
                    position |= (response[3] as u16) << 8;
                    position |= (response[4] as u16) << 4;
                    position |= response[5] as u16;

                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ZoomPosition { position },
                    ))
                }
                ViscaResponseType::FocusPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }

                    let mut position = (response[2] as u16) << 12;
                    position |= (response[3] as u16) << 8;
                    position |= (response[4] as u16) << 4;
                    position |= response[5] as u16;

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
                // New inquiry response parsers
                ViscaResponseType::Sharpness => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let value = (response[4] << 4) | response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Sharpness { value },
                    ))
                }
                ViscaResponseType::ExposureCompensation => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let raw_value = response[5];
                    let value = (raw_value as i8) - 7; // Convert 0x0..0xE to -7..+7
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ExposureCompensation { value },
                    ))
                }
                ViscaResponseType::ExposureCompensationMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let on = response[2] == 0x02;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ExposureCompensationMode { on },
                    ))
                }
                ViscaResponseType::Iris => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Iris { position },
                    ))
                }
                ViscaResponseType::Shutter => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = ((response[4] as u16) << 4) | (response[5] as u16);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Shutter { position },
                    ))
                }
                ViscaResponseType::Bright => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = ((response[4] as u16) << 4) | (response[5] as u16);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Bright { position },
                    ))
                }
                ViscaResponseType::Gain => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let gain = (response[4] << 4) | response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Gain { gain },
                    ))
                }
                ViscaResponseType::GainLimit => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let limit = response[2];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::GainLimit { limit },
                    ))
                }
                ViscaResponseType::AntiFlicker => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    use crate::command::gain::AntiFlickerMode;
                    let mode = match response[2] {
                        0x00 => AntiFlickerMode::Off,
                        0x01 => AntiFlickerMode::Hz50,
                        0x02 => AntiFlickerMode::Hz60,
                        _ => return Err(ViscaError::UnexpectedResponseType),
                    };
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::AntiFlicker { mode },
                    ))
                }
                ViscaResponseType::Saturation => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let level = response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Saturation { level },
                    ))
                }
                ViscaResponseType::Hue => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let hue = response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::Hue { hue },
                    ))
                }
                ViscaResponseType::RedGain => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let gain = (response[2] as i8) - 10; // Convert 0x00..0x14 to -10..+10
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::RedGain { gain },
                    ))
                }
                ViscaResponseType::BlueGain => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let gain = (response[2] as i8) - 10; // Convert 0x00..0x14 to -10..+10
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::BlueGain { gain },
                    ))
                }
                ViscaResponseType::ImageFlip => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let vertical = (response[2] & 0x02) != 0;
                    let horizontal = (response[2] & 0x01) != 0;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ImageFlip { vertical, horizontal },
                    ))
                }
                ViscaResponseType::SharpnessMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    use super::SharpnessMode;
                    let mode = match response[2] {
                        0x02 => SharpnessMode::Auto,
                        0x03 => SharpnessMode::Manual,
                        _ => return Err(ViscaError::UnexpectedResponseType),
                    };
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::SharpnessMode { mode },
                    ))
                }
                ViscaResponseType::ColorTemperature => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let temperature = ((response[4] as u16) << 4) | (response[5] as u16);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ColorTemperature { temperature },
                    ))
                }
                ViscaResponseType::NoiseReduction2D => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let level = response[2];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::NoiseReduction2D { level },
                    ))
                }
                ViscaResponseType::NoiseReduction3D => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let level = response[2];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::NoiseReduction3D { level },
                    ))
                }
                ViscaResponseType::BlackWhite => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let on = response[2] == 0x04;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::BlackWhite { on },
                    ))
                }
                ViscaResponseType::FocusZone => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    use super::FocusZone;
                    let zone = match response[2] {
                        0x00 => FocusZone::Top,
                        0x01 => FocusZone::Center,
                        0x02 => FocusZone::Bottom,
                        _ => return Err(ViscaError::UnexpectedResponseType),
                    };
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::FocusZone { zone },
                    ))
                }
                ViscaResponseType::AFSensitivity => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    use super::AFSensitivity;
                    let sensitivity = match response[2] {
                        0x02 => AFSensitivity::High,
                        0x01 => AFSensitivity::Normal,
                        0x00 => AFSensitivity::Low,
                        _ => return Err(ViscaError::UnexpectedResponseType),
                    };
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::AFSensitivity { sensitivity },
                    ))
                }
                ViscaResponseType::FocusNearLimit => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let mut position = (response[2] as u16) << 12;
                    position |= (response[3] as u16) << 8;
                    position |= (response[4] as u16) << 4;
                    position |= response[5] as u16;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::FocusNearLimit { position },
                    ))
                }
                ViscaResponseType::DynamicRange => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let level = response[5];
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::DynamicRange { level },
                    ))
                }
                _ => Ok(ViscaResponse::Completion),
            }
        }
        0x60..=0x6F => Err(ViscaError::from_code(response[2])),
        _ => {
            error!("Unknown response: {:02X?}", response);
            Ok(ViscaResponse::Unknown(response.to_vec()))
        }
    }
}
