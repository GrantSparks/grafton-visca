//! Inquiry response decoders for VISCA protocol.
//!
//! Dispatch logic is generated from the built-in inquiry table in
//! [`crate::command::inquiry_structs`]. This module re-exports the generated
//! profile-independent and profile-aware dispatcher entry points.

// Re-export the generated dispatchers as the canonical decoder entry points.
pub(crate) use crate::command::inquiry_structs::{dispatch, dispatch_for};

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::command::{response::payload::Payload, InquiryData, InquiryKind, Response};

    /// Helper to dispatch a raw payload slice for a given inquiry kind.
    #[allow(clippy::unwrap_used)]
    fn decode(kind: InquiryKind, raw: &[u8]) -> Response {
        let payload = Payload::new(raw);
        dispatch(kind, payload).unwrap()
    }

    #[test]
    fn test_red_channel_g2_wire_format() {
        let resp = decode(InquiryKind::RedChannel, &[0x00, 0x00, 0x0D, 0x05]);
        match resp {
            Response::Inquiry(InquiryData::RedChannel { gain }) => assert_eq!(gain, 0xD5),
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_blue_channel_g2_wire_format() {
        let resp = decode(InquiryKind::BlueChannel, &[0x00, 0x00, 0x0B, 0x01]);
        match resp {
            Response::Inquiry(InquiryData::BlueChannel { gain }) => assert_eq!(gain, 0xB1),
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_color_temperature_g2_wire_format() {
        let resp = decode(InquiryKind::ColorTemperature, &[0x28]);
        match resp {
            Response::Inquiry(InquiryData::ColorTemperature { temperature }) => {
                assert_eq!(temperature, 0x28);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_red_tuning_g2_wire_format() {
        let resp = decode(InquiryKind::RedTuning, &[0x00, 0x00, 0x00, 0x0F]);
        match resp {
            Response::Inquiry(InquiryData::RedTuning { level }) => assert_eq!(level, 5),
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn test_blue_tuning_g2_wire_format() {
        let resp = decode(InquiryKind::BlueTuning, &[0x00, 0x00, 0x00, 0x0A]);
        match resp {
            Response::Inquiry(InquiryData::BlueTuning { level }) => assert_eq!(level, 0),
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
