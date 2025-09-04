//! System-related response decoders.

use std::borrow::Cow;

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
    payload: &[u8],
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::Version => {
            // Version response format: VV VV MM MM FF FF KK
            // VV VV = Vendor ID (2 bytes)
            // MM MM = Model ID (2 bytes)
            // FF FF = ROM version (2 bytes)
            // KK = Max socket number (1 byte)
            if payload.len() != 7 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let vendor = ((payload[0] as u16) << 8) | (payload[1] as u16);
            let model = ((payload[2] as u16) << 8) | (payload[3] as u16);
            let rom_version = ((payload[4] as u32) << 8) | (payload[5] as u32);
            let max_socket = payload[6];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            })))
        }
        ViscaResponseType::Resolution => {
            // Resolution inquiry response
            // Single byte indicating resolution mode
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Resolution values based on common Ptz camera patterns:
            // 0x00 = 1080p60, 0x01 = 1080p30, 0x02 = 720p60, 0x03 = 720p30, etc.
            let resolution_mode = payload[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Resolution(
                resolution_mode,
            ))))
        }
        ViscaResponseType::MenuOpenClose => {
            // Menu open/close status response
            // Single byte: 0x02 = closed, 0x03 = open
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let is_open = match payload[0] {
                0x02 => false, // Menu closed
                0x03 => true,  // Menu open
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "menu_status",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
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
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "UsbAudio status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
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
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "RTMP status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Rtmp { on })))
        }
        ViscaResponseType::NightDayMode => {
            // Night/Day mode inquiry response
            // Single byte: 0x02 = Day mode, 0x03 = Night mode
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let is_night = match payload[0] {
                0x02 => false, // Day mode
                0x03 => true,  // Night mode
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "night_day_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
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
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "Digital mode status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Digital { on })))
        }
        ViscaResponseType::AutoTrace => {
            // Auto trace mode inquiry response
            // Single byte: 0x02 = Off, 0x03 = On
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "AutoTrace mode status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
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
                InquiryResponse::NdFilterPreset { preset: payload[0] },
            )))
        }
        _ => None,
    }
}
