//! Sony FR7 exposure and relative iris controls from R7, version 4.00.
//!
//! Commands are documented on p. 14 and inquiry replies on p. 21. AE level
//! changes only by up/down commands. Iris movement takes a relative step
//! count; R7 documents auto-iris state, but no absolute iris-position inquiry.

use crate::wire::WireValue;

use crate::{
    command::{
        bytes::{
            constants::{exposure, sony_fr7_exposure},
            FrameWriter,
        },
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

/// FR7 AE level in quarter-EV steps, from -12 through +12 (R7 p. 21).
///
/// `-12`, `0` and `12` represent -3.0 EV, neutral and +3.0 EV. Protocol
/// values `00`, `0C` and `18` encode those levels, respectively.
#[crate::value(min=-12, max=12, display="AE Level (quarter EV) {}", wire=biased(12,u8))]
pub struct AeLevel(i8);

impl AeLevel {
    /// Converts -3.0 through +3.0 EV to the nearest quarter-EV level.
    ///
    /// Halfway values round away from zero. Nonfinite values and values
    /// outside the physical domain are refused before rounding.
    pub fn from_ev(ev: f64) -> Result<Self, Error> {
        if !(-3.0..=3.0).contains(&ev) {
            return Err(Error::invalid_parameter(
                "ae_level_ev",
                ev.to_string(),
                "AE level must be finite and between -3.0 and +3.0 EV",
            ));
        }
        Self::new((ev * 4.0).round() as i8)
    }

    /// Returns the exposure adjustment in EV.
    #[must_use]
    pub fn to_ev(self) -> f64 {
        f64::from(self.value()) / 4.0
    }
}

/// Adjusts the FR7 AE level by one step (R7 p. 14).
///
/// R7 documents up/down commands, with no direct-set or reset packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Fr7AeLevel {
    /// Increase the AE level by one quarter EV.
    Up,
    /// Decrease the AE level by one quarter EV.
    Down,
}

impl WireEncode for Fr7AeLevel {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&exposure::COMPENSATION_STEP)
            .byte(match self {
                Self::Up => 2,
                Self::Down => 3,
            })
            .finish()
    }
}

macro_rules! automatic_control {
    ($(#[$doc:meta])* $name:ident, $prefix:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub struct $name {
            enabled: bool,
        }

        impl $name {
            /// Creates an enable or disable command.
            #[must_use]
            pub const fn new(enabled: bool) -> Self {
                Self { enabled }
            }

            /// Returns the requested enablement state.
            #[must_use]
            pub const fn enabled(self) -> bool {
                self.enabled
            }
        }

        impl WireEncode for $name {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                FrameWriter::new(camera_id, buffer)
                    .bytes(&$prefix)
                    .byte(if self.enabled { 2 } else { 3 })
                    .finish()
            }
        }
    };
}

automatic_control! {
    /// Enables or disables FR7 automatic gain control (R7 p. 14).
    Fr7AutoGain, sony_fr7_exposure::AUTO_GAIN
}
automatic_control! {
    /// Enables or disables the FR7 automatic shutter (R7 p. 14).
    Fr7AutoShutter, sony_fr7_exposure::AUTO_SHUTTER
}
automatic_control! {
    /// Enables or disables FR7 automatic iris control (R7 p. 14).
    Fr7AutoIris, sony_fr7_exposure::AUTO_IRIS
}

/// Relative FR7 iris adjustment count, from 1 through 255 (R7 p. 14).
///
/// Each step changes aperture by approximately 1/256 EV. The count does
/// not identify an absolute aperture position.
#[crate::value(min = 1, max = 255, display = "Iris Step {}")]
pub struct IrisStep(u8);

macro_rules! iris_adjustment {
    ($(#[$doc:meta])* $name:ident, $direction:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub struct $name {
            steps: IrisStep,
        }

        impl $name {
            /// Creates an iris adjustment with a validated step count.
            #[must_use]
            pub const fn new(steps: IrisStep) -> Self {
                Self { steps }
            }

            /// Returns the relative adjustment count.
            #[must_use]
            pub const fn steps(self) -> IrisStep {
                self.steps
            }
        }

        impl WireEncode for $name {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                FrameWriter::new(camera_id, buffer)
                    .bytes(&sony_fr7_exposure::IRIS_STEP)
                    .byte($direction)
                    .nibbles::<2>(self.steps.value())
                    .finish()
            }
        }
    };
}

iris_adjustment! {
    /// Opens the FR7 iris by the requested relative count (R7 p. 14).
    ///
    /// This moves the lens iris toward a brighter exposure. R7 provides no
    /// iris-position inquiry with which to observe an absolute target.
    Fr7IrisUp, 2
}
iris_adjustment! {
    /// Closes the FR7 iris by the requested relative count (R7 p. 14).
    ///
    /// This moves the lens iris toward a darker exposure. R7 provides no
    /// iris-position inquiry with which to observe an absolute target.
    Fr7IrisDown, 3
}

pub(crate) fn decode_ae_level(payload: Payload<'_>) -> Result<AeLevel, Error> {
    AeLevel::from_wire(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)
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
    fn r7_page_14_ae_and_automatic_controls_match_their_own_packets() {
        literal(&Fr7AeLevel::Up, &[0x81, 1, 4, 0x0e, 2, 0xff]);
        literal(&Fr7AeLevel::Down, &[0x81, 1, 4, 0x0e, 3, 0xff]);
        for (enabled, code) in [(true, 2), (false, 3)] {
            let gain = Fr7AutoGain::new(enabled);
            assert_eq!(gain.enabled(), enabled);
            literal(&gain, &[0x81, 1, 0x7e, 1, 0x75, code, 0xff]);
            let shutter = Fr7AutoShutter::new(enabled);
            assert_eq!(shutter.enabled(), enabled);
            literal(&shutter, &[0x81, 1, 5, 0x35, code, 0xff]);
            let iris = Fr7AutoIris::new(enabled);
            assert_eq!(iris.enabled(), enabled);
            literal(&iris, &[0x81, 1, 5, 0x34, code, 0xff]);
        }
    }

    #[test]
    fn r7_page_14_iris_steps_are_two_nibbles_in_both_directions() {
        for (steps, digits) in [(IrisStep::MIN, [0, 1]), (IrisStep::MAX, [0xf, 0xf])] {
            let up = Fr7IrisUp::new(steps);
            assert_eq!(up.steps(), steps);
            literal(
                &up,
                &[0x81, 1, 0x7e, 4, 0x4b, 2, digits[0], digits[1], 0xff],
            );
            let down = Fr7IrisDown::new(steps);
            assert_eq!(down.steps(), steps);
            literal(
                &down,
                &[0x81, 1, 0x7e, 4, 0x4b, 3, digits[0], digits[1], 0xff],
            );
        }
        assert!(IrisStep::new(0).is_err());
        for count in 1..=u8::MAX {
            assert_eq!(IrisStep::new(count).unwrap().value(), count);
        }
    }

    #[test]
    fn ae_levels_round_trip_every_source_code_without_overflow_aliases() {
        for wire in 0..=u8::MAX {
            let result = AeLevel::from_wire(wire);
            if wire <= 24 {
                let level = result.unwrap();
                assert_eq!(level.to_wire(), wire);
                assert_eq!(AeLevel::from_ev(level.to_ev()).unwrap(), level);
            } else {
                assert!(result.is_err(), "AE level wire {wire:02x}");
            }
        }
        for invalid in [i8::MIN, -13, 13, i8::MAX] {
            assert!(AeLevel::new(invalid).is_err());
        }
    }

    #[test]
    fn ev_conversion_rounds_half_away_from_zero_after_domain_validation() {
        for (ev, quarters) in [
            (-3.0, -12),
            (-0.375, -2),
            (-0.125, -1),
            (-0.124, 0),
            (0.124, 0),
            (0.125, 1),
            (0.375, 2),
            (3.0, 12),
        ] {
            assert_eq!(AeLevel::from_ev(ev).unwrap().value(), quarters);
        }
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -3.001, 3.001] {
            assert!(AeLevel::from_ev(invalid).is_err());
        }
    }

    #[test]
    fn r7_page_21_ae_reply_requires_exact_zero_padded_nibbles() {
        for (payload, quarters) in [([0, 0, 0, 0], -12), ([0, 0, 0, 0xc], 0), ([0, 0, 1, 8], 12)] {
            assert_eq!(
                decode_ae_level(Payload::new(&payload)).unwrap().value(),
                quarters
            );
        }
        for payload in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 0][..], &[0; 5][..]] {
            assert!(decode_ae_level(Payload::new(payload)).is_err());
        }
        for position in 0..4 {
            let mut payload = [0; 4];
            payload[position] = 0x10;
            assert!(decode_ae_level(Payload::new(&payload)).is_err());
        }
        for payload in [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 9]] {
            assert!(decode_ae_level(Payload::new(&payload)).is_err());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_keeps_checked_exposure_values_and_command_fields() {
        fn round_trip<T>(value: T)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
        }
        for value in [AeLevel::MIN, AeLevel::new(0).unwrap(), AeLevel::MAX] {
            round_trip(value);
        }
        for value in [IrisStep::MIN, IrisStep::MAX] {
            round_trip(value);
        }
        round_trip(Fr7AeLevel::Up);
        round_trip(Fr7AeLevel::Down);
        round_trip(Fr7AutoGain::new(true));
        round_trip(Fr7AutoShutter::new(false));
        round_trip(Fr7AutoIris::new(true));
        round_trip(Fr7IrisUp::new(IrisStep::MIN));
        round_trip(Fr7IrisDown::new(IrisStep::MAX));
        for invalid in ["-13", "13", "0.25", "null"] {
            assert!(serde_json::from_str::<AeLevel>(invalid).is_err());
        }
        for invalid in ["0", "256", "1.5"] {
            assert!(serde_json::from_str::<IrisStep>(invalid).is_err());
        }
        assert!(serde_json::from_str::<Fr7AeLevel>(r#""direct""#).is_err());
        assert!(serde_json::from_str::<Fr7AutoGain>(r#"{"enabled":2}"#).is_err());
        assert!(serde_json::from_str::<Fr7AutoShutter>(r#"{"enabled":true,"extra":0}"#).is_err());
        assert!(serde_json::from_str::<Fr7AutoIris>(r#"{"enabled":null}"#).is_err());
        assert!(serde_json::from_str::<Fr7IrisUp>(r#"{"steps":0}"#).is_err());
        assert!(serde_json::from_str::<Fr7IrisDown>(r#"{"steps":256}"#).is_err());
    }
}
