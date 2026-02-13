//! Domain-specific decoders for VISCA inquiry responses.
//!
//! This module provides a consolidated decoder dispatch for all VISCA inquiry types.
//! The dispatch uses a single `match` statement for O(1) dispatch performance,
//! replacing the previous chained `.or_else()` approach.
//!
//! # Architecture
//!
//! The `dispatch` function matches `InquiryKind` variants to their corresponding
//! decoder logic. Each decoder extracts payload data and constructs the appropriate
//! `InquiryData` variant.
//!
//! # Decoder Categories
//!
//! Decoders are organized by their parsing pattern:
//!
//! - **Boolean**: Power, Backlight, etc. (0x02/0x03 convention)
//! - **Position (4-nibble)**: ZoomPosition, FocusPosition (4 nibbles → u16)
//! - **Position (8-nibble)**: PanTiltPosition (8 nibbles → 2×i16)
//! - **Direct byte**: Luminance, Contrast, GainLimit
//! - **Last nibble**: Iris, Saturation, Hue, Gain
//! - **Mode enum**: ExposureMode, WhiteBalanceMode, FocusMode
//! - **Offset values**: RedChannel, BlueChannel (subtract offset)
//! - **Bit flags**: FlipState (horizontal/vertical)

use std::borrow::Cow;

use super::payload::{BoolConvention, Nibbles, Nibbles4Or8, Payload};
use super::types::{InquiryKind, Response};
use crate::capabilities::{PanTilt, Profile};
use crate::command::image::{
    BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed, SharpnessMode,
};
use crate::command::resolution::{NdFilterPosition, PictureEffectMode, ResolutionMode};
use crate::command::system::{MotionSyncMode, MotionSyncPreset};
use crate::command::{
    AntiFlickerMode, AutoFocusSensitivity, AutoWhiteBalanceSensitivity, ExposureMode, FocusMode,
    FocusRange, FocusZone, InquiryData, WhiteBalanceMode,
};
use crate::error::{format_payload_hex, Error};
use crate::types::DefogLevel;

/// Decode an inquiry response by dispatching to the appropriate decoder.
///
/// This function provides O(1) dispatch via a single `match` statement,
/// compared to the previous O(n) chained decoder approach.
///
/// # Arguments
/// * `kind` - The expected inquiry response type
/// * `payload` - The response payload bytes
///
/// # Returns
/// * `Ok(Response)` - Successfully decoded response
/// * `Err(Error)` - Decoding error (invalid payload, unknown enum value, etc.)
pub(crate) fn dispatch(kind: InquiryKind, payload: Payload<'_>) -> Result<Response, Error> {
    match kind {
        // ============================================================
        // Power-related decoders
        // ============================================================
        InquiryKind::Power => {
            // Power uses inverted convention (0x02 = on)
            let on = payload.parse_bool("power_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::Power { on }))
        }
        InquiryKind::Standby => {
            let in_standby = payload.parse_bool("standby_mode", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::Standby { in_standby }))
        }

        // ============================================================
        // Pan/Tilt-related decoders
        // ============================================================
        InquiryKind::PanTiltPosition => decode_pan_tilt_position(payload),

        // ============================================================
        // Zoom-related decoders
        // ============================================================
        InquiryKind::ZoomPosition => {
            let nibbles = Nibbles4Or8::try_from(payload)?;
            // Per VISCA spec, zoom position is 16-bit (4 nibbles).
            // Some devices send 8 nibbles; we use only the first 4 per spec.
            if matches!(nibbles, Nibbles4Or8::N8(_)) {
                tracing::warn!(
                    "ZoomPosition: Received extended format (8 nibbles). Using first 4 nibbles (16-bit) per VISCA spec."
                );
            }
            let position = nibbles.first_u16();
            Ok(Response::Inquiry(InquiryData::ZoomPosition { position }))
        }
        InquiryKind::ZoomOut => {
            let active = payload.parse_bool("zoom_out_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::ZoomOut { active }))
        }
        InquiryKind::ZoomIn => {
            let active = payload.parse_bool("zoom_in_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::ZoomIn { active }))
        }
        InquiryKind::ZoomTeleWide => {
            let tele = payload.parse_bool("zoom_tele_wide_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::ZoomTeleWide { tele }))
        }

        // ============================================================
        // Focus-related decoders
        // ============================================================
        InquiryKind::FocusPosition => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = nibbles.u16_quad(0);
            Ok(Response::Inquiry(InquiryData::FocusPosition { position }))
        }
        InquiryKind::FocusNearLimit => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = nibbles.u16_quad(0);
            Ok(Response::Inquiry(InquiryData::FocusNearLimit { position }))
        }
        InquiryKind::FocusZone => {
            require_len(&payload, 1)?;
            let zone = match payload.as_slice()[0] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "focus_zone",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown focus zone value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::FocusZone { zone }))
        }
        InquiryKind::AutoFocusSensitivity => {
            require_len(&payload, 1)?;
            let sensitivity = match payload.as_slice()[0] {
                0x00 => AutoFocusSensitivity::Low,
                0x01 => AutoFocusSensitivity::Normal,
                0x02 => AutoFocusSensitivity::High,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "auto_focus_sensitivity",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown auto focus sensitivity value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::AutoFocusSensitivity {
                sensitivity,
            }))
        }
        InquiryKind::FocusMode => {
            require_len(&payload, 1)?;
            let mode = match payload.as_slice()[0] {
                0x02 => FocusMode::Auto,
                0x03 => FocusMode::Manual,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "focus_mode",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown focus mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::FocusMode { mode }))
        }
        InquiryKind::FocusRange => {
            require_nonempty(&payload)?;
            let range = FocusRange::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::FocusRange { range }))
        }
        InquiryKind::AutoFocus => {
            let enabled = payload.parse_bool("autofocus_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::AutoFocus { enabled }))
        }
        InquiryKind::FocusUnlock => {
            let unlocked = payload.parse_bool("focus_unlock", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::FocusUnlock { unlocked }))
        }
        InquiryKind::FocusNearFar => {
            let near = payload.parse_bool("focus_near_far_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::FocusNearFar { near }))
        }

        // ============================================================
        // Exposure-related decoders
        // ============================================================
        InquiryKind::ExposureMode => {
            require_len(&payload, 1)?;
            let mode = match payload.as_slice()[0] {
                0x00 => ExposureMode::Auto,
                0x03 => ExposureMode::Manual,
                0x0A => ExposureMode::Shutter,
                0x0B => ExposureMode::Iris,
                0x0D => ExposureMode::Bright,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "exposure_mode",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown exposure mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::ExposureMode { mode }))
        }
        InquiryKind::ExposureCompensationMode => {
            // ExposureCompensationMode uses inverted convention (0x02 = on)
            let on = payload.parse_bool("exposure_compensation_mode", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::ExposureCompensationMode {
                on,
            }))
        }
        InquiryKind::ExposureCompensation => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let raw_value = nibbles.u8_pair(2);
            #[allow(clippy::cast_possible_wrap)]
            let value = raw_value as i8 - 7;
            Ok(Response::Inquiry(InquiryData::ExposureCompensation {
                value,
            }))
        }
        InquiryKind::ExposureCompensationPosition => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = crate::types::ExposureCompensationPosition::new(nibbles.u16_quad(0));
            Ok(Response::Inquiry(
                InquiryData::ExposureCompensationPosition { position },
            ))
        }
        InquiryKind::Shutter => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = nibbles.u8_pair(2) as u16;
            Ok(Response::Inquiry(InquiryData::Shutter { position }))
        }
        InquiryKind::Iris => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::Iris {
                position: nibbles.last_nibble(),
            }))
        }
        InquiryKind::Brightness => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = nibbles.u16_quad(0);
            Ok(Response::Inquiry(InquiryData::Brightness { position }))
        }
        InquiryKind::Gain => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::GainLevel {
                gain: nibbles.last_nibble(),
            }))
        }
        InquiryKind::GainLimit => {
            require_len(&payload, 1)?;
            Ok(Response::Inquiry(InquiryData::GainLimit {
                limit: payload.as_slice()[0],
            }))
        }
        InquiryKind::IrisControl => {
            let auto = payload.parse_bool("iris_control", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::IrisControl { auto }))
        }
        InquiryKind::IrisUp => {
            let active = payload.parse_bool("iris_up_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::IrisUp { active }))
        }
        InquiryKind::IrisDown => {
            let active = payload.parse_bool("iris_down_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::IrisDown { active }))
        }

        // ============================================================
        // Color-related decoders
        // ============================================================
        InquiryKind::WhiteBalanceMode => {
            require_len(&payload, 1)?;
            let mode = match payload.as_slice()[0] {
                0x00 => WhiteBalanceMode::Auto,
                0x01 => WhiteBalanceMode::Indoor,
                0x02 => WhiteBalanceMode::Outdoor,
                0x03 => WhiteBalanceMode::OnePush,
                0x05 => WhiteBalanceMode::Manual,
                0x20 => WhiteBalanceMode::ColorTemperature,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "white_balance_mode",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown white balance mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::WhiteBalanceMode { mode }))
        }
        InquiryKind::ColorTemperature => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            // Extract the color temperature from nibbles 2 and 3
            let temperature = nibbles.u8_pair(2) as u16;
            Ok(Response::Inquiry(InquiryData::ColorTemperature {
                temperature,
            }))
        }
        InquiryKind::RedChannel => {
            require_len(&payload, 1)?;
            #[allow(clippy::cast_possible_wrap)]
            let gain = payload.as_slice()[0] as i8 - 10;
            Ok(Response::Inquiry(InquiryData::RedChannel { gain }))
        }
        InquiryKind::BlueChannel => {
            require_len(&payload, 1)?;
            #[allow(clippy::cast_possible_wrap)]
            let gain = payload.as_slice()[0] as i8 - 10;
            Ok(Response::Inquiry(InquiryData::BlueChannel { gain }))
        }
        InquiryKind::RedTuning => {
            require_nonempty(&payload)?;
            // Convert from wire format (0-20) to semantic value (-10 to +10)
            #[allow(clippy::cast_possible_wrap)]
            let level = payload.as_slice()[0] as i8 - 10;
            Ok(Response::Inquiry(InquiryData::RedTuning { level }))
        }
        InquiryKind::BlueTuning => {
            require_nonempty(&payload)?;
            // Convert from wire format (0-20) to semantic value (-10 to +10)
            #[allow(clippy::cast_possible_wrap)]
            let level = payload.as_slice()[0] as i8 - 10;
            Ok(Response::Inquiry(InquiryData::BlueTuning { level }))
        }
        InquiryKind::AutoWhiteBalanceSensitivity => {
            require_nonempty(&payload)?;
            // VISCA byte representation: High=0x00, Normal=0x01, Low=0x02
            let sensitivity = match payload.as_slice()[0] {
                0x00 => AutoWhiteBalanceSensitivity::High,
                0x01 => AutoWhiteBalanceSensitivity::Normal,
                0x02 => AutoWhiteBalanceSensitivity::Low,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "auto_wb_sensitivity",
                        value: Cow::Owned(format!("0x{v:02X}")),
                        reason: Cow::Borrowed(
                            "Invalid auto white balance sensitivity. Expected 0x00 (High), 0x01 (Normal), or 0x02 (Low)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(
                InquiryData::AutoWhiteBalanceSensitivity { sensitivity },
            ))
        }

        // ============================================================
        // Image-related decoders
        // ============================================================
        InquiryKind::Sharpness => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let value = nibbles.u8_pair(2);
            Ok(Response::Inquiry(InquiryData::Sharpness { value }))
        }
        InquiryKind::SharpnessPosition => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            let position = nibbles.u16_quad(0);
            Ok(Response::Inquiry(InquiryData::SharpnessPosition {
                position,
            }))
        }
        InquiryKind::SharpnessMode => {
            require_nonempty(&payload)?;
            let mode = match payload.as_slice()[0] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "sharpness_mode",
                        value: Cow::Owned(format!("0x{v:02X}")),
                        reason: Cow::Borrowed("Expected 0x02 (Auto) or 0x03 (Manual)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryData::SharpnessMode { mode }))
        }
        InquiryKind::Saturation => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::Saturation {
                level: nibbles.last_nibble(),
            }))
        }
        InquiryKind::Hue => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::Hue {
                hue: nibbles.last_nibble(),
            }))
        }
        InquiryKind::Contrast => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::Contrast {
                level: nibbles.last_nibble(),
            }))
        }
        InquiryKind::PictureEffect => {
            require_nonempty(&payload)?;
            Ok(Response::Inquiry(InquiryData::PictureEffect {
                effect: PictureEffectMode::from_byte(payload.as_slice()[0]),
            }))
        }
        InquiryKind::BlackWhite => {
            require_len(&payload, 1)?;
            Ok(Response::Inquiry(InquiryData::BlackWhite {
                on: payload.as_slice()[0] == 0x04,
            }))
        }
        InquiryKind::BlackWhiteMode => {
            require_len(&payload, 1)?;
            let mode = BlackWhiteMode::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::BlackWhiteMode { mode }))
        }
        InquiryKind::NoiseReduction2D => {
            require_len(&payload, 1)?;
            let level = payload.as_slice()[0];
            Ok(Response::Inquiry(InquiryData::NoiseReduction2D { level }))
        }
        InquiryKind::NoiseReduction3D => {
            require_len(&payload, 1)?;
            let level = payload.as_slice()[0];
            Ok(Response::Inquiry(InquiryData::NoiseReduction3D { level }))
        }
        InquiryKind::NoiseReductionMode => {
            require_len(&payload, 1)?;
            let mode = NoiseReductionMode::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::NoiseReductionMode { mode }))
        }
        InquiryKind::NoiseReductionSpeed => {
            require_len(&payload, 1)?;
            let speed = NoiseReductionSpeed::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::NoiseReductionSpeed {
                speed,
            }))
        }
        InquiryKind::NoiseReductionLevel => {
            require_len(&payload, 1)?;
            Ok(Response::Inquiry(InquiryData::NoiseReductionLevel(
                payload.as_slice()[0],
            )))
        }
        InquiryKind::FlipState => {
            require_nonempty(&payload)?;
            let mode = payload.as_slice()[0];
            let horizontal = (mode & 0x01) != 0;
            let vertical = (mode & 0x02) != 0;
            Ok(Response::Inquiry(InquiryData::FlipState {
                horizontal,
                vertical,
            }))
        }
        InquiryKind::DynamicRange => {
            require_len(&payload, 1)?;
            let level = payload.as_slice()[0];
            Ok(Response::Inquiry(InquiryData::DynamicRange { level }))
        }
        InquiryKind::Backlight => {
            // Backlight uses inverted convention (0x02 = on/true)
            let status = payload.parse_bool("backlight_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::Backlight { status }))
        }
        InquiryKind::Luminance => {
            let nibbles = Nibbles::<4>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::Luminance {
                level: nibbles.last_nibble(),
            }))
        }
        InquiryKind::NdFilter => {
            require_nonempty(&payload)?;
            Ok(Response::Inquiry(InquiryData::NdFilter {
                position: NdFilterPosition::from_byte(payload.as_slice()[0]),
            }))
        }
        InquiryKind::Gamma => {
            require_nonempty(&payload)?;
            Ok(Response::Inquiry(InquiryData::Gamma {
                value: payload.as_slice()[0],
            }))
        }
        InquiryKind::TwoToneMode => {
            // TwoToneMode uses inverted convention (0x02 = on)
            let on = payload.parse_bool("two_tone_mode_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::TwoToneMode { on }))
        }
        InquiryKind::DefogMode => {
            let enabled = payload.parse_bool("defog_mode", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::DefogMode { enabled }))
        }
        InquiryKind::DefogLevel => {
            require_nonempty(&payload)?;
            let level =
                DefogLevel::new(payload.as_slice()[0]).map_err(|_| Error::InvalidParameter {
                    parameter: "defog_level",
                    value: Cow::Owned(payload.as_slice()[0].to_string()),
                    reason: Cow::Borrowed("value out of range (0-5)"),
                })?;
            Ok(Response::Inquiry(InquiryData::DefogLevel { level }))
        }

        // ============================================================
        // System-related decoders
        // ============================================================
        InquiryKind::Version => {
            if payload.len() != 7 {
                return Err(Error::invalid_response_length(7, payload.as_slice()));
            }
            let vendor = ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
            let model = ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
            let rom_version =
                ((payload.as_slice()[4] as u32) << 8) | (payload.as_slice()[5] as u32);
            let max_socket = payload.as_slice()[6];
            Ok(Response::Inquiry(InquiryData::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }))
        }
        InquiryKind::Resolution => {
            require_len(&payload, 1)?;
            let resolution_mode = ResolutionMode::from_byte(payload.as_slice()[0]);
            Ok(Response::Inquiry(InquiryData::Resolution(resolution_mode)))
        }
        InquiryKind::MenuOpenClose => {
            let is_open = payload.parse_bool("menu_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::MenuOpenClose { is_open }))
        }
        InquiryKind::UsbAudio => {
            let on = payload.parse_bool("usb_audio_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::UsbAudio { on }))
        }
        InquiryKind::Rtmp => {
            let on = payload.parse_bool("rtmp_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::Rtmp { on }))
        }
        InquiryKind::NightDayMode => {
            let is_night = payload.parse_bool("night_day_mode", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::NightDayMode { is_night }))
        }
        InquiryKind::Digital => {
            let on = payload.parse_bool("digital_mode_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::Digital { on }))
        }
        InquiryKind::AutoTrace => {
            let enabled = payload.parse_bool("auto_trace", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::AutoTrace { enabled }))
        }
        InquiryKind::NdFilterPreset => {
            require_len(&payload, 1)?;
            let preset =
                crate::types::NdFilterPreset::new(payload.as_slice()[0]).map_err(|_| {
                    Error::InvalidParameter {
                        parameter: "nd_filter_preset",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }
                })?;
            Ok(Response::Inquiry(InquiryData::NdFilterPreset { preset }))
        }
        InquiryKind::DigitalPtz => {
            let enabled = payload.parse_bool("digital_ptz", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled }))
        }
        InquiryKind::BroadcastDomain => {
            require_len(&payload, 1)?;
            let domain =
                crate::types::BroadcastDomain::new(payload.as_slice()[0]).map_err(|_| {
                    Error::InvalidParameter {
                        parameter: "broadcast_domain",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }
                })?;
            Ok(Response::Inquiry(InquiryData::BroadcastDomain(domain)))
        }
        InquiryKind::MotionSyncMode => {
            require_len(&payload, 1)?;
            let mode = MotionSyncMode::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode }))
        }
        InquiryKind::MotionSyncPreset => {
            require_len(&payload, 1)?;
            let speed = MotionSyncPreset::try_from(payload.as_slice()[0])?;
            Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed }))
        }
        InquiryKind::NightDayPosition => {
            require_len(&payload, 1)?;
            Ok(Response::Inquiry(InquiryData::NightDayPosition {
                position: payload.as_slice()[0],
            }))
        }
        InquiryKind::NightDaySwitch => {
            let enabled = payload.parse_bool("night_day_switch", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::NightDaySwitch { enabled }))
        }

        // ============================================================
        // Tally-related decoders
        // ============================================================
        InquiryKind::TallyRed => {
            // TallyRed uses inverted convention (0x02 = on)
            let on = payload.parse_bool("tally_red_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::TallyRed { on }))
        }
        InquiryKind::TallyGreen => {
            // TallyGreen uses inverted convention (0x02 = on)
            let on = payload.parse_bool("tally_green_status", BoolConvention::OnIs02)?;
            Ok(Response::Inquiry(InquiryData::TallyGreen { on }))
        }
        InquiryKind::TallyStatus => {
            // TallyStatus has 2 bytes: red then green, both use standard convention
            if payload.len() < 2 {
                return Err(Error::invalid_response_length(2, payload.as_slice()));
            }
            let red_payload = Payload::new(&payload.as_slice()[0..1]);
            let green_payload = Payload::new(&payload.as_slice()[1..2]);
            let red_on = red_payload.parse_bool("tally_red_status", BoolConvention::OnIs03)?;
            let green_on =
                green_payload.parse_bool("tally_green_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::TallyStatus {
                red_on,
                green_on,
            }))
        }
        InquiryKind::TallyAutoAdjust => {
            let on = payload.parse_bool("tally_auto_adjust_status", BoolConvention::OnIs03)?;
            Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on }))
        }

        // ============================================================
        // Flicker mode decoder
        // ============================================================
        InquiryKind::FlickerMode => {
            require_len(&payload, 1)?;
            let mode = match payload.as_slice()[0] {
                0x00 => AntiFlickerMode::Off,
                0x01 => AntiFlickerMode::Hz50,
                0x02 => AntiFlickerMode::Hz60,
                v => {
                    return Err(Error::InvalidParameter {
                        parameter: "flicker_mode",
                        value: Cow::Owned(format!("{v:02X}")),
                        reason: Cow::Borrowed("Unknown flicker mode value"),
                    });
                }
            };
            Ok(Response::Inquiry(InquiryData::FlickerMode { mode }))
        }
    }
}

/// Profile-aware decoder dispatch for responses that need coordinate conversion.
///
/// This function uses the Profile's `COORDINATE_SYSTEM` to correctly convert
/// pan/tilt values from camera coordinates to logical coordinates.
pub(crate) fn dispatch_for<P: Profile + PanTilt>(
    kind: InquiryKind,
    payload: Payload<'_>,
) -> Result<Response, Error> {
    // Special handling for PanTiltPosition which needs coordinate conversion
    if kind == InquiryKind::PanTiltPosition {
        return decode_pan_tilt_position_for::<P>(payload);
    }

    // For all other response types, use the standard dispatch
    dispatch(kind, payload)
}

// ============================================================
// Helper functions
// ============================================================

/// Require payload to have exactly the specified length.
#[inline]
fn require_len(payload: &Payload<'_>, expected: usize) -> Result<(), Error> {
    if payload.len() != expected {
        return Err(Error::invalid_response_length(expected, payload.as_slice()));
    }
    Ok(())
}

/// Require payload to be non-empty.
#[inline]
fn require_nonempty(payload: &Payload<'_>) -> Result<(), Error> {
    if payload.is_empty() {
        return Err(Error::invalid_response_length(1, payload.as_slice()));
    }
    Ok(())
}

/// Decode PanTiltPosition without profile awareness.
///
/// This decoder interprets pan/tilt positions as signed 16-bit values.
fn decode_pan_tilt_position(payload: Payload<'_>) -> Result<Response, Error> {
    if payload.len() == 8 {
        let nibbles = Nibbles::<8>::try_from(payload)?;
        let pan = nibbles.i16_quad(0);
        let tilt = nibbles.i16_quad(4);
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else if payload.len() == 4 {
        tracing::warn!(
            "PanTiltPosition: Received compact format (4 bytes). Payload: {:02X?}. Treating as home position.",
            payload.as_slice()
        );
        let pan = if payload.len() >= 2 {
            #[allow(clippy::cast_possible_wrap)]
            let p = ((payload.as_slice()[0] as i16) << 8) | (payload.as_slice()[1] as i16);
            p
        } else {
            0
        };
        let tilt = if payload.len() >= 4 {
            #[allow(clippy::cast_possible_wrap)]
            let t = ((payload.as_slice()[2] as i16) << 8) | (payload.as_slice()[3] as i16);
            t
        } else {
            0
        };
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else {
        // Return error to indicate this decoder doesn't handle this payload
        tracing::debug!(
            "PanTiltPosition: Payload length {} doesn't match pan/tilt format (expected 8 or 4 bytes)",
            payload.len()
        );
        Err(Error::DecoderNotFound {
            inquiry_kind: InquiryKind::PanTiltPosition,
            payload_hex: format_payload_hex(payload.as_slice()),
        })
    }
}

/// Profile-aware decode for pan/tilt-related inquiry responses.
///
/// This function uses the profile's coordinate system to convert camera
/// coordinates to logical coordinates.
fn decode_pan_tilt_position_for<P: Profile + PanTilt>(
    payload: Payload<'_>,
) -> Result<Response, Error> {
    if payload.len() == 8 {
        let nibbles = Nibbles::<8>::try_from(payload)?;
        // Extract as u16 values first (camera coordinates)
        let pan_u16 = nibbles.u16_quad(0);
        let tilt_u16 = nibbles.u16_quad(4);

        // Convert from camera coordinates to logical coordinates using profile's coordinate system
        let (pan, tilt) = P::COORDINATE_SYSTEM.convert_from_camera_coords(pan_u16, tilt_u16);

        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else if payload.len() == 4 {
        tracing::warn!(
            "PanTiltPosition: Received compact format (4 bytes). Payload: {:02X?}. Treating as home position.",
            payload.as_slice()
        );
        // For compact format, extract as u16 and convert
        let pan_u16 = if payload.len() >= 2 {
            ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16)
        } else {
            0x8000 // Center position for unsigned-centered systems
        };
        let tilt_u16 = if payload.len() >= 4 {
            ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16)
        } else {
            0x8000 // Center position for unsigned-centered systems
        };

        // Convert from camera coordinates to logical coordinates
        let (pan, tilt) = P::COORDINATE_SYSTEM.convert_from_camera_coords(pan_u16, tilt_u16);

        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else {
        // Return error to indicate this decoder doesn't handle this payload
        tracing::debug!(
            "PanTiltPosition: Payload length {} doesn't match pan/tilt format (expected 8 or 4 bytes)",
            payload.len()
        );
        Err(Error::DecoderNotFound {
            inquiry_kind: InquiryKind::PanTiltPosition,
            payload_hex: format_payload_hex(payload.as_slice()),
        })
    }
}
