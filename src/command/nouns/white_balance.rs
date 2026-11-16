//! White-balance requests and typed readback.

use crate::command::nouns::{forward_inquiry, forward_request};
use crate::{
    command::{color, inquiry_structs, sony_fr7_white_balance, white_balance_wire},
    types, Request, TargetRegistration,
};

/// Read mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeInquiry;
forward_inquiry!(ModeInquiry, inquiry_structs::WhiteBalanceModeInquiry; inquiry_structs::WhiteBalanceModeInquiry);

/// Request set sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSensitivity {
    sensitivity: types::AutoWhiteBalanceSensitivity,
}
impl SetSensitivity {
    /// Creates this request from checked domain values.
    pub const fn new(sensitivity: types::AutoWhiteBalanceSensitivity) -> Self {
        Self { sensitivity }
    }

    /// Returns the requested sensitivity.
    #[must_use]
    pub const fn sensitivity(&self) -> types::AutoWhiteBalanceSensitivity {
        self.sensitivity
    }
}
forward_request!(SetSensitivity, white_balance_wire::AWBSensitivityCommand, Plain, rows = [AutoWhiteBalanceSensitivity]; |request| white_balance_wire::AWBSensitivityCommand::new(request.sensitivity));

/// Read sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensitivityInquiry;
forward_inquiry!(SensitivityInquiry, inquiry_structs::AutoWhiteBalanceSensitivityInquiry; inquiry_structs::AutoWhiteBalanceSensitivityInquiry);

/// Request set red tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRedTuning {
    level: types::RgbTuning,
}
impl SetRedTuning {
    /// Creates this request from checked domain values.
    pub const fn new(level: types::RgbTuning) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(&self) -> types::RgbTuning {
        self.level
    }
}
forward_request!(SetRedTuning, color::RedTuningCommand, Plain, rows = [RedTuning]; |request| color::RedTuningCommand::new(request.level));

/// Request set blue tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBlueTuning {
    level: types::RgbTuning,
}
impl SetBlueTuning {
    /// Creates this request from checked domain values.
    pub const fn new(level: types::RgbTuning) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(&self) -> types::RgbTuning {
        self.level
    }
}
forward_request!(SetBlueTuning, color::BlueTuningCommand, Plain, rows = [BlueTuning]; |request| color::BlueTuningCommand::new(request.level));

/// Read color temperature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTemperatureInquiry;
forward_inquiry!(ColorTemperatureInquiry, inquiry_structs::ColorTemperatureInquiry; inquiry_structs::ColorTemperatureInquiry);

/// Request color temperature reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTemperatureReset;
forward_request!(ColorTemperatureReset, color::ColorTemperature, Plain, rows = [ColorTemperatureReset]; color::ColorTemperature::Reset);

/// Request color temperature up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTemperatureUp;
forward_request!(ColorTemperatureUp, color::ColorTemperature, Plain, rows = [ColorTemperatureUp]; color::ColorTemperature::Up);

/// Request color temperature down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTemperatureDown;
forward_request!(ColorTemperatureDown, color::ColorTemperature, Plain, rows = [ColorTemperatureDown]; color::ColorTemperature::Down);

/// Request set color temperature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetColorTemperature {
    temperature: types::Kelvin,
}
impl SetColorTemperature {
    /// Checks this physical temperature against the selected target.
    pub fn for_target(
        target: &TargetRegistration,
        temperature: types::Kelvin,
    ) -> crate::Result<Self> {
        let request = Self { temperature };
        request.validate_for_target(target)?;
        Ok(request)
    }

    /// Returns the requested temperature.
    #[must_use]
    pub const fn temperature(&self) -> types::Kelvin {
        self.temperature
    }
}
forward_request!(SetColorTemperature, color::ColorTemperature, Plain, rows = [ColorTemperatureDirect]; |request| color::ColorTemperature::SetTemperature(request.temperature));

/// Read red gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedGainInquiry;
forward_inquiry!(RedGainInquiry, inquiry_structs::RedGainInquiry; inquiry_structs::RedGainInquiry);

/// Request red gain reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedGainReset;
forward_request!(RedGainReset, color::RedGain, Plain, rows = [RedGainReset]; color::RedGain::Reset);

/// Request red gain up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedGainUp;
forward_request!(RedGainUp, color::RedGain, Plain, rows = [RedGainUp]; color::RedGain::Up);

/// Request red gain down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedGainDown;
forward_request!(RedGainDown, color::RedGain, Plain, rows = [RedGainDown]; color::RedGain::Down);

/// Request set red gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRedGain {
    gain: types::RgbGain,
}
impl SetRedGain {
    /// Creates this request from checked domain values.
    pub const fn new(gain: types::RgbGain) -> Self {
        Self { gain }
    }

    /// Returns the requested gain.
    #[must_use]
    pub const fn gain(&self) -> types::RgbGain {
        self.gain
    }
}
forward_request!(SetRedGain, color::RedGain, Plain, rows = [RedGainDirect]; |request| color::RedGain::SetValue(request.gain));

/// Read blue gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueGainInquiry;
forward_inquiry!(BlueGainInquiry, inquiry_structs::BlueGainInquiry; inquiry_structs::BlueGainInquiry);

/// Request blue gain reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueGainReset;
forward_request!(BlueGainReset, color::BlueGain, Plain, rows = [BlueGainReset]; color::BlueGain::Reset);

/// Request blue gain up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueGainUp;
forward_request!(BlueGainUp, color::BlueGain, Plain, rows = [BlueGainUp]; color::BlueGain::Up);

/// Request blue gain down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueGainDown;
forward_request!(BlueGainDown, color::BlueGain, Plain, rows = [BlueGainDown]; color::BlueGain::Down);

/// Request set blue gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBlueGain {
    gain: types::RgbGain,
}
impl SetBlueGain {
    /// Creates this request from checked domain values.
    pub const fn new(gain: types::RgbGain) -> Self {
        Self { gain }
    }

    /// Returns the requested gain.
    #[must_use]
    pub const fn gain(&self) -> types::RgbGain {
        self.gain
    }
}
forward_request!(SetBlueGain, color::BlueGain, Plain, rows = [BlueGainDirect]; |request| color::BlueGain::SetValue(request.gain));

/// Read red tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedTuningInquiry;
forward_inquiry!(RedTuningInquiry, inquiry_structs::RedTuningInquiry; inquiry_structs::RedTuningInquiry);

/// Read blue tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueTuningInquiry;
forward_inquiry!(BlueTuningInquiry, inquiry_structs::BlueTuningInquiry; inquiry_structs::BlueTuningInquiry);

/// Raises the preset white-balance color temperature by `steps` protocol adjustments (`1..=255`), with a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetWhiteUpBy {
    steps: types::WhiteBalanceStep,
}
impl PresetWhiteUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(PresetWhiteUpBy, sony_fr7_white_balance::Fr7PresetWhite, Plain, rows = [Fr7PresetWhiteUp]; |request| sony_fr7_white_balance::Fr7PresetWhite::Up(request.steps));

/// Lowers the preset white-balance color temperature by `steps` protocol adjustments (`1..=255`), with a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetWhiteDownBy {
    steps: types::WhiteBalanceStep,
}
impl PresetWhiteDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(PresetWhiteDownBy, sony_fr7_white_balance::Fr7PresetWhite, Plain, rows = [Fr7PresetWhiteDown]; |request| sony_fr7_white_balance::Fr7PresetWhite::Down(request.steps));

/// Sets the preset white-balance color temperature in Kelvin over `2000..=15000 K` (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetPresetWhite {
    temperature: types::Kelvin,
}
impl SetPresetWhite {
    /// Creates this request from checked domain values.
    pub const fn new(temperature: types::Kelvin) -> Self {
        Self { temperature }
    }

    /// Returns the requested temperature.
    #[must_use]
    pub const fn temperature(&self) -> types::Kelvin {
        self.temperature
    }
}
forward_request!(SetPresetWhite, sony_fr7_white_balance::Fr7PresetWhite, Plain, rows = [Fr7PresetWhiteDirect]; |request| sony_fr7_white_balance::Fr7PresetWhite::Direct(request.temperature));

/// Reads the preset white-balance color temperature in Kelvin over `2000..=15000 K` (R7 pp. 15 and 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetWhiteInquiry;
forward_inquiry!(PresetWhiteInquiry, inquiry_structs::Fr7PresetWhiteInquiry; inquiry_structs::Fr7PresetWhiteInquiry);

/// Raises the Memory A white-balance color temperature by `steps` protocol adjustments (`1..=30`), with a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryColorTemperatureUpBy {
    steps: types::MemoryColorTemperatureStep,
}
impl MemoryColorTemperatureUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::MemoryColorTemperatureStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::MemoryColorTemperatureStep {
        self.steps
    }
}
forward_request!(MemoryColorTemperatureUpBy, sony_fr7_white_balance::Fr7MemoryColorTemperature, Plain, rows = [Fr7MemoryColorTemperatureUp]; |request| sony_fr7_white_balance::Fr7MemoryColorTemperature::Up(request.steps));

/// Lowers the Memory A white-balance color temperature by `steps` protocol adjustments (`1..=30`), with a documented resulting range of 2000–15000 K. The count is not a Kelvin delta (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryColorTemperatureDownBy {
    steps: types::MemoryColorTemperatureStep,
}
impl MemoryColorTemperatureDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::MemoryColorTemperatureStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::MemoryColorTemperatureStep {
        self.steps
    }
}
forward_request!(MemoryColorTemperatureDownBy, sony_fr7_white_balance::Fr7MemoryColorTemperature, Plain, rows = [Fr7MemoryColorTemperatureDown]; |request| sony_fr7_white_balance::Fr7MemoryColorTemperature::Down(request.steps));

/// Sets the Memory A white-balance color temperature in Kelvin over `2000..=15000 K` (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMemoryColorTemperature {
    temperature: types::Kelvin,
}
impl SetMemoryColorTemperature {
    /// Creates this request from checked domain values.
    pub const fn new(temperature: types::Kelvin) -> Self {
        Self { temperature }
    }

    /// Returns the requested temperature.
    #[must_use]
    pub const fn temperature(&self) -> types::Kelvin {
        self.temperature
    }
}
forward_request!(SetMemoryColorTemperature, sony_fr7_white_balance::Fr7MemoryColorTemperature, Plain, rows = [Fr7MemoryColorTemperatureDirect]; |request| sony_fr7_white_balance::Fr7MemoryColorTemperature::Direct(request.temperature));

/// Reads the Memory A white-balance color temperature in Kelvin over `2000..=15000 K` (R7 pp. 15 and 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryColorTemperatureInquiry;
forward_inquiry!(MemoryColorTemperatureInquiry, inquiry_structs::Fr7MemoryColorTemperatureInquiry; inquiry_structs::Fr7MemoryColorTemperatureInquiry);

/// Raises the Memory A red-channel white-balance gain by `1..=255` relative adjustment steps. Direct values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRedGainUpBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryRedGainUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryRedGainUpBy, sony_fr7_white_balance::Fr7MemoryRedGain, Plain, rows = [Fr7MemoryRedGainUp]; |request| sony_fr7_white_balance::Fr7MemoryRedGain::Up(request.steps));

/// Lowers the Memory A red-channel white-balance gain by `1..=255` relative adjustment steps. Direct values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRedGainDownBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryRedGainDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryRedGainDownBy, sony_fr7_white_balance::Fr7MemoryRedGain, Plain, rows = [Fr7MemoryRedGainDown]; |request| sony_fr7_white_balance::Fr7MemoryRedGain::Down(request.steps));

/// Sets the Memory A red-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents −99.0 to +99.0 displayed units, with zero neutral (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMemoryRedGain {
    value: types::WhiteBalanceGain,
}
impl SetMemoryRedGain {
    /// Creates this request from checked domain values.
    pub const fn new(value: types::WhiteBalanceGain) -> Self {
        Self { value }
    }

    /// Returns the requested value.
    #[must_use]
    pub const fn value(&self) -> types::WhiteBalanceGain {
        self.value
    }
}
forward_request!(SetMemoryRedGain, sony_fr7_white_balance::Fr7MemoryRedGain, Plain, rows = [Fr7MemoryRedGainDirect]; |request| sony_fr7_white_balance::Fr7MemoryRedGain::Direct(request.value));

/// Reads the Memory A red-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents −99.0 to +99.0 displayed units, with zero neutral (R7 pp. 15 and 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRedGainInquiry;
forward_inquiry!(MemoryRedGainInquiry, inquiry_structs::Fr7MemoryRedGainInquiry; inquiry_structs::Fr7MemoryRedGainInquiry);

/// Raises the Memory A blue-channel white-balance gain by `1..=255` relative adjustment steps. Direct values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBlueGainUpBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryBlueGainUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryBlueGainUpBy, sony_fr7_white_balance::Fr7MemoryBlueGain, Plain, rows = [Fr7MemoryBlueGainUp]; |request| sony_fr7_white_balance::Fr7MemoryBlueGain::Up(request.steps));

/// Lowers the Memory A blue-channel white-balance gain by `1..=255` relative adjustment steps. Direct values span −99.0 to +99.0 displayed units; the count is not an absolute gain (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBlueGainDownBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryBlueGainDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryBlueGainDownBy, sony_fr7_white_balance::Fr7MemoryBlueGain, Plain, rows = [Fr7MemoryBlueGainDown]; |request| sony_fr7_white_balance::Fr7MemoryBlueGain::Down(request.steps));

/// Sets the Memory A blue-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents −99.0 to +99.0 displayed units, with zero neutral (R7 p. 15). Changing gain can also change the related Memory A temperature/tint readback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMemoryBlueGain {
    value: types::WhiteBalanceGain,
}
impl SetMemoryBlueGain {
    /// Creates this request from checked domain values.
    pub const fn new(value: types::WhiteBalanceGain) -> Self {
        Self { value }
    }

    /// Returns the requested value.
    #[must_use]
    pub const fn value(&self) -> types::WhiteBalanceGain {
        self.value
    }
}
forward_request!(SetMemoryBlueGain, sony_fr7_white_balance::Fr7MemoryBlueGain, Plain, rows = [Fr7MemoryBlueGainDirect]; |request| sony_fr7_white_balance::Fr7MemoryBlueGain::Direct(request.value));

/// Reads the Memory A blue-channel white-balance gain as `WhiteBalanceGain`: `-990..=990` represents −99.0 to +99.0 displayed units, with zero neutral (R7 pp. 15 and 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBlueGainInquiry;
forward_inquiry!(MemoryBlueGainInquiry, inquiry_structs::Fr7MemoryBlueGainInquiry; inquiry_structs::Fr7MemoryBlueGainInquiry);

/// Raises the Memory A white-balance tint by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryTintUpBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryTintUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryTintUpBy, sony_fr7_white_balance::Fr7MemoryTint, Plain, rows = [Fr7MemoryTintUp]; |request| sony_fr7_white_balance::Fr7MemoryTint::Up(request.steps));

/// Lowers the Memory A white-balance tint by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryTintDownBy {
    steps: types::WhiteBalanceStep,
}
impl MemoryTintDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(MemoryTintDownBy, sony_fr7_white_balance::Fr7MemoryTint, Plain, rows = [Fr7MemoryTintDown]; |request| sony_fr7_white_balance::Fr7MemoryTint::Down(request.steps));

/// Sets the Memory A white-balance tint as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMemoryTint {
    offset: types::WhiteBalanceOffset,
}
impl SetMemoryTint {
    /// Creates this request from checked domain values.
    pub const fn new(offset: types::WhiteBalanceOffset) -> Self {
        Self { offset }
    }

    /// Returns the requested offset.
    #[must_use]
    pub const fn offset(&self) -> types::WhiteBalanceOffset {
        self.offset
    }
}
forward_request!(SetMemoryTint, sony_fr7_white_balance::Fr7MemoryTint, Plain, rows = [Fr7MemoryTintDirect]; |request| sony_fr7_white_balance::Fr7MemoryTint::Direct(request.offset));

/// Reads the Memory A white-balance tint as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 pp. 15 and 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryTintInquiry;
forward_inquiry!(MemoryTintInquiry, inquiry_structs::Fr7MemoryTintInquiry; inquiry_structs::Fr7MemoryTintInquiry);

/// Raises the ATW color-temperature offset by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwColorTemperatureOffsetUpBy {
    steps: types::WhiteBalanceStep,
}
impl AtwColorTemperatureOffsetUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(AtwColorTemperatureOffsetUpBy, sony_fr7_white_balance::Fr7AtwColorTemperatureOffset, Plain, rows = [Fr7AtwColorTemperatureOffsetUp]; |request| sony_fr7_white_balance::Fr7AtwColorTemperatureOffset::Up(request.steps));

/// Lowers the ATW color-temperature offset by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwColorTemperatureOffsetDownBy {
    steps: types::WhiteBalanceStep,
}
impl AtwColorTemperatureOffsetDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(AtwColorTemperatureOffsetDownBy, sony_fr7_white_balance::Fr7AtwColorTemperatureOffset, Plain, rows = [Fr7AtwColorTemperatureOffsetDown]; |request| sony_fr7_white_balance::Fr7AtwColorTemperatureOffset::Down(request.steps));

/// Sets the ATW color-temperature offset as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAtwColorTemperatureOffset {
    offset: types::WhiteBalanceOffset,
}
impl SetAtwColorTemperatureOffset {
    /// Creates this request from checked domain values.
    pub const fn new(offset: types::WhiteBalanceOffset) -> Self {
        Self { offset }
    }

    /// Returns the requested offset.
    #[must_use]
    pub const fn offset(&self) -> types::WhiteBalanceOffset {
        self.offset
    }
}
forward_request!(SetAtwColorTemperatureOffset, sony_fr7_white_balance::Fr7AtwColorTemperatureOffset, Plain, rows = [Fr7AtwColorTemperatureOffsetDirect]; |request| sony_fr7_white_balance::Fr7AtwColorTemperatureOffset::Direct(request.offset));

/// Reads the ATW color-temperature offset as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 pp. 15 and 21). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwColorTemperatureOffsetInquiry;
forward_inquiry!(AtwColorTemperatureOffsetInquiry, inquiry_structs::Fr7AtwColorTemperatureOffsetInquiry; inquiry_structs::Fr7AtwColorTemperatureOffsetInquiry);

/// Raises the ATW tint offset by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwTintOffsetUpBy {
    steps: types::WhiteBalanceStep,
}
impl AtwTintOffsetUpBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(AtwTintOffsetUpBy, sony_fr7_white_balance::Fr7AtwTintOffset, Plain, rows = [Fr7AtwTintOffsetUp]; |request| sony_fr7_white_balance::Fr7AtwTintOffset::Up(request.steps));

/// Lowers the ATW tint offset by `1..=255` relative adjustment steps within the documented −99 to +99 offset domain. These are offset levels, not Kelvin (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwTintOffsetDownBy {
    steps: types::WhiteBalanceStep,
}
impl AtwTintOffsetDownBy {
    /// Creates this request from checked domain values.
    pub const fn new(steps: types::WhiteBalanceStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(&self) -> types::WhiteBalanceStep {
        self.steps
    }
}
forward_request!(AtwTintOffsetDownBy, sony_fr7_white_balance::Fr7AtwTintOffset, Plain, rows = [Fr7AtwTintOffsetDown]; |request| sony_fr7_white_balance::Fr7AtwTintOffset::Down(request.steps));

/// Sets the ATW tint offset as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 p. 15). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAtwTintOffset {
    offset: types::WhiteBalanceOffset,
}
impl SetAtwTintOffset {
    /// Creates this request from checked domain values.
    pub const fn new(offset: types::WhiteBalanceOffset) -> Self {
        Self { offset }
    }

    /// Returns the requested offset.
    #[must_use]
    pub const fn offset(&self) -> types::WhiteBalanceOffset {
        self.offset
    }
}
forward_request!(SetAtwTintOffset, sony_fr7_white_balance::Fr7AtwTintOffset, Plain, rows = [Fr7AtwTintOffsetDirect]; |request| sony_fr7_white_balance::Fr7AtwTintOffset::Direct(request.offset));

/// Reads the ATW tint offset as a signed offset level in `-99..=99`, with zero neutral. The offset is dimensionless rather than a Kelvin value (R7 pp. 15 and 21). The camera applies this offset only when its `Offset White<ATW>` setting permits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtwTintOffsetInquiry;
forward_inquiry!(AtwTintOffsetInquiry, inquiry_structs::Fr7AtwTintOffsetInquiry; inquiry_structs::Fr7AtwTintOffsetInquiry);

/// Request set mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMode {
    mode: types::WhiteBalanceMode,
}
impl SetMode {
    /// Creates this request from checked domain values.
    pub const fn new(mode: types::WhiteBalanceMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(&self) -> types::WhiteBalanceMode {
        self.mode
    }
}
forward_request!(SetMode, white_balance_wire::WhiteBalanceCommand, Plain; |request| white_balance_wire::WhiteBalanceCommand::new(request.mode));

/// Capture white balance from the current scene without changing its mode.
///
/// Standard VISCA uses one-push white-balance mode. Sony FR7 performs WB SET
/// in Memory A or ATW; preset-mode capture is not established by its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnePush;

impl OnePush {
    fn encoding_for_target(
        target: &TargetRegistration,
    ) -> crate::Result<crate::capabilities::OnePushWhiteBalanceEncoding> {
        target.profile().encodings().one_push_white_balance().ok_or(
            crate::Error::FeatureNotSupported {
                feature: "one-push white balance encoding",
            },
        )
    }
}

forward_request!(OnePush, color::OnePushTriggerCommand, Plain;
    select target |request, target| (Self::encoding_for_target(target)?)
        default (crate::capabilities::OnePushWhiteBalanceEncoding::Standard) {
        crate::capabilities::OnePushWhiteBalanceEncoding::Standard => (
            color::OnePushTriggerCommand, OnePushWhiteBalanceTrigger,
            color::OnePushTriggerCommand::new()
        ),
        crate::capabilities::OnePushWhiteBalanceEncoding::SonyFr7 => (
            sony_fr7_white_balance::Fr7WhiteBalanceSet, Fr7WhiteBalanceSet,
            sony_fr7_white_balance::Fr7WhiteBalanceSet
        ),
    }
);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        command::semantics::{BuiltinCommand, BuiltinRequestContract},
        CameraId, ProfileSpec,
    };

    fn target<P: crate::CompileTimeProfile>() -> TargetRegistration {
        TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<P>().expect("profile"),
        )
        .expect("target")
    }

    #[test]
    fn one_push_preserves_contextual_evidence_and_identical_frames() {
        let ptz = target::<crate::profiles::PtzOpticsG2>();
        let fr7 = target::<crate::profiles::SonyFr7>();
        assert_eq!(
            OnePush
                .selected_ledger_row_for_target(&ptz)
                .expect("standard row"),
            BuiltinCommand::OnePushWhiteBalanceTrigger
        );
        assert_eq!(
            OnePush
                .selected_ledger_row_for_target(&fr7)
                .expect("FR7 row"),
            BuiltinCommand::Fr7WhiteBalanceSet
        );
        for target in [&ptz, &fr7] {
            OnePush
                .validate_for_target(target)
                .expect("selected supported contract");
            let mut bytes = [0; 6];
            let len = OnePush
                .write_for_target(
                    target,
                    &mut bytes,
                    crate::requests::RequestContractAuthority::new(),
                )
                .expect("target frame");
            assert_eq!(&bytes[..len], &[0x81, 0x01, 0x04, 0x10, 0x05, 0xFF]);
        }
        assert_eq!(<OnePush as BuiltinRequestContract>::LEDGER_ROWS.len(), 2);
    }

    #[test]
    fn gain_requests_declare_only_their_exact_packet_rows() {
        assert_eq!(
            <SetRedGain as BuiltinRequestContract>::LEDGER_ROWS,
            &[BuiltinCommand::RedGainDirect]
        );
        assert_eq!(
            <RedGainReset as BuiltinRequestContract>::LEDGER_ROWS,
            &[BuiltinCommand::RedGainReset]
        );
        let request = SetMemoryTint::new(types::WhiteBalanceOffset::new(-99).expect("tint"));
        assert_eq!(request.offset().value(), -99);
        assert_eq!(
            request.selected_ledger_row(),
            BuiltinCommand::Fr7MemoryTintDirect
        );
        let mut bytes = [0; SetMemoryTint::MAX_SIZE];
        let len = request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("biased tint frame");
        assert_eq!(
            &bytes[..len],
            &[0x81, 0x01, 0x05, 0x44, 0x02, 0, 0, 0, 0, 0xFF]
        );
    }

    #[test]
    fn physical_temperature_requires_a_documented_target_step() {
        let ptz = target::<crate::profiles::PtzOpticsG2>();
        let temperature = types::Kelvin::new(3200).expect("Kelvin");
        let request = SetColorTemperature::for_target(&ptz, temperature).expect("documented step");
        assert_eq!(request.temperature(), temperature);
        let mut bytes = [0; SetColorTemperature::MAX_SIZE];
        let len = request
            .write_for_target(
                &ptz,
                &mut bytes,
                crate::requests::RequestContractAuthority::new(),
            )
            .expect("lowered target frame");
        assert_eq!(&bytes[..len], &[0x81, 0x01, 0x04, 0x20, 0x00, 0x07, 0xFF]);
        assert!(SetColorTemperature::for_target(
            &ptz,
            types::Kelvin::new(3201).expect("physical Kelvin")
        )
        .is_err());
    }
}
