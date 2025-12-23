//! System-related response decoders.

use std::borrow::Cow;

use super::super::payload::{BoolConvention, Payload};
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
            let is_open = match payload.parse_bool("menu_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::MenuOpenClose {
                is_open,
            })))
        }
        InquiryKind::UsbAudio => {
            let on = match payload.parse_bool("usb_audio_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::UsbAudio { on })))
        }
        InquiryKind::Rtmp => {
            let on = match payload.parse_bool("rtmp_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::Rtmp { on })))
        }
        InquiryKind::NightDayMode => {
            let is_night = match payload.parse_bool("night_day_mode", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::NightDayMode {
                is_night,
            })))
        }
        InquiryKind::Digital => {
            let on = match payload.parse_bool("digital_mode_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::Digital { on })))
        }
        InquiryKind::AutoTrace => {
            let enabled = match payload.parse_bool("auto_trace", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::AutoTrace { enabled })))
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
            let enabled = match payload.parse_bool("digital_ptz", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled })))
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
            let enabled = match payload.parse_bool("night_day_switch", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::NightDaySwitch {
                enabled,
            })))
        }
        _ => None,
    }
}
