//! Exposure-related response decoders.

use std::borrow::Cow;

use super::super::nibbles::{combine_nibbles_u16, combine_nibbles_u8};
use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        ExposureMode, InquiryResponse,
    },
    error::Error,
};

/// Decode exposure-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: &[u8],
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::ExposureMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match payload[0] {
                0x00 => ExposureMode::Auto,
                0x03 => ExposureMode::Manual,
                0x0A => ExposureMode::Shutter,
                0x0B => ExposureMode::Iris,
                0x0D => ExposureMode::Bright,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "exposure_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown exposure mode value"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureMode {
                mode,
            })))
        }
        ViscaResponseType::ExposureCompensationMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::ExposureCompensationMode {
                    on: payload[0] == 0x02,
                },
            )))
        }
        ViscaResponseType::ExposureCompensation => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let raw_value = combine_nibbles_u8(&payload[2..4]);
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::ExposureCompensation {
                    value: raw_value as i8 - 7,
                },
            )))
        }
        ViscaResponseType::ExposureCompensationPosition => {
            // Exposure compensation position inquiry response
            // 4 bytes: PP PP (position as nibbles)
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::ExposureCompensationPosition { position },
            )))
        }
        ViscaResponseType::Shutter => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let position = combine_nibbles_u8(&payload[2..4]) as u16;
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Shutter {
                position,
            })))
        }
        ViscaResponseType::Iris => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the iris position from the last nibble
            let position = payload[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Iris {
                position,
            })))
        }
        ViscaResponseType::Bright => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Bright {
                position,
            })))
        }
        ViscaResponseType::Gain => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the gain value from the last nibble
            let gain = payload[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::GainLevel {
                gain,
            })))
        }
        ViscaResponseType::GainLimit => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::GainLimit {
                limit: payload[0],
            })))
        }
        ViscaResponseType::IrisControl => {
            // Iris control inquiry response
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let auto = match payload[0] {
                0x02 => false, // Manual iris control
                0x03 => true,  // Auto iris control
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "iris_control",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid iris control value. Expected 0x02 (manual) or 0x03 (auto)",
                        ),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::IrisControl {
                auto,
            })))
        }
        ViscaResponseType::IrisUp => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisUp status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::IrisUp {
                active,
            })))
        }
        ViscaResponseType::IrisDown => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisDown status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::IrisDown {
                active,
            })))
        }
        _ => None,
    }
}
