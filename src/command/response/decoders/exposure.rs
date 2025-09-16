//! Exposure-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
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
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::ExposureMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match payload.as_slice()[0] {
                0x00 => ExposureMode::Auto,
                0x03 => ExposureMode::Manual,
                0x0A => ExposureMode::Shutter,
                0x0B => ExposureMode::Iris,
                0x0D => ExposureMode::Bright,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "exposure_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
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
                    on: payload.as_slice()[0] == 0x02,
                },
            )))
        }
        ViscaResponseType::ExposureCompensation => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let raw_value = nibbles.u8_pair(2);
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::ExposureCompensation {
                        value: raw_value as i8 - 7,
                    },
                )))
            }
            Err(e) => Some(Err(e)),
        },
        ViscaResponseType::ExposureCompensationPosition => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::ExposureCompensationPosition { position },
                )))
            }
            Err(e) => Some(Err(e)),
        },
        ViscaResponseType::Shutter => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u8_pair(2) as u16;
                Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Shutter {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ViscaResponseType::Iris => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the iris position from the last nibble
            let position = payload.as_slice()[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Iris {
                position,
            })))
        }
        ViscaResponseType::Bright => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Bright {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ViscaResponseType::Gain => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the gain value from the last nibble
            let gain = payload.as_slice()[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::GainLevel {
                gain,
            })))
        }
        ViscaResponseType::GainLimit => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::GainLimit {
                limit: payload.as_slice()[0],
            })))
        }
        ViscaResponseType::IrisControl => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let auto = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "iris_control",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
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
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisUp status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
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
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisDown status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
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
