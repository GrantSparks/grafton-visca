//! Sony FR7 white-balance controls from R7, software version 4.00, pp. 15 and 21.
//!
//! The FR7's Memory A gain, tint and Kelvin controls have their own packets
//! and domains. They do not use the shared VISCA red/blue gain registers.
//! Source: [Sony VISCA Command List v4.00](https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf).
//! `WhiteBalanceMode::Manual` selects Memory A; `Preset` selects preset white.

use crate::{
    command::{
        bytes::FrameWriter,
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

crate::__grafton_visca_newtype! {
    /// FR7 preset-white or Memory A color temperature, in Kelvin (R7 p. 15).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "2000", max = "15000", display_prefix = "White Kelvin")]
    pub struct WhiteKelvin(u16);
}

crate::__grafton_visca_newtype! {
    /// FR7 Memory A red or blue gain, in tenths of a displayed unit.
    ///
    /// `-990`, `0` and `990` represent -99.0, neutral and +99.0. R7 pp. 15
    /// and 21 encode them as `0000`, `03DE` and `07BC`, respectively.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "-990", max = "990", display_prefix = "White Balance Gain (tenths)")]
    pub struct WhiteBalanceGain(i16);
}

impl WhiteBalanceGain {
    /// Creates a gain from -99.0 through +99.0, rounding to the nearest tenth.
    ///
    /// Exact halfway values round away from zero. Nonfinite values and values
    /// outside the physical domain are refused before rounding.
    pub fn from_gain(gain: f64) -> Result<Self, Error> {
        if !(-99.0..=99.0).contains(&gain) {
            return Err(Error::invalid_parameter(
                "white_balance_gain",
                gain.to_string(),
                "gain must be finite and between -99.0 and +99.0",
            ));
        }
        Self::new((gain * 10.0).round() as i16)
    }

    /// Returns the displayed gain, from -99.0 through +99.0.
    #[must_use]
    pub fn to_gain(self) -> f64 {
        f64::from(self.value()) / 10.0
    }

    /// Decodes the biased `0000..=07BC` protocol value (R7 p. 21).
    pub const fn from_protocol_value(value: u16) -> Result<Self, Error> {
        if value > 1980 {
            return Err(Error::parameter_out_of_range(
                "white_balance_gain_wire",
                value as i32,
                0,
                1980,
            ));
        }
        Self::new(value as i16 - 990)
    }

    /// Returns the biased `0000..=07BC` protocol value (R7 p. 15).
    #[must_use]
    pub const fn to_protocol_value(self) -> u16 {
        (self.value() + 990) as u16
    }
}

crate::__grafton_visca_newtype! {
    /// FR7 Memory A tint or ATW offset, from -99 through +99.
    ///
    /// R7 pp. 15 and 21 encode -99, neutral and +99 as `00`, `63` and `C6`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "-99", max = "99", display_prefix = "Tint Offset")]
    pub struct TintOffset(i16);
}

impl TintOffset {
    /// Decodes a biased `00..=C6` protocol value.
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
    /// Number of FR7 preset-white, gain, tint or ATW offset steps (R7 p. 15).
    ///
    /// This is a relative adjustment count, not a direct setting value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "1", max = "255", display_prefix = "White Balance Step")]
    pub struct WhiteBalanceStep(u8);
}

crate::__grafton_visca_newtype! {
    /// Number of FR7 Memory A color-temperature steps (R7 p. 15).
    ///
    /// Memory A permits `01..=1E`; preset white permits the wider `01..=FF`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "1", max = "30", display_prefix = "Memory Color Temperature Step")]
    pub struct MemoryColorTemperatureStep(u8);
}

/// Perform the FR7's WB SET adjustment (R7 p. 15).
///
/// This is a plain white-balance setting, with no mechanical motion axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7WhiteBalanceSet;

impl WireEncode for Fr7WhiteBalanceSet {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x04, 0x10, 0x05])
            .finish()
    }
}

// R7 p. 15 uses two nibble bytes for every relative count, and four for
// every direct setting. A fixed selector in each prefix makes invalid bank
// and channel combinations unrepresentable.
macro_rules! white_balance_control {
    (
        $(#[$doc:meta])* $name:ident, $step:ty, $value:ty,
        up = $up:expr, down = $down:expr, direct = $direct:expr,
        wire = $wire:expr
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case", deny_unknown_fields))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub enum $name {
            /// Increase the setting by the given number of steps.
            Up($step),
            /// Decrease the setting by the given number of steps.
            Down($step),
            /// Set an exact validated value.
            Direct($value),
        }

        impl WireEncode for $name {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                let frame = FrameWriter::new(camera_id, buffer);
                match *self {
                    Self::Up(step) => frame.bytes(&$up).nibbles::<2>(step.value()),
                    Self::Down(step) => frame.bytes(&$down).nibbles::<2>(step.value()),
                    Self::Direct(value) => frame.bytes(&$direct).nibbles::<4>(($wire)(value)),
                }
                .finish()
            }
        }
    };
}

white_balance_control! {
    /// FR7 preset-white Kelvin adjustment (R7 p. 15), bank 1.
    Fr7PresetWhite, WhiteBalanceStep, WhiteKelvin,
    up = [0x01, 0x05, 0x03, 0x02, 0x01],
    down = [0x01, 0x05, 0x03, 0x03, 0x01],
    direct = [0x01, 0x05, 0x43, 0x01],
    wire = WhiteKelvin::value
}

white_balance_control! {
    /// FR7 Memory A color-temperature adjustment (R7 p. 15), bank 2.
    Fr7MemoryColorTemperature, MemoryColorTemperatureStep, WhiteKelvin,
    up = [0x01, 0x05, 0x03, 0x02, 0x02],
    down = [0x01, 0x05, 0x03, 0x03, 0x02],
    direct = [0x01, 0x05, 0x43, 0x02],
    wire = WhiteKelvin::value
}

white_balance_control! {
    /// FR7 Memory A red-gain adjustment (R7 p. 15), bank 2.
    Fr7MemoryRedGain, WhiteBalanceStep, WhiteBalanceGain,
    up = [0x01, 0x7E, 0x01, 0x63, 0x02, 0x02],
    down = [0x01, 0x7E, 0x01, 0x63, 0x03, 0x02],
    direct = [0x01, 0x7E, 0x04, 0x46, 0x02],
    wire = WhiteBalanceGain::to_protocol_value
}

white_balance_control! {
    /// FR7 Memory A blue-gain adjustment (R7 p. 15), bank 2.
    Fr7MemoryBlueGain, WhiteBalanceStep, WhiteBalanceGain,
    up = [0x01, 0x7E, 0x01, 0x64, 0x02, 0x02],
    down = [0x01, 0x7E, 0x01, 0x64, 0x03, 0x02],
    direct = [0x01, 0x7E, 0x04, 0x56, 0x02],
    wire = WhiteBalanceGain::to_protocol_value
}

white_balance_control! {
    /// FR7 Memory A tint adjustment (R7 p. 15), bank 2.
    Fr7MemoryTint, WhiteBalanceStep, TintOffset,
    up = [0x01, 0x05, 0x04, 0x02, 0x02],
    down = [0x01, 0x05, 0x04, 0x03, 0x02],
    direct = [0x01, 0x05, 0x44, 0x02],
    wire = |value: TintOffset| u16::from(value.to_protocol_value())
}

white_balance_control! {
    /// FR7 ATW color-temperature offset adjustment (R7 p. 15), bank 0.
    Fr7AtwColorTemperatureOffset, WhiteBalanceStep, TintOffset,
    up = [0x01, 0x05, 0x05, 0x02, 0x00],
    down = [0x01, 0x05, 0x05, 0x03, 0x00],
    direct = [0x01, 0x05, 0x45, 0x00],
    wire = |value: TintOffset| u16::from(value.to_protocol_value())
}

white_balance_control! {
    /// FR7 ATW tint offset adjustment (R7 p. 15), bank 0.
    Fr7AtwTintOffset, WhiteBalanceStep, TintOffset,
    up = [0x01, 0x05, 0x06, 0x02, 0x00],
    down = [0x01, 0x05, 0x06, 0x03, 0x00],
    direct = [0x01, 0x05, 0x46, 0x00],
    wire = |value: TintOffset| u16::from(value.to_protocol_value())
}

pub(crate) fn decode_white_kelvin(payload: Payload<'_>) -> Result<WhiteKelvin, Error> {
    let value = Nibbles::<4>::try_from(payload)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    WhiteKelvin::new(value)
}

pub(crate) fn decode_white_balance_gain(payload: Payload<'_>) -> Result<WhiteBalanceGain, Error> {
    let value = Nibbles::<4>::try_from(payload)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    WhiteBalanceGain::from_protocol_value(value)
}

pub(crate) fn decode_tint_offset(payload: Payload<'_>) -> Result<TintOffset, Error> {
    TintOffset::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)
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
    fn r7_page_15_kelvin_packets_preserve_distinct_bank_and_step_domains() {
        literal(&Fr7WhiteBalanceSet, &[0x81, 1, 4, 0x10, 5, 0xff]);
        literal(
            &Fr7PresetWhite::Up(WhiteBalanceStep::MAX),
            &[0x81, 1, 5, 3, 2, 1, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7PresetWhite::Down(WhiteBalanceStep::MIN),
            &[0x81, 1, 5, 3, 3, 1, 0, 1, 0xff],
        );
        literal(
            &Fr7PresetWhite::Direct(WhiteKelvin::MIN),
            &[0x81, 1, 5, 0x43, 1, 0, 7, 0xd, 0, 0xff],
        );
        literal(
            &Fr7MemoryColorTemperature::Up(MemoryColorTemperatureStep::MAX),
            &[0x81, 1, 5, 3, 2, 2, 1, 0xe, 0xff],
        );
        literal(
            &Fr7MemoryColorTemperature::Down(MemoryColorTemperatureStep::MIN),
            &[0x81, 1, 5, 3, 3, 2, 0, 1, 0xff],
        );
        literal(
            &Fr7MemoryColorTemperature::Direct(WhiteKelvin::MAX),
            &[0x81, 1, 5, 0x43, 2, 3, 0xa, 9, 8, 0xff],
        );
        assert!(WhiteBalanceStep::new(0).is_err());
        assert!(MemoryColorTemperatureStep::new(0).is_err());
        assert!(MemoryColorTemperatureStep::new(31).is_err());
        assert!(WhiteBalanceStep::new(31).is_ok());
    }

    #[test]
    fn r7_page_15_memory_gain_packets_use_the_vendor_registers() {
        literal(
            &Fr7MemoryRedGain::Up(WhiteBalanceStep::MAX),
            &[0x81, 1, 0x7e, 1, 0x63, 2, 2, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7MemoryRedGain::Down(WhiteBalanceStep::MIN),
            &[0x81, 1, 0x7e, 1, 0x63, 3, 2, 0, 1, 0xff],
        );
        literal(
            &Fr7MemoryRedGain::Direct(WhiteBalanceGain::MIN),
            &[0x81, 1, 0x7e, 4, 0x46, 2, 0, 0, 0, 0, 0xff],
        );
        literal(
            &Fr7MemoryRedGain::Direct(WhiteBalanceGain::new(0).unwrap()),
            &[0x81, 1, 0x7e, 4, 0x46, 2, 0, 3, 0xd, 0xe, 0xff],
        );
        literal(
            &Fr7MemoryBlueGain::Up(WhiteBalanceStep::MIN),
            &[0x81, 1, 0x7e, 1, 0x64, 2, 2, 0, 1, 0xff],
        );
        literal(
            &Fr7MemoryBlueGain::Down(WhiteBalanceStep::MAX),
            &[0x81, 1, 0x7e, 1, 0x64, 3, 2, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7MemoryBlueGain::Direct(WhiteBalanceGain::MAX),
            &[0x81, 1, 0x7e, 4, 0x56, 2, 0, 7, 0xb, 0xc, 0xff],
        );
    }

    #[test]
    fn r7_page_15_tint_and_atw_packets_preserve_fixed_bank_and_zero_padding() {
        literal(
            &Fr7MemoryTint::Up(WhiteBalanceStep::MIN),
            &[0x81, 1, 5, 4, 2, 2, 0, 1, 0xff],
        );
        literal(
            &Fr7MemoryTint::Down(WhiteBalanceStep::MAX),
            &[0x81, 1, 5, 4, 3, 2, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7MemoryTint::Direct(TintOffset::MIN),
            &[0x81, 1, 5, 0x44, 2, 0, 0, 0, 0, 0xff],
        );
        literal(
            &Fr7AtwColorTemperatureOffset::Up(WhiteBalanceStep::MAX),
            &[0x81, 1, 5, 5, 2, 0, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7AtwColorTemperatureOffset::Down(WhiteBalanceStep::MIN),
            &[0x81, 1, 5, 5, 3, 0, 0, 1, 0xff],
        );
        literal(
            &Fr7AtwColorTemperatureOffset::Direct(TintOffset::new(0).unwrap()),
            &[0x81, 1, 5, 0x45, 0, 0, 0, 6, 3, 0xff],
        );
        literal(
            &Fr7AtwTintOffset::Up(WhiteBalanceStep::MIN),
            &[0x81, 1, 5, 6, 2, 0, 0, 1, 0xff],
        );
        literal(
            &Fr7AtwTintOffset::Down(WhiteBalanceStep::MAX),
            &[0x81, 1, 5, 6, 3, 0, 0xf, 0xf, 0xff],
        );
        literal(
            &Fr7AtwTintOffset::Direct(TintOffset::MAX),
            &[0x81, 1, 5, 0x46, 0, 0, 0, 0xc, 6, 0xff],
        );
    }

    #[test]
    fn value_domains_round_trip_every_wire_value_without_aliases() {
        for wire in 2000..=15000 {
            assert_eq!(WhiteKelvin::new(wire).unwrap().value(), wire);
        }
        assert!(WhiteKelvin::new(1999).is_err());
        assert!(WhiteKelvin::new(15001).is_err());
        for wire in 0..=1980 {
            let value = WhiteBalanceGain::from_protocol_value(wire).unwrap();
            assert_eq!(value.to_protocol_value(), wire);
            assert_eq!(WhiteBalanceGain::from_gain(value.to_gain()).unwrap(), value);
        }
        assert!(WhiteBalanceGain::from_protocol_value(1981).is_err());
        assert!(WhiteBalanceGain::from_protocol_value(u16::MAX).is_err());
        assert!(WhiteBalanceGain::new(-991).is_err());
        assert!(WhiteBalanceGain::new(991).is_err());
        for wire in 0..=198 {
            let value = TintOffset::from_protocol_value(wire).unwrap();
            assert_eq!(value.to_protocol_value(), wire);
        }
        assert!(TintOffset::from_protocol_value(199).is_err());
        assert!(TintOffset::from_protocol_value(u8::MAX).is_err());
        assert!(TintOffset::new(-100).is_err());
        assert!(TintOffset::new(100).is_err());
    }

    #[test]
    fn fractional_gain_rounds_nearest_and_rejects_out_of_domain_before_rounding() {
        for (gain, tenths) in [
            (-99.0, -990),
            (-0.15, -2),
            (-0.05, -1),
            (-0.049, 0),
            (0.049, 0),
            (0.05, 1),
            (0.15, 2),
            (99.0, 990),
        ] {
            assert_eq!(WhiteBalanceGain::from_gain(gain).unwrap().value(), tenths);
        }
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -99.001, 99.001] {
            assert!(WhiteBalanceGain::from_gain(invalid).is_err());
        }
    }

    #[test]
    fn r7_page_21_reply_decoders_preserve_boundaries_and_refuse_bad_shapes() {
        for (payload, kelvin) in [([0, 7, 0xd, 0], 2000), ([3, 0xa, 9, 8], 15000)] {
            assert_eq!(
                decode_white_kelvin(Payload::new(&payload)).unwrap().value(),
                kelvin
            );
        }
        for (payload, tenths) in [
            ([0, 0, 0, 0], -990),
            ([0, 3, 0xd, 0xe], 0),
            ([0, 7, 0xb, 0xc], 990),
        ] {
            assert_eq!(
                decode_white_balance_gain(Payload::new(&payload))
                    .unwrap()
                    .value(),
                tenths
            );
        }
        for (payload, offset) in [([0, 0, 0, 0], -99), ([0, 0, 6, 3], 0), ([0, 0, 0xc, 6], 99)] {
            assert_eq!(
                decode_tint_offset(Payload::new(&payload)).unwrap().value(),
                offset
            );
        }
        for payload in [
            &[][..],
            &[0, 0, 0][..],
            &[0, 0, 0, 0, 0][..],
            &[0, 0, 0, 0x10][..],
        ] {
            assert!(decode_white_kelvin(Payload::new(payload)).is_err());
            assert!(decode_white_balance_gain(Payload::new(payload)).is_err());
            assert!(decode_tint_offset(Payload::new(payload)).is_err());
        }
        assert!(decode_white_kelvin(Payload::new(&[0, 7, 0xc, 0xf])).is_err());
        assert!(decode_white_kelvin(Payload::new(&[3, 0xa, 9, 9])).is_err());
        assert!(decode_white_balance_gain(Payload::new(&[0, 7, 0xb, 0xd])).is_err());
        assert!(decode_tint_offset(Payload::new(&[0, 0, 0xc, 7])).is_err());
        assert!(decode_tint_offset(Payload::new(&[0, 1, 0, 0])).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_uses_checked_domains_for_values_and_command_variants() {
        for value in [
            WhiteBalanceGain::MIN,
            WhiteBalanceGain::new(0).unwrap(),
            WhiteBalanceGain::MAX,
        ] {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(
                serde_json::from_str::<WhiteBalanceGain>(&json).unwrap(),
                value
            );
        }
        for invalid in ["-991", "991", "0.5", "null"] {
            assert!(serde_json::from_str::<WhiteBalanceGain>(invalid).is_err());
        }
        for invalid in ["1999", "15001"] {
            assert!(serde_json::from_str::<WhiteKelvin>(invalid).is_err());
        }
        for invalid in ["-100", "100"] {
            assert!(serde_json::from_str::<TintOffset>(invalid).is_err());
        }
        assert!(serde_json::from_str::<WhiteBalanceStep>("0").is_err());
        assert!(serde_json::from_str::<MemoryColorTemperatureStep>("31").is_err());
        let command = Fr7MemoryRedGain::Direct(WhiteBalanceGain::new(-77).unwrap());
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(
            serde_json::from_str::<Fr7MemoryRedGain>(&json).unwrap(),
            command
        );
        assert!(serde_json::from_str::<Fr7MemoryRedGain>(r#"{"direct":991}"#).is_err());
        assert!(serde_json::from_str::<Fr7MemoryColorTemperature>(r#"{"up":31}"#).is_err());
    }
}
