//! Sony FR7 black, detail and knee controls from R7, version 4.00.
//!
//! Commands use the packets on p. 16; black and detail replies are on p. 21,
//! and knee replies and permission selectors are on p. 22. These settings are
//! plain image controls and do not start mechanical motion. Detail level uses
//! the separate shared sharpness command; this module models detail enablement.

use crate::{
    command::{
        bytes::{constants::sony_fr7_image, FrameWriter},
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

crate::__grafton_visca_newtype! {
    /// FR7 master, red or blue black level, in tenths of a displayed unit.
    ///
    /// `-990`, `0` and `990` represent -99.0, neutral and +99.0. R7 pp. 16
    /// and 21 encode them as `0000`, `03DE` and `07BC`, respectively.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "-990", max = "990", display_prefix = "Black Level (tenths)")]
    pub struct BlackLevel(i16);
}

impl BlackLevel {
    /// Creates a level from -99.0 through +99.0, rounding to the nearest tenth.
    ///
    /// Halfway values round away from zero. Nonfinite values and values
    /// outside the physical domain are refused before rounding.
    pub fn from_level(level: f64) -> Result<Self, Error> {
        if !(-99.0..=99.0).contains(&level) {
            return Err(Error::invalid_parameter(
                "black_level",
                level.to_string(),
                "level must be finite and between -99.0 and +99.0",
            ));
        }
        Self::new((level * 10.0).round() as i16)
    }

    /// Returns the displayed level, from -99.0 through +99.0.
    #[must_use]
    pub fn to_level(self) -> f64 {
        f64::from(self.value()) / 10.0
    }

    /// Decodes the biased `0000..=07BC` protocol value (R7 p. 21).
    pub const fn from_protocol_value(value: u16) -> Result<Self, Error> {
        if value > 1980 {
            return Err(Error::parameter_out_of_range(
                "black_level_wire",
                value as i32,
                0,
                1980,
            ));
        }
        Self::new(value as i16 - 990)
    }

    /// Returns the biased `0000..=07BC` protocol value (R7 p. 16).
    #[must_use]
    pub const fn to_protocol_value(self) -> u16 {
        (self.value() + 990) as u16
    }
}

crate::__grafton_visca_newtype! {
    /// Number of relative FR7 black-level adjustment steps (R7 p. 16).
    ///
    /// The step count `01..=FF` is distinct from a direct black level.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "1", max = "255", display_prefix = "Black Step")]
    pub struct BlackStep(u8);
}

crate::__grafton_visca_newtype! {
    /// FR7 knee slope, from -99 through +99 (R7 pp. 16 and 22).
    ///
    /// The protocol biases the displayed value by 99: `00`, `63` and `C6`
    /// represent -99, neutral and +99.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "-99", max = "99", display_prefix = "Knee Slope")]
    pub struct KneeSlope(i16);
}

impl KneeSlope {
    /// Decodes the biased `00..=C6` protocol value.
    pub const fn from_protocol_value(value: u8) -> Result<Self, Error> {
        Self::new(value as i16 - 99)
    }

    /// Returns the biased `00..=C6` protocol value.
    #[must_use]
    pub const fn to_protocol_value(self) -> u8 {
        (self.value() + 99) as u8
    }
}

crate::__grafton_visca_newtype! {
    /// FR7 knee point, in percent, from 75 through 109 (R7 pp. 16 and 22).
    ///
    /// Protocol values `00..=22` represent 75% through 109%.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "75", max = "109", display_prefix = "Knee Point (%)")]
    pub struct KneePoint(u8);
}

impl KneePoint {
    /// Decodes a `00..=22` protocol value into a percentage.
    pub const fn from_protocol_value(value: u8) -> Result<Self, Error> {
        if value > 34 {
            return Err(Error::parameter_out_of_range(
                "knee_point_wire",
                value as i32,
                0,
                34,
            ));
        }
        Self::new(value + 75)
    }

    /// Returns the `00..=22` protocol value for this percentage.
    #[must_use]
    pub const fn to_protocol_value(self) -> u8 {
        self.value() - 75
    }
}

macro_rules! black_control {
    ($(#[$doc:meta])* $name:ident, step = $step:expr, direct = $direct:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case", deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub enum $name {
            /// Increase the black level by the given number of steps.
            Up(BlackStep),
            /// Decrease the black level by the given number of steps.
            Down(BlackStep),
            /// Set an exact validated black level.
            Direct(BlackLevel),
        }

        impl WireEncode for $name {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                let frame = FrameWriter::new(camera_id, buffer);
                match *self {
                    Self::Up(step) => frame.bytes(&$step).byte(2).nibbles::<2>(step.value()),
                    Self::Down(step) => frame.bytes(&$step).byte(3).nibbles::<2>(step.value()),
                    Self::Direct(level) => frame.bytes(&$direct).nibbles::<4>(level.to_protocol_value()),
                }
                .finish()
            }
        }
    };
}

black_control! {
    /// FR7 master-black adjustment (R7 p. 16).
    Fr7MasterBlack, step = sony_fr7_image::MASTER_BLACK_STEP, direct = sony_fr7_image::MASTER_BLACK_DIRECT
}
black_control! {
    /// FR7 red-black adjustment (R7 p. 16).
    Fr7RedBlack, step = sony_fr7_image::RED_BLACK_STEP, direct = sony_fr7_image::RED_BLACK_DIRECT
}
black_control! {
    /// FR7 blue-black adjustment (R7 p. 16).
    Fr7BlueBlack, step = sony_fr7_image::BLUE_BLACK_STEP, direct = sony_fr7_image::BLUE_BLACK_DIRECT
}

macro_rules! boolean_setting {
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

boolean_setting! {
    /// Enables or disables FR7 detail processing (R7 p. 16).
    Fr7DetailSetting, sony_fr7_image::DETAIL_SETTING
}
boolean_setting! {
    /// Enables or disables the FR7 knee (R7 p. 16).
    Fr7KneeSetting, sony_fr7_image::KNEE_SETTING
}

macro_rules! protocol_choice {
    ($(#[$doc:meta])* $name:ident { $($(#[$variant_doc:meta])* $variant:ident = $value:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case", deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub enum $name {
            $($(#[$variant_doc])* $variant,)+
        }

        impl $name {
            /// Decodes a source-backed protocol code, refusing unknown values.
            pub fn from_protocol_value(value: u8) -> Result<Self, Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(Error::invalid_response(
                        concat!("valid ", stringify!($name), " protocol code"),
                        vec![value],
                    )),
                }
            }

            /// Returns this value's source-backed protocol code.
            #[must_use]
            pub const fn to_protocol_value(self) -> u8 {
                match self {
                    $(Self::$variant => $value,)+
                }
            }
        }
    };
}

protocol_choice! {
    /// FR7 knee mode (R7 pp. 16 and 22).
    Fr7KneeMode {
        /// Automatically adjust the knee.
        Auto = 0,
        /// Use the configured knee point and slope.
        Manual = 4,
    }
}
protocol_choice! {
    /// FR7 knee setting selected by a permission inquiry (R7 p. 22).
    Fr7KneeParameter {
        /// Knee enablement.
        Setting = 0,
        /// Automatic or manual mode.
        Mode = 1,
        /// Knee point.
        Point = 2,
        /// Knee slope.
        Slope = 3,
    }
}
protocol_choice! {
    /// FR7 knee parameter permission (R7 p. 22).
    Fr7KneePermission {
        /// The setting can be configured.
        Enabled = 0,
        /// The setting is unavailable.
        Disabled = 1,
        /// The acquired value is valid, but cannot be configured.
        DisplayOnly = 2,
    }
}

macro_rules! knee_setting {
    ($(#[$doc:meta])* $name:ident, $field:ident: $value:ty, $prefix:expr, $digits:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub struct $name {
            $field: $value,
        }

        impl $name {
            /// Creates a command carrying the validated setting.
            #[must_use]
            pub const fn new($field: $value) -> Self {
                Self { $field }
            }

            /// Returns the requested setting.
            #[must_use]
            pub const fn $field(self) -> $value {
                self.$field
            }
        }

        impl WireEncode for $name {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                FrameWriter::new(camera_id, buffer)
                    .bytes(&$prefix)
                    .nibbles::<$digits>(self.$field.to_protocol_value())
                    .finish()
            }
        }
    };
}

knee_setting! {
    /// Selects FR7 automatic or manual knee mode (R7 p. 16).
    Fr7SetKneeMode, mode: Fr7KneeMode, sony_fr7_image::KNEE_MODE, 1
}
knee_setting! {
    /// Sets the FR7 knee slope from -99 through +99 (R7 p. 16).
    Fr7SetKneeSlope, slope: KneeSlope, sony_fr7_image::KNEE_SLOPE, 2
}
knee_setting! {
    /// Sets the FR7 knee point from 75% through 109% (R7 p. 16).
    Fr7SetKneePoint, point: KneePoint, sony_fr7_image::KNEE_POINT, 2
}

pub(crate) fn decode_black_level(payload: Payload<'_>) -> Result<BlackLevel, Error> {
    let value = Nibbles::<4>::try_from(payload)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    BlackLevel::from_protocol_value(value)
}

pub(crate) fn decode_fr7_knee_mode(payload: Payload<'_>) -> Result<Fr7KneeMode, Error> {
    Fr7KneeMode::from_protocol_value(
        Nibbles::<1>::try_from(payload)?
            .byte(0)
            .ok_or(Error::InvalidResponseFormat)?,
    )
}

pub(crate) fn decode_knee_slope(payload: Payload<'_>) -> Result<KneeSlope, Error> {
    KneeSlope::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)
}

pub(crate) fn decode_knee_point(payload: Payload<'_>) -> Result<KneePoint, Error> {
    KneePoint::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)
}

pub(crate) fn decode_fr7_knee_permission(payload: Payload<'_>) -> Result<Fr7KneePermission, Error> {
    Fr7KneePermission::from_protocol_value(
        Nibbles::<1>::try_from(payload)?
            .byte(0)
            .ok_or(Error::InvalidResponseFormat)?,
    )
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
    fn r7_page_16_black_packets_preserve_master_and_vendor_registers() {
        literal(
            &Fr7MasterBlack::Up(BlackStep::MAX),
            &[0x81, 1, 5, 0x18, 2, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7MasterBlack::Down(BlackStep::MIN),
            &[0x81, 1, 5, 0x18, 3, 0, 1, 0xff],
        );
        literal(
            &Fr7MasterBlack::Direct(BlackLevel::new(0).unwrap()),
            &[0x81, 1, 5, 0x48, 0, 3, 0xd, 0xe, 0xff],
        );
        literal(
            &Fr7RedBlack::Up(BlackStep::MIN),
            &[0x81, 1, 0x7e, 1, 0x65, 2, 0, 1, 0xff],
        );
        literal(
            &Fr7RedBlack::Down(BlackStep::MAX),
            &[0x81, 1, 0x7e, 1, 0x65, 3, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7RedBlack::Direct(BlackLevel::MIN),
            &[0x81, 1, 0x7e, 4, 0x43, 0, 0, 0, 0, 0xff],
        );
        literal(
            &Fr7BlueBlack::Up(BlackStep::MAX),
            &[0x81, 1, 0x7e, 1, 0x66, 2, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7BlueBlack::Down(BlackStep::MIN),
            &[0x81, 1, 0x7e, 1, 0x66, 3, 0, 1, 0xff],
        );
        literal(
            &Fr7BlueBlack::Direct(BlackLevel::MAX),
            &[0x81, 1, 0x7e, 4, 0x44, 0, 7, 0xb, 0xc, 0xff],
        );
    }

    #[test]
    fn r7_page_16_detail_and_knee_boolean_packets_use_two_on_three_off() {
        for (enabled, code) in [(true, 2), (false, 3)] {
            let detail = Fr7DetailSetting::new(enabled);
            assert_eq!(detail.enabled(), enabled);
            literal(&detail, &[0x81, 1, 0x7e, 1, 0x60, code, 0xff]);
            let knee = Fr7KneeSetting::new(enabled);
            assert_eq!(knee.enabled(), enabled);
            literal(&knee, &[0x81, 1, 0x7e, 1, 0x6d, code, 0xff]);
        }
    }

    #[test]
    fn r7_page_16_knee_settings_use_unpadded_command_values() {
        for (mode, code) in [(Fr7KneeMode::Auto, 0), (Fr7KneeMode::Manual, 4)] {
            let command = Fr7SetKneeMode::new(mode);
            assert_eq!(command.mode(), mode);
            literal(&command, &[0x81, 1, 0x7e, 1, 0x54, code, 0xff]);
        }
        for (slope, digits) in [(-99, [0, 0]), (0, [6, 3]), (99, [0xc, 6])] {
            let slope = KneeSlope::new(slope).unwrap();
            let command = Fr7SetKneeSlope::new(slope);
            assert_eq!(command.slope(), slope);
            literal(
                &command,
                &[0x81, 1, 0x7e, 1, 0x6f, digits[0], digits[1], 0xff],
            );
        }
        for (point, digits) in [(75, [0, 0]), (109, [2, 2])] {
            let point = KneePoint::new(point).unwrap();
            let command = Fr7SetKneePoint::new(point);
            assert_eq!(command.point(), point);
            literal(
                &command,
                &[0x81, 1, 0x7e, 1, 0x6e, digits[0], digits[1], 0xff],
            );
        }
    }

    #[test]
    fn value_domains_round_trip_and_refuse_every_outside_protocol_code() {
        for wire in 0..=u16::MAX {
            let result = BlackLevel::from_protocol_value(wire);
            if wire <= 1980 {
                let level = result.unwrap();
                assert_eq!(level.to_protocol_value(), wire);
                assert_eq!(BlackLevel::from_level(level.to_level()).unwrap(), level);
            } else {
                assert!(result.is_err(), "black wire {wire:04x}");
            }
        }
        for wire in 0..=u8::MAX {
            let slope = KneeSlope::from_protocol_value(wire);
            if wire <= 198 {
                assert_eq!(slope.unwrap().to_protocol_value(), wire);
            } else {
                assert!(slope.is_err(), "knee slope wire {wire:02x}");
            }
            let point = KneePoint::from_protocol_value(wire);
            if wire <= 34 {
                assert_eq!(point.unwrap().to_protocol_value(), wire);
            } else {
                assert!(point.is_err(), "knee point wire {wire:02x}");
            }
        }
        for invalid in [i16::MIN, -991, 991, i16::MAX] {
            assert!(BlackLevel::new(invalid).is_err());
        }
        for invalid in [i16::MIN, -100, 100, i16::MAX] {
            assert!(KneeSlope::new(invalid).is_err());
        }
        for invalid in [0, 74, 110, u8::MAX] {
            assert!(KneePoint::new(invalid).is_err());
        }
        assert!(BlackStep::new(0).is_err());
        assert_eq!(BlackStep::new(1).unwrap(), BlackStep::MIN);
        assert_eq!(BlackStep::new(255).unwrap(), BlackStep::MAX);
    }

    #[test]
    fn black_level_rounds_half_away_from_zero_after_checking_the_physical_domain() {
        for (level, tenths) in [
            (-99.0, -990),
            (-0.15, -2),
            (-0.05, -1),
            (-0.049, 0),
            (0.049, 0),
            (0.05, 1),
            (0.15, 2),
            (99.0, 990),
        ] {
            assert_eq!(BlackLevel::from_level(level).unwrap().value(), tenths);
        }
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -99.001, 99.001] {
            assert!(BlackLevel::from_level(invalid).is_err());
        }
    }

    #[test]
    fn r7_pages_21_and_22_numeric_replies_are_exact_and_zero_padded() {
        for (payload, tenths) in [
            ([0, 0, 0, 0], -990),
            ([0, 3, 0xd, 0xe], 0),
            ([0, 7, 0xb, 0xc], 990),
        ] {
            assert_eq!(
                decode_black_level(Payload::new(&payload)).unwrap().value(),
                tenths
            );
        }
        for (payload, slope) in [([0, 0, 0, 0], -99), ([0, 0, 6, 3], 0), ([0, 0, 0xc, 6], 99)] {
            assert_eq!(
                decode_knee_slope(Payload::new(&payload)).unwrap().value(),
                slope
            );
        }
        for (payload, percent) in [([0, 0, 0, 0], 75), ([0, 0, 2, 2], 109)] {
            assert_eq!(
                decode_knee_point(Payload::new(&payload)).unwrap().value(),
                percent
            );
        }
        for payload in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 0][..], &[0; 5][..]] {
            assert!(decode_black_level(Payload::new(payload)).is_err());
            assert!(decode_knee_slope(Payload::new(payload)).is_err());
            assert!(decode_knee_point(Payload::new(payload)).is_err());
        }
        for position in 0..4 {
            let mut payload = [0; 4];
            payload[position] = 0x10;
            assert!(decode_black_level(Payload::new(&payload)).is_err());
            assert!(decode_knee_slope(Payload::new(&payload)).is_err());
            assert!(decode_knee_point(Payload::new(&payload)).is_err());
        }
        assert!(decode_black_level(Payload::new(&[0, 7, 0xb, 0xd])).is_err());
        assert!(decode_knee_slope(Payload::new(&[0, 0, 0xc, 7])).is_err());
        assert!(decode_knee_point(Payload::new(&[0, 0, 2, 3])).is_err());
        for payload in [[1, 0, 0, 0], [0, 1, 0, 0]] {
            assert!(decode_knee_slope(Payload::new(&payload)).is_err());
            assert!(decode_knee_point(Payload::new(&payload)).is_err());
        }
    }

    #[test]
    fn knee_mode_permission_and_parameter_domains_are_closed() {
        for wire in 0..=u8::MAX {
            let mode = Fr7KneeMode::from_protocol_value(wire);
            assert_eq!(mode.is_ok(), matches!(wire, 0 | 4));
            if let Ok(mode) = mode {
                assert_eq!(mode.to_protocol_value(), wire);
                assert_eq!(decode_fr7_knee_mode(Payload::new(&[wire])).unwrap(), mode);
            } else {
                assert!(decode_fr7_knee_mode(Payload::new(&[wire])).is_err());
            }
            let permission = Fr7KneePermission::from_protocol_value(wire);
            assert_eq!(permission.is_ok(), wire <= 2);
            if let Ok(permission) = permission {
                assert_eq!(permission.to_protocol_value(), wire);
                assert_eq!(
                    decode_fr7_knee_permission(Payload::new(&[wire])).unwrap(),
                    permission
                );
            } else {
                assert!(decode_fr7_knee_permission(Payload::new(&[wire])).is_err());
            }
            let parameter = Fr7KneeParameter::from_protocol_value(wire);
            assert_eq!(parameter.is_ok(), wire <= 3);
            if let Ok(parameter) = parameter {
                assert_eq!(parameter.to_protocol_value(), wire);
            }
        }
        for payload in [&[][..], &[0, 0][..], &[0, 0, 0, 0][..]] {
            assert!(decode_fr7_knee_mode(Payload::new(payload)).is_err());
            assert!(decode_fr7_knee_permission(Payload::new(payload)).is_err());
        }
        assert_eq!(
            decode_fr7_knee_permission(Payload::new(&[2])).unwrap(),
            Fr7KneePermission::DisplayOnly,
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_checked_values_commands_and_closed_choices() {
        fn round_trip<T>(value: T)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
        }
        for value in [
            BlackLevel::MIN,
            BlackLevel::new(0).unwrap(),
            BlackLevel::MAX,
        ] {
            round_trip(value);
        }
        for value in [KneeSlope::MIN, KneeSlope::new(0).unwrap(), KneeSlope::MAX] {
            round_trip(value);
        }
        for value in [KneePoint::MIN, KneePoint::MAX] {
            round_trip(value);
        }
        for value in [BlackStep::MIN, BlackStep::MAX] {
            round_trip(value);
        }
        round_trip(Fr7MasterBlack::Direct(BlackLevel::MIN));
        round_trip(Fr7RedBlack::Up(BlackStep::MAX));
        round_trip(Fr7BlueBlack::Down(BlackStep::MIN));
        round_trip(Fr7DetailSetting::new(true));
        round_trip(Fr7KneeSetting::new(false));
        round_trip(Fr7SetKneeMode::new(Fr7KneeMode::Manual));
        round_trip(Fr7SetKneeSlope::new(KneeSlope::MIN));
        round_trip(Fr7SetKneePoint::new(KneePoint::MAX));
        for value in [
            Fr7KneePermission::Enabled,
            Fr7KneePermission::Disabled,
            Fr7KneePermission::DisplayOnly,
        ] {
            round_trip(value);
        }
        for value in [
            Fr7KneeParameter::Setting,
            Fr7KneeParameter::Mode,
            Fr7KneeParameter::Point,
            Fr7KneeParameter::Slope,
        ] {
            round_trip(value);
        }
        for invalid in ["-991", "991", "0.5", "null"] {
            assert!(serde_json::from_str::<BlackLevel>(invalid).is_err());
        }
        for invalid in ["-100", "100", "0.5"] {
            assert!(serde_json::from_str::<KneeSlope>(invalid).is_err());
        }
        for invalid in ["74", "110", "75.5"] {
            assert!(serde_json::from_str::<KneePoint>(invalid).is_err());
        }
        for invalid in ["0", "256"] {
            assert!(serde_json::from_str::<BlackStep>(invalid).is_err());
        }
        assert!(serde_json::from_str::<Fr7MasterBlack>(r#"{"direct":991}"#).is_err());
        assert!(serde_json::from_str::<Fr7RedBlack>(r#"{"up":0}"#).is_err());
        assert!(serde_json::from_str::<Fr7BlueBlack>(r#"{"down":256}"#).is_err());
        assert!(
            serde_json::from_str::<Fr7DetailSetting>(r#"{"enabled":true,"future":0}"#).is_err()
        );
        assert!(serde_json::from_str::<Fr7SetKneeSlope>(r#"{"slope":100}"#).is_err());
        assert!(serde_json::from_str::<Fr7SetKneePoint>(r#"{"point":110}"#).is_err());
        assert!(serde_json::from_str::<Fr7SetKneeMode>(r#"{"mode":"unknown"}"#).is_err());
        assert!(serde_json::from_str::<Fr7KneePermission>(r#""unknown""#).is_err());
        assert!(serde_json::from_str::<Fr7KneeParameter>("4").is_err());
    }
}
