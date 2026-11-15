//! Sony FR7 high-resolution zoom control and Clear Image Zoom readback.
//!
//! R7 version 4.00 p. 16 documents the drive and stop packets. Position
//! readback on p. 22 includes the Clear Image Zoom range described on p. 13.
//! This module provides no Clear Image Zoom position setter: R7 explicitly
//! disables direct zoom positioning while Clear Image Zoom is enabled.

use crate::{
    command::{
        bytes::FrameWriter,
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

crate::__grafton_visca_newtype! {
    /// FR7 high-resolution zoom speed, from `0000` through `7FFE` (R7 p. 16).
    ///
    /// Zero selects the slowest drive speed; it does not stop the lens.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "0", max = "0x7FFE", display_prefix = "High Resolution Zoom Speed")]
    pub struct HighResolutionZoomSpeed(u16);
}

/// Drives the FR7 zoom lens at a validated high-resolution speed (R7 p. 16).
///
/// This starts lens motion. Use [`Fr7ZoomHighResolutionStop`] to stop it;
/// even the minimum speed is a drive command rather than a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Fr7ZoomHighResolution {
    /// Drive toward telephoto at the requested speed.
    Tele(HighResolutionZoomSpeed),
    /// Drive toward wide angle at the requested speed.
    Wide(HighResolutionZoomSpeed),
}

impl WireEncode for Fr7ZoomHighResolution {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let (direction, speed) = match *self {
            Self::Tele(speed) => (2, speed),
            Self::Wide(speed) => (3, speed),
        };
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x17])
            .byte(direction)
            .nibbles::<4>(speed.value())
            .finish()
    }
}

/// Stops FR7 high-resolution zoom movement (R7 p. 16).
///
/// The stop packet has its own zero direction code and four zero padding
/// nibbles; it is distinct from driving at the minimum speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7ZoomHighResolutionStop;

impl WireEncode for Fr7ZoomHighResolutionStop {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x17, 0x00])
            .nibbles::<4>(0_u16)
            .finish()
    }
}

crate::__grafton_visca_newtype! {
    /// FR7 optical and Clear Image Zoom position readback (R7 pp. 13 and 22).
    ///
    /// `0000` is optical wide, `4000` is optical tele, `5556` is the 1.5x
    /// Clear Image Zoom endpoint at 3840x2160 or higher, and `6000` is the
    /// 2.0x endpoint. This is a readback value, with no direct-set command.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "0", max = "0x6000", display_prefix = "Clear Image Zoom Position")]
    pub struct ClearImageZoomPosition(u16);
}

pub(crate) fn decode_clear_image_zoom_position(
    payload: Payload<'_>,
) -> Result<ClearImageZoomPosition, Error> {
    let value = Nibbles::<4>::try_from(payload)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    ClearImageZoomPosition::new(value)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn literal(command: &impl WireEncode, expected: &[u8]) {
        let mut frame = [0; 32];
        let written = command
            .write_into(CameraId::CAMERA_1, &mut frame)
            .expect("literal frame encodes");
        assert_eq!(&frame[..written], expected);
        for length in 0..expected.len() {
            let error = command
                .write_into(CameraId::CAMERA_1, &mut frame[..length])
                .expect_err("every shorter buffer fails");
            assert!(matches!(error, Error::BufferTooSmall { required, actual }
                if required == expected.len() && actual == length));
        }
        let written = command
            .write_into(CameraId::CAMERA_7, &mut frame)
            .expect("camera seven encodes");
        assert_eq!(frame[0], 0x87);
        assert_eq!(&frame[1..written], &expected[1..]);
    }

    #[test]
    fn r7_page_16_high_resolution_zoom_preserves_speed_width_and_stop_padding() {
        literal(
            &Fr7ZoomHighResolution::Tele(HighResolutionZoomSpeed::MIN),
            &[0x81, 1, 0x7e, 4, 0x17, 2, 0, 0, 0, 0, 0xff],
        );
        literal(
            &Fr7ZoomHighResolution::Tele(HighResolutionZoomSpeed::MAX),
            &[0x81, 1, 0x7e, 4, 0x17, 2, 7, 0xf, 0xf, 0xe, 0xff],
        );
        literal(
            &Fr7ZoomHighResolution::Wide(HighResolutionZoomSpeed::MIN),
            &[0x81, 1, 0x7e, 4, 0x17, 3, 0, 0, 0, 0, 0xff],
        );
        literal(
            &Fr7ZoomHighResolution::Wide(HighResolutionZoomSpeed::MAX),
            &[0x81, 1, 0x7e, 4, 0x17, 3, 7, 0xf, 0xf, 0xe, 0xff],
        );
        literal(
            &Fr7ZoomHighResolutionStop,
            &[0x81, 1, 0x7e, 4, 0x17, 0, 0, 0, 0, 0, 0xff],
        );
    }

    #[test]
    fn zoom_speed_and_position_validate_every_u16_wire_value() {
        for wire in 0..=u16::MAX {
            let speed = HighResolutionZoomSpeed::new(wire);
            assert_eq!(speed.is_ok(), wire <= 0x7ffe, "speed {wire:04x}");
            if let Ok(speed) = speed {
                assert_eq!(speed.value(), wire);
            }
            let position = ClearImageZoomPosition::new(wire);
            assert_eq!(position.is_ok(), wire <= 0x6000, "position {wire:04x}");
            if let Ok(position) = position {
                assert_eq!(position.value(), wire);
            }
        }
    }

    #[test]
    fn r7_page_22_position_reply_preserves_optical_and_clear_image_endpoints() {
        for (payload, value) in [
            ([0, 0, 0, 0], 0),
            ([4, 0, 0, 0], 0x4000),
            ([5, 5, 5, 6], 0x5556),
            ([6, 0, 0, 0], 0x6000),
        ] {
            assert_eq!(
                decode_clear_image_zoom_position(Payload::new(&payload))
                    .unwrap()
                    .value(),
                value,
            );
        }
        for payload in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 0][..], &[0; 5][..]] {
            assert!(decode_clear_image_zoom_position(Payload::new(payload)).is_err());
        }
        for position in 0..4 {
            let mut payload = [0; 4];
            payload[position] = 0x10;
            assert!(decode_clear_image_zoom_position(Payload::new(&payload)).is_err());
        }
        assert!(decode_clear_image_zoom_position(Payload::new(&[6, 0, 0, 1])).is_err());
        assert!(decode_clear_image_zoom_position(Payload::new(&[0xf; 4])).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_checks_zoom_values_and_rejects_unknown_drive_variants() {
        fn round_trip<T>(value: T)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
        }
        for speed in [HighResolutionZoomSpeed::MIN, HighResolutionZoomSpeed::MAX] {
            round_trip(speed);
            round_trip(Fr7ZoomHighResolution::Tele(speed));
            round_trip(Fr7ZoomHighResolution::Wide(speed));
        }
        for position in [ClearImageZoomPosition::MIN, ClearImageZoomPosition::MAX] {
            round_trip(position);
        }
        round_trip(Fr7ZoomHighResolutionStop);
        for invalid in ["-1", "32767", "65535", "0.5", "null"] {
            assert!(serde_json::from_str::<HighResolutionZoomSpeed>(invalid).is_err());
        }
        for invalid in ["-1", "24577", "65535", "0.5", "null"] {
            assert!(serde_json::from_str::<ClearImageZoomPosition>(invalid).is_err());
        }
        assert!(serde_json::from_str::<Fr7ZoomHighResolution>(r#"{"tele":32767}"#).is_err());
        assert!(serde_json::from_str::<Fr7ZoomHighResolution>(r#"{"wide":-1}"#).is_err());
        assert!(serde_json::from_str::<Fr7ZoomHighResolution>(r#"{"stop":0}"#).is_err());
        assert!(serde_json::from_str::<Fr7ZoomHighResolutionStop>(r#"{"speed":1}"#).is_err());
    }
}
