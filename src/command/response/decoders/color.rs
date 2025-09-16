//! Color-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        response::types::{Response, ResponseKind},
        AutoWhiteBalanceSensitivity, InquiryResponse, WhiteBalanceMode,
    },
    error::Error,
};

/// Decode color-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::WhiteBalanceMode => {
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
            Some(Ok(Response::Inquiry(InquiryResponse::WhiteBalanceMode {
                mode,
            })))
        }
        ResponseKind::ColorTemperature => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                // Extract the color temperature from nibbles 2 and 3
                let temperature = nibbles.u8_pair(2) as u16;
                Some(Ok(Response::Inquiry(InquiryResponse::ColorTemperature {
                    temperature,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ResponseKind::RedChannel => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::RedChannel {
                gain: payload.as_slice()[0] as i8 - 10,
            })))
        }
        ResponseKind::BlueChannel => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::BlueChannel {
                gain: payload.as_slice()[0] as i8 - 10,
            })))
        }
        ResponseKind::RedTuning => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::RedTuning {
                level: payload.as_slice()[0],
            })))
        }
        ResponseKind::BlueTuning => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::BlueTuning {
                level: payload.as_slice()[0],
            })))
        }
        ResponseKind::AutoWhiteBalanceSensitivity => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let sensitivity = match payload.as_slice()[0] {
                0x00 => AutoWhiteBalanceSensitivity::Low,
                0x01 => AutoWhiteBalanceSensitivity::High,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "auto_white_balance_sensitivity",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown auto white balance sensitivity value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(
                InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity },
            )))
        }
        _ => None,
    }
}
