//! System-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode system-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::Version => {
            if payload.len() != 7 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let vendor = ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
            let model = ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
            let rom_version =
                ((payload.as_slice()[4] as u32) << 8) | (payload.as_slice()[5] as u32);
            let max_socket = payload.as_slice()[6];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            })))
        }
        ViscaResponseType::Resolution => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let resolution_mode = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Resolution(
                resolution_mode,
            ))))
        }
        ViscaResponseType::MenuOpenClose => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::MenuOpenClose {
                is_open,
            })))
        }
        ViscaResponseType::UsbAudio => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::UsbAudio { on })))
        }
        ViscaResponseType::Rtmp => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Rtmp { on })))
        }
        ViscaResponseType::NightDayMode => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::NightDayMode {
                is_night,
            })))
        }
        ViscaResponseType::Digital => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Digital { on })))
        }
        ViscaResponseType::AutoTrace => {
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
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::AutoTrace {
                enabled: on,
            })))
        }
        ViscaResponseType::NdFilterPreset => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::NdFilterPreset {
                    preset: payload.as_slice()[0],
                },
            )))
        }
        _ => None,
    }
}
