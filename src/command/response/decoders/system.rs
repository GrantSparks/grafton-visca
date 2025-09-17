//! System-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        InquiryData,
    },
    error::Error,
};

/// Decode system-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::Version => {
            if payload.len() != 7 {
                return Some(Err(Error::InvalidResponseLength));
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
                return Some(Err(Error::InvalidResponseLength));
            }
            let resolution_mode = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryData::Resolution(
                resolution_mode,
            ))))
        }
        InquiryKind::MenuOpenClose => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let is_open = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "menu_status",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed(
                            "Invalid menu status value. Expected 0x02 (closed) or 0x03 (open)",
                        ),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::MenuOpenClose {
                is_open,
            })))
        }
        InquiryKind::UsbAudio => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
                return Some(Err(Error::InvalidResponseLength));
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let is_night = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "night_day_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed(
                            "Invalid night/day mode value. Expected 0x02 (day) or 0x03 (night)",
                        ),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::NightDayMode {
                is_night,
            })))
        }
        InquiryKind::Digital => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "AutoTrace mode status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::AutoTrace {
                enabled: on,
            })))
        }
        InquiryKind::NdFilterPreset => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::NdFilterPreset {
                preset: payload.as_slice()[0],
            })))
        }
        _ => None,
    }
}
