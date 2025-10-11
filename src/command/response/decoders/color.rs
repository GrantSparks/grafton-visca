//! Color-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        AutoWhiteBalanceSensitivity, InquiryData, WhiteBalanceMode,
    },
    error::Error,
};

/// Decode color-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::WhiteBalanceMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match payload.as_slice()[0] {
                0x00 => WhiteBalanceMode::Auto,
                0x01 => WhiteBalanceMode::Indoor,
                0x02 => WhiteBalanceMode::Outdoor,
                0x03 => WhiteBalanceMode::OnePush,
                0x05 => WhiteBalanceMode::Manual,
                0x20 => WhiteBalanceMode::ColorTemperature,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "white_balance_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown white balance mode value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::WhiteBalanceMode {
                mode,
            })))
        }
        InquiryKind::ColorTemperature => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                // Extract the color temperature from nibbles 2 and 3
                let temperature = nibbles.u8_pair(2) as u16;
                Some(Ok(Response::Inquiry(InquiryData::ColorTemperature {
                    temperature,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::RedChannel => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::RedChannel {
                gain: payload.as_slice()[0] as i8 - 10,
            })))
        }
        InquiryKind::BlueChannel => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::BlueChannel {
                gain: payload.as_slice()[0] as i8 - 10,
            })))
        }
        InquiryKind::RedTuning => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::RedTuning {
                level: payload.as_slice()[0],
            })))
        }
        InquiryKind::BlueTuning => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::BlueTuning {
                level: payload.as_slice()[0],
            })))
        }
        InquiryKind::AutoWhiteBalanceSensitivity => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let sensitivity = match payload.as_slice()[0] {
                0x00 => AutoWhiteBalanceSensitivity::High,
                0x01 => AutoWhiteBalanceSensitivity::Normal,
                0x02 => AutoWhiteBalanceSensitivity::Low,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "auto_white_balance_sensitivity",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown auto white balance sensitivity value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(
                InquiryData::AutoWhiteBalanceSensitivity { sensitivity },
            )))
        }
        _ => None,
    }
}
