//! System-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        system::{MotionSyncMode, MotionSyncPreset},
        InquiryData,
    },
    error::Error,
};

/// Decode system-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::Version => {
            if payload.len() != 7 {
                return Some(Err(Error::invalid_response_length(7, payload.as_slice())));
            }
            let vendor = ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
            let model = ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
            let rom_version =
                ((payload.as_slice()[4] as u32) << 8) | (payload.as_slice()[5] as u32);
            let max_socket = payload.as_slice()[6];
            Some(Ok(Response::Inquiry(InquiryData::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            })))
        }
        InquiryKind::Resolution => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let resolution_mode =
                crate::command::resolution::ResolutionMode::from_byte(payload.as_slice()[0]);
            Some(Ok(Response::Inquiry(InquiryData::Resolution(
                resolution_mode,
            ))))
        }
        InquiryKind::MenuOpenClose => {
            Some(super::super::parse_menu_open_close(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::UsbAudio => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "UsbAudio status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::UsbAudio { on })))
        }
        InquiryKind::Rtmp => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "RTMP status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::Rtmp { on })))
        }
        InquiryKind::NightDayMode => {
            Some(super::super::parse_night_day_mode(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::Digital => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "Digital mode status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::Digital { on })))
        }
        InquiryKind::AutoTrace => {
            Some(super::super::parse_auto_trace(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::NdFilterPreset => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let preset = match crate::types::NdFilterPreset::new(payload.as_slice()[0]) {
                Ok(p) => p,
                Err(_) => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "nd_filter_preset",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::NdFilterPreset {
                preset,
            })))
        }
        InquiryKind::DigitalPtz => {
            Some(super::super::parse_digital_ptz(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::BroadcastDomain => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let domain = match crate::types::BroadcastDomain::new(payload.as_slice()[0]) {
                Ok(d) => d,
                Err(_) => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "broadcast_domain",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::BroadcastDomain(domain))))
        }
        InquiryKind::MotionSyncMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match MotionSyncMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode })))
        }
        InquiryKind::MotionSyncPreset => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let speed = match MotionSyncPreset::try_from(payload.as_slice()[0]) {
                Ok(speed) => speed,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::MotionSyncPreset {
                speed,
            })))
        }
        InquiryKind::NightDayPosition => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::NightDayPosition {
                position: payload.as_slice()[0],
            })))
        }
        InquiryKind::NightDaySwitch => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let enabled = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "night_day_switch",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed(
                            "Invalid night/day switch value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::NightDaySwitch {
                enabled,
            })))
        }
        _ => None,
    }
}
