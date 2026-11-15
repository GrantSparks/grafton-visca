//! Exhaustive built-in encoder snapshot for issue #715.
//!
//! The fixture is the reviewed current wire inventory: one row per generated
//! case, covering every built-in command encoder, every generated inquiry,
//! camera-address variants and registered profile-specific preparation. Each
//! row was reviewed against the source register in `docs/visca_reference.md`
//! when it entered the fixture,
//! including the source-backed 2.0 corrections ratified in D5 (#808, #819),
//! the three 2D-NR mode rows, and the `FocusZone[Zone03]` row
//! (`81 01 04 AA 03 FF`, PTZOptics G2 bench, 2026-10-04, #795). The
//! semantic-ledger assertion below makes a newly added built-in command fail
//! until it is deliberately represented by this generator and reviewed in the
//! fixture; generated inquiry metadata is checked the same way.
//!
//! The fixture is literal text and every change to it is reviewed as a diff.
//! After a deliberate encoder change, regenerate it from [`rows`] with
//! `GRAFTON_VISCA_BLESS_WIRE_GOLDEN=1 cargo test --lib wire_ledger_matches_literal_golden_inventory`.
//! The bless run writes the fixture and then fails on purpose, so it can never
//! report green; it is refused when `CI` is set. Review the diff and rerun
//! without the variable.

#![allow(clippy::redundant_closure_call)]

use std::collections::HashSet;

use crate::command::inquiry_structs::BUILTIN_INQUIRIES;
use crate::command::preset::{PresetCommand, PresetRecallSpeedCommand};
use crate::command::semantics::BuiltinCommand;
use crate::command::system::{AddressSetCommand, CommandCancelCommand, InterfaceClearCommand};
use crate::command::*;
use crate::types::*;
use crate::{CameraId, Error, ViscaSocket};

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn encode<C: Enc>(c: &C, id: CameraId) -> String {
    let mut buf = [0u8; 64];
    match c.write_into(id, &mut buf) {
        Ok(n) => hex(&buf[..n]),
        Err(e) => format!("ERR({e:?})"),
    }
}

macro_rules! row {
    ($out:ident, $name:expr, $cmd:expr) => {{
        let c = $cmd;
        $out.push(format!("{}\t{}", $name, encode(&c, CameraId::CAMERA_1)));
    }};
    ($out:ident, $name:expr, $cmd:expr, $id:expr) => {{
        let c = $cmd;
        $out.push(format!("{}\t{}", $name, encode(&c, $id)));
    }};
}

macro_rules! rowr {
    ($out:ident, $name:expr, $cmd:expr) => {{
        let r: Result<_, Error> = (|| $cmd)();
        match r {
            Ok(c) => $out.push(format!("{}\t{}", $name, encode(&c, CameraId::CAMERA_1))),
            Err(e) => $out.push(format!("{}\tCTOR_ERR({e:?})", $name)),
        }
    }};
}

macro_rules! cover {
    ($covered:ident, $($command:ident),+ $(,)?) => {{
        $(
            assert!(
                $covered.insert(BuiltinCommand::$command),
                "duplicate wire-inventory coverage for {:?}",
                BuiltinCommand::$command,
            );
        )+
    }};
}

fn ps(v: u8) -> Result<PanSpeed, Error> {
    PanSpeed::new(v)
}
fn ts(v: u8) -> Result<TiltSpeed, Error> {
    TiltSpeed::new(v)
}

pub(crate) fn rows() -> Vec<String> {
    let mut o: Vec<String> = Vec::new();
    let mut covered = HashSet::new();
    // power
    cover!(covered, PowerOn, PowerStandby);
    row!(o, "PowerOn", PowerOn::new());
    row!(o, "PowerStandby", PowerStandby::new());
    row!(o, "PowerOn@cam7", PowerOn::new(), CameraId::CAMERA_7);
    // pan/tilt
    cover!(
        covered,
        PanTiltHome,
        PanTiltReset,
        PanTiltDrive,
        PanTiltStop,
        PanTiltAbsolute,
        PanTiltRelative,
        PanTiltLimitSet,
        PanTiltLimitClear,
    );
    row!(o, "PanTilt::Home", PanTilt::Home);
    row!(o, "PanTilt::Reset", PanTilt::Reset);
    for (dn, d) in [
        ("Up", PanTiltDirection::Up),
        ("Down", PanTiltDirection::Down),
        ("Left", PanTiltDirection::Left),
        ("Right", PanTiltDirection::Right),
        ("UpLeft", PanTiltDirection::UpLeft),
        ("UpRight", PanTiltDirection::UpRight),
        ("DownLeft", PanTiltDirection::DownLeft),
        ("DownRight", PanTiltDirection::DownRight),
        ("Stop", PanTiltDirection::Stop),
    ] {
        rowr!(
            o,
            format!("PanTilt::Move[{dn},18,14]"),
            Ok(PanTilt::Move {
                direction: d,
                pan_speed: ps(0x18)?,
                tilt_speed: ts(0x14)?
            })
        );
    }
    for (p, t) in [(0u8, 0u8), (1, 1), (0x18, 0x18), (0x19, 0x14), (0x18, 0x15)] {
        rowr!(
            o,
            format!("PanTilt::Move[Up,{p:02X},{t:02X}]"),
            Ok(PanTilt::Move {
                direction: PanTiltDirection::Up,
                pan_speed: ps(p)?,
                tilt_speed: ts(t)?
            })
        );
    }
    // The profile-less frame carries any 16-bit word per axis.
    for (p, t) in [
        (144i16, -72i16),
        (0, 0),
        (-1, -1),
        (-1, 0),
        (2448, 1296),
        (-2448, -432),
        (0x1234, 0xABCD_u16 as i16),
        (i16::MAX, i16::MIN),
        (i16::MIN, i16::MAX),
    ] {
        rowr!(
            o,
            format!("PanTilt::AbsolutePosition[{p},{t}]"),
            Ok(PanTilt::AbsolutePosition {
                pan: p,
                tilt: t,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        rowr!(
            o,
            format!("PanTilt::RelativePosition[{p},{t}]"),
            Ok(PanTilt::RelativePosition {
                pan: p,
                tilt: t,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        row!(
            o,
            format!("PanTilt::LimitSet[DownLeft,{p},{t}]"),
            PanTilt::LimitSet {
                corner: PanTiltLimitCorner::DownLeft,
                pan: p,
                tilt: t
            }
        );
        row!(
            o,
            format!("PanTilt::LimitSet[UpRight,{p},{t}]"),
            PanTilt::LimitSet {
                corner: PanTiltLimitCorner::UpRight,
                pan: p,
                tilt: t
            }
        );
    }
    row!(
        o,
        "PanTilt::LimitClear[DownLeft]",
        PanTilt::LimitClear {
            corner: PanTiltLimitCorner::DownLeft
        }
    );
    row!(
        o,
        "PanTilt::LimitClear[UpRight]",
        PanTilt::LimitClear {
            corner: PanTiltLimitCorner::UpRight
        }
    );
    // zoom
    cover!(
        covered,
        ZoomStop,
        ZoomTele,
        ZoomWide,
        ZoomTeleVariable,
        ZoomWideVariable,
        ZoomPosition,
        DigitalZoom,
    );
    row!(o, "Zoom::Stop", Zoom::Stop);
    row!(o, "Zoom::TeleStd", Zoom::TeleStd);
    row!(o, "Zoom::WideStd", Zoom::WideStd);
    for s in 0u8..=8 {
        rowr!(
            o,
            format!("Zoom::TeleVariable[{s}]"),
            Ok(Zoom::TeleVariable(ZoomSpeed::new(s)?))
        );
        rowr!(
            o,
            format!("Zoom::WideVariable[{s}]"),
            Ok(Zoom::WideVariable(ZoomSpeed::new(s)?))
        );
    }
    for p in [0u16, 0x1234, 0x4000, 0x7AC0, 0xFFFF] {
        rowr!(
            o,
            format!("Zoom::Position[{p:04X}]"),
            Ok(Zoom::Position(ZoomPosition::new(p)?))
        );
    }
    row!(o, "DigitalZoom[on]", DigitalZoom::new(true));
    row!(o, "DigitalZoom[off]", DigitalZoom::new(false));
    // focus
    cover!(
        covered,
        FocusStop,
        FocusFar,
        FocusNear,
        FocusFarVariable,
        FocusNearVariable,
        FocusPosition,
        FocusAuto,
        FocusManual,
        FocusOnePush,
        FocusInfinity,
        FocusToggle,
        FocusSnap,
        FocusZone,
        FocusAutoSensitivity,
        FocusNearLimit,
        FocusLock,
        PushAfPress,
        PushAfRelease,
    );
    row!(o, "Focus::Stop", Focus::Stop);
    row!(o, "Focus::Far", Focus::Far);
    row!(o, "Focus::Near", Focus::Near);
    for s in 0u8..=8 {
        rowr!(
            o,
            format!("Focus::FarWithSpeed[{s}]"),
            Ok(Focus::FarWithSpeed(FocusSpeed::new(s)?))
        );
        rowr!(
            o,
            format!("Focus::NearWithSpeed[{s}]"),
            Ok(Focus::NearWithSpeed(FocusSpeed::new(s)?))
        );
    }
    for p in [0u16, 0x1234, 0xF000, 0xFFFF] {
        row!(
            o,
            format!("Focus::Position[{p:04X}]"),
            Focus::Position(FocusPosition::new(p))
        );
        row!(
            o,
            format!("FocusNearLimitCommand[{p:04X}]"),
            FocusNearLimitCommand::new(FocusPosition::new(p))
        );
    }
    row!(o, "Focus::Auto", Focus::Auto);
    row!(o, "Focus::Manual", Focus::Manual);
    row!(o, "Focus::OnePushTrigger", Focus::OnePushTrigger);
    row!(o, "Focus::Infinity", Focus::Infinity);
    row!(o, "Focus::Toggle", Focus::Toggle);
    row!(o, "Focus::Snap", Focus::Snap);
    row!(o, "FocusZone[Top]", FocusZoneCommand::new(FocusZone::Top));
    row!(
        o,
        "FocusZone[Center]",
        FocusZoneCommand::new(FocusZone::Center)
    );
    row!(
        o,
        "FocusZone[Bottom]",
        FocusZoneCommand::new(FocusZone::Bottom)
    );
    row!(
        o,
        "FocusZone[Zone03]",
        FocusZoneCommand::new(FocusZone::Zone03)
    );
    row!(
        o,
        "AFSensitivity[High]",
        AutoFocusSensitivityCommand::new(AutoFocusSensitivity::High)
    );
    row!(
        o,
        "AFSensitivity[Normal]",
        AutoFocusSensitivityCommand::new(AutoFocusSensitivity::Normal)
    );
    row!(
        o,
        "AFSensitivity[Low]",
        AutoFocusSensitivityCommand::new(AutoFocusSensitivity::Low)
    );
    row!(o, "FocusLock::On", FocusLock::On);
    row!(o, "FocusLock::Off", FocusLock::Off);
    row!(o, "PushAF::Press", PushAF::Press);
    row!(o, "PushAF::Release", PushAF::Release);
    // presets
    cover!(
        covered,
        PresetRecall,
        PresetRecallSpeed,
        PresetSet,
        PresetReset,
    );
    for (an, a) in [
        ("Reset", PresetAction::Reset),
        ("Set", PresetAction::Set),
        ("Recall", PresetAction::Recall),
    ] {
        for n in [0u8, 1, 89, 127, 128, 254, 255] {
            rowr!(
                o,
                format!("Preset[{an},{n}]"),
                Ok(PresetCommand {
                    action: a,
                    preset_number: PresetNumber::new(n)?
                })
            );
        }
    }
    for s in [0u8, 1, 24, 25] {
        rowr!(
            o,
            format!("PresetRecallSpeed[{s}]"),
            Ok(PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(s)?
            })
        );
    }
    // exposure
    cover!(
        covered,
        ExposureMode,
        ExposureCompensationOn,
        ExposureCompensationOff,
        ExposureCompensationReset,
        ExposureCompensationUp,
        ExposureCompensationDown,
        ExposureCompensationDirect,
        DynamicRange,
        IrisReset,
        IrisUp,
        IrisDown,
        IrisDirect,
        ShutterReset,
        ShutterUp,
        ShutterDown,
        ShutterDirect,
        BrightnessReset,
        BrightnessUp,
        BrightnessDown,
        BrightnessSet,
        AntiFlicker,
        SpotlightOn,
        SpotlightOff,
        AutoSlowShutterOn,
        AutoSlowShutterOff,
    );
    for (mn, m) in [
        ("Auto", ExposureMode::Auto),
        ("Manual", ExposureMode::Manual),
        ("Shutter", ExposureMode::Shutter),
        ("Iris", ExposureMode::Iris),
        ("Bright", ExposureMode::Bright),
    ] {
        row!(o, format!("ExposureMode[{mn}]"), ExposureCommand::new(m));
    }
    row!(o, "ExpComp::On", ExposureCompensation::On);
    row!(o, "ExpComp::Off", ExposureCompensation::Off);
    row!(o, "ExpComp::Reset", ExposureCompensation::Reset);
    row!(o, "ExpComp::Up", ExposureCompensation::Up);
    row!(o, "ExpComp::Down", ExposureCompensation::Down);
    for l in [-8i8, -7, -1, 0, 1, 7, 8] {
        rowr!(
            o,
            format!("ExpComp::SetLevel[{l}]"),
            Ok(ExposureCompensation::SetLevel(
                ExposureCompensationLevel::new(l)?
            ))
        );
    }
    for l in [0u8, 1, 5, 6, 8, 0xFF] {
        rowr!(
            o,
            format!("DynamicRange[{l}]"),
            Ok(DynamicRange::new(DynamicRangeLevel::new(l)?))
        );
    }
    row!(o, "Iris::Reset", Iris::Reset);
    row!(o, "Iris::Up", Iris::Up);
    row!(o, "Iris::Down", Iris::Down);
    for l in [0u8, 5, 0x11, 0x12, 0xFF] {
        rowr!(
            o,
            format!("Iris::SetAperture[{l:02X}]"),
            Ok(Iris::SetAperture(IrisLevel::new(l)?))
        );
    }
    row!(o, "Shutter::Reset", Shutter::Reset);
    row!(o, "Shutter::Up", Shutter::Up);
    row!(o, "Shutter::Down", Shutter::Down);
    for l in [0u8, 5, 0x15, 0x16, 0x21, 0x22, 0xFF] {
        row!(
            o,
            format!("Shutter::SetSpeed[{l:04X}]"),
            Shutter::SetSpeed(ShutterSpeed::new(l))
        );
    }
    row!(o, "Brightness::Reset", Brightness::Reset);
    row!(o, "Brightness::Up", Brightness::Up);
    row!(o, "Brightness::Down", Brightness::Down);
    for l in [0u16, 5, 0x1B, 0x1F, 0x20, 0xFF, 0x100] {
        rowr!(
            o,
            format!("Brightness::SetLevel[{l:04X}]"),
            Ok(Brightness::SetLevel(BrightnessLevel::try_from(l)?))
        );
    }
    row!(
        o,
        "AntiFlicker[Off]",
        AntiFlickerCommand::new(AntiFlickerMode::Off)
    );
    row!(
        o,
        "AntiFlicker[50]",
        AntiFlickerCommand::new(AntiFlickerMode::Hz50)
    );
    row!(
        o,
        "AntiFlicker[60]",
        AntiFlickerCommand::new(AntiFlickerMode::Hz60)
    );
    row!(o, "SpotlightOn", SpotlightOn::new());
    row!(o, "SpotlightOff", SpotlightOff::new());
    row!(o, "AutoSlowShutterOn", AutoSlowShutterOn::new());
    row!(o, "AutoSlowShutterOff", AutoSlowShutterOff::new());
    // gain
    cover!(covered, GainReset, GainUp, GainDown, GainDirect, GainLimit);
    row!(o, "Gain::Reset", Gain::Reset);
    row!(o, "Gain::Up", Gain::Up);
    row!(o, "Gain::Down", Gain::Down);
    for l in [0u8, 5, 0x0F, 0x10, 0xFF] {
        rowr!(
            o,
            format!("Gain::SetValue[{l:02X}]"),
            Ok(Gain::SetValue(GainLevel::new(l)?))
        );
        rowr!(
            o,
            format!("GainLimit[{l:02X}]"),
            Ok(GainLimitCommand::new(GainLimit::new(l)?))
        );
    }
    // white balance
    cover!(
        covered,
        WhiteBalanceAuto,
        WhiteBalanceIndoor,
        WhiteBalanceOutdoor,
        WhiteBalanceOnePush,
        WhiteBalanceAutoTracking,
        WhiteBalanceManual,
        WhiteBalanceColorTemperature,
        AutoWhiteBalanceSensitivity,
        OnePushWhiteBalanceTrigger,
    );
    for (mn, m) in [
        ("Auto", WhiteBalanceMode::Auto),
        ("Indoor", WhiteBalanceMode::Indoor),
        ("Outdoor", WhiteBalanceMode::Outdoor),
        ("OnePush", WhiteBalanceMode::OnePush),
        ("ATW", WhiteBalanceMode::ATW),
        ("Manual", WhiteBalanceMode::Manual),
        ("ColorTemperature", WhiteBalanceMode::ColorTemperature),
    ] {
        row!(
            o,
            format!("WhiteBalance[{mn}]"),
            WhiteBalanceCommand::new(m)
        );
    }
    row!(
        o,
        "AWBSensitivity[High]",
        AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::High)
    );
    row!(
        o,
        "AWBSensitivity[Normal]",
        AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::Normal)
    );
    row!(
        o,
        "AWBSensitivity[Low]",
        AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::Low)
    );
    // color
    cover!(
        covered,
        RedTuning,
        BlueTuning,
        Saturation,
        Hue,
        ColorTemperatureReset,
        ColorTemperatureUp,
        ColorTemperatureDown,
        ColorTemperatureDirect,
        RedGainReset,
        RedGainUp,
        RedGainDown,
        RedGainDirect,
        BlueGainReset,
        BlueGainUp,
        BlueGainDown,
        BlueGainDirect,
    );
    row!(o, "OnePushTrigger", OnePushTriggerCommand::new());
    for l in [-11i8, -10, -5, -1, 0, 1, 5, 10, 11] {
        rowr!(
            o,
            format!("RedTuning[{l}]"),
            Ok(RedTuningCommand::new(RedTuning::new(l)?))
        );
        rowr!(
            o,
            format!("BlueTuning[{l}]"),
            Ok(BlueTuningCommand::new(BlueTuning::new(l)?))
        );
    }
    for l in [0u8, 5, 0x0E, 0x0F, 0x10, 0xFF] {
        rowr!(
            o,
            format!("Saturation[{l:02X}]"),
            Ok(SaturationCommand::new(SaturationLevel::new(l)?))
        );
        rowr!(
            o,
            format!("Hue[{l:02X}]"),
            Ok(HueCommand::new(HueLevel::new(l)?))
        );
        rowr!(
            o,
            format!("Luminance[{l:02X}]"),
            Ok(Luminance::new(LuminanceLevel::new(l)?))
        );
        rowr!(
            o,
            format!("Contrast[{l:02X}]"),
            Ok(Contrast::new(ContrastLevel::new(l)?))
        );
        rowr!(
            o,
            format!("Gamma[{l:02X}]"),
            Ok(GammaCommand::new(GammaLevel::new(l)?))
        );
    }
    row!(o, "ColorTemperature::Reset", ColorTemperature::Reset);
    row!(o, "ColorTemperature::Up", ColorTemperature::Up);
    row!(o, "ColorTemperature::Down", ColorTemperature::Down);
    for l in [0u16, 0x30, 0x37, 0x38, 0xFF, 0x100, 0xFFFF] {
        rowr!(
            o,
            format!("ColorTemperature::Set[{l:04X}]"),
            Ok(ColorTemperature::SetTemperature(ColorTemp::new(l)?))
        );
    }
    row!(o, "RedGain::Reset", RedGain::Reset);
    row!(o, "RedGain::Up", RedGain::Up);
    row!(o, "RedGain::Down", RedGain::Down);
    row!(o, "BlueGain::Reset", BlueGain::Reset);
    row!(o, "BlueGain::Up", BlueGain::Up);
    row!(o, "BlueGain::Down", BlueGain::Down);
    for l in [0u8, 0x40, 0x80, 0xFF] {
        rowr!(
            o,
            format!("RedGain::Set[{l:02X}]"),
            Ok(RedGain::SetValue(RedChannel::new(l)?))
        );
        rowr!(
            o,
            format!("BlueGain::Set[{l:02X}]"),
            Ok(BlueGain::SetValue(BlueChannel::new(l)?))
        );
    }
    // image
    cover!(
        covered,
        SharpnessMode,
        SharpnessReset,
        SharpnessUp,
        SharpnessDown,
        SharpnessDirect,
        Luminance,
        Contrast,
        Gamma,
        Backlight,
        NoiseReduction2dMode,
        NoiseReduction2d,
        NoiseReduction2dOff,
        NoiseReduction3d,
        NoiseReduction3dOff,
        ImageFlipOff,
        ImageFlipHorizontal,
        ImageFlipHorizontalOff,
        ImageFlipVertical,
        ImageFlipBoth,
        ImageFlipCombined,
        ImageFreezeOn,
        ImageFreezeOff,
        PictureEffect,
    );
    row!(
        o,
        "Sharpness::Mode[Auto]",
        Sharpness::Mode(SharpnessMode::Auto)
    );
    row!(
        o,
        "Sharpness::Mode[Manual]",
        Sharpness::Mode(SharpnessMode::Manual)
    );
    row!(o, "Sharpness::Reset", Sharpness::Reset);
    row!(o, "Sharpness::Up", Sharpness::Up);
    row!(o, "Sharpness::Down", Sharpness::Down);
    for v in [0u8, 5, 15, 16, 0xFF] {
        rowr!(
            o,
            format!("Sharpness::SetLevel[{v:02X}]"),
            Ok(Sharpness::SetLevel {
                value: SharpnessLevel::new(v)?
            })
        );
    }
    row!(o, "Backlight[on]", BacklightCommand::new(true));
    row!(o, "Backlight[off]", BacklightCommand::new(false));
    row!(o, "NR2D::off", NoiseReduction2D::off());
    for l in [0u8, 1, 5, 6, 0xFF] {
        rowr!(
            o,
            format!("NR2D[{l}]"),
            Ok(NoiseReduction2D::with_level(NoiseReduction2DLevel::new(l)?))
        );
    }
    row!(o, "NR3D::off", NoiseReduction3D::off());
    for l in [0u8, 1, 5, 8, 9, 0xFF] {
        rowr!(
            o,
            format!("NR3D[{l}]"),
            Ok(NoiseReduction3D::with_level(NoiseReduction3DLevel::new(l)?))
        );
    }
    for (mn, m) in [
        ("Off", ImageFlipMode::Off),
        ("Horizontal", ImageFlipMode::Horizontal),
        ("Vertical", ImageFlipMode::Vertical),
        ("Both", ImageFlipMode::Both),
    ] {
        row!(
            o,
            format!("ImageFlipCombined[{mn}]"),
            ImageFlipCombinedCommand::new(m)
        );
    }
    row!(
        o,
        "PictureEffect[Off]",
        PictureEffectCommand {
            mode: PictureEffectMode::Off
        }
    );
    row!(
        o,
        "PictureEffect[BW]",
        PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite
        }
    );
    rowr!(
        o,
        "PictureEffect[Unknown 0x05]",
        Ok(PictureEffectCommand {
            mode: PictureEffectMode::Unknown(UnknownPictureEffect::new(0x05)?)
        })
    );
    row!(o, "ImageFlip[On]", flip::ImageFlip { flip: Flip::On });
    row!(o, "ImageFlip[Off]", flip::ImageFlip { flip: Flip::Off });
    row!(o, "HorizontalFlip[on]", flip::HorizontalFlip { on: true });
    row!(o, "HorizontalFlip[off]", flip::HorizontalFlip { on: false });
    row!(o, "ImageFreeze[on]", ImageFreeze { on: true });
    row!(o, "ImageFreeze[off]", ImageFreeze { on: false });
    // nd
    cover!(
        covered,
        NdFilterMode,
        NdFilterDirect,
        NdFilterStepUp,
        NdFilterStepDown,
        NdFilterAutoOn,
        NdFilterAutoOff,
    );
    row!(
        o,
        "NdFilterMode[Preset]",
        NdFilterModeCommand::new(NdFilterMode::Preset)
    );
    row!(
        o,
        "NdFilterMode[Variable]",
        NdFilterModeCommand::new(NdFilterMode::Variable)
    );
    for v in [0u16, 5, 0x14, 0x15, 0xFFFF] {
        rowr!(o, format!("NdFilterValue[{v:04X}]"), NdFilterValue::new(v));
    }
    row!(
        o,
        "NdFilterStep[Up]",
        NdFilterStepCommand::new(NdFilterStep::Up)
    );
    row!(
        o,
        "NdFilterStep[Down]",
        NdFilterStepCommand::new(NdFilterStep::Down)
    );
    row!(o, "AutoNd[on]", AutoNdCommand::new(true));
    row!(o, "AutoNd[off]", AutoNdCommand::new(false));
    // tally
    cover!(
        covered,
        TallyRedOn,
        TallyRedOff,
        TallyBrightLow,
        TallyBrightHigh,
        TallyGreenOn,
        TallyGreenOff,
        TallyFlash,
        TallyOn,
        TallyOff,
    );
    row!(o, "TallyRedOn", TallyRedOn);
    row!(o, "TallyRedOff", TallyRedOff);
    row!(o, "TallyBrightLo", TallyBrightLo);
    row!(o, "TallyBrightHi", TallyBrightHi);
    row!(o, "TallyGreenOn", TallyGreenOn);
    row!(o, "TallyGreenOff", TallyGreenOff);
    row!(o, "TallyFlash", TallyFlash);
    row!(o, "TallyOn", TallyOn);
    row!(o, "TallyOff", TallyOff);
    // menu
    cover!(
        covered,
        MenuDisplay,
        MenuNavigate,
        MenuSelect,
        MenuCancel,
        DirectMenu,
    );
    row!(o, "MenuDisplay[on]", SetMenuDisplay::new(true));
    row!(o, "MenuDisplay[off]", SetMenuDisplay::new(false));
    for (dn, d) in [
        ("Up", MenuDirection::Up),
        ("Down", MenuDirection::Down),
        ("Left", MenuDirection::Left),
        ("Right", MenuDirection::Right),
    ] {
        row!(o, format!("MenuNavigate[{dn}]"), MenuNavigate::new(d));
    }
    row!(
        o,
        "MenuAction[Select]",
        PerformMenuAction::new(MenuAction::Select)
    );
    row!(
        o,
        "MenuAction[Cancel]",
        PerformMenuAction::new(MenuAction::Cancel)
    );
    for (a, b) in [
        (0x00u8, 0x01u8),
        (0x00, 0xFF),
        (0xFF, 0x00),
        (0xFF, 0xFF),
        (0xFF, 0x81),
        (0x81, 0x01),
    ] {
        rowr!(o, format!("DirectMenu[{a:02X},{b:02X}]"), dmc(a, b));
    }
    // streaming
    cover!(
        covered,
        MulticastStreamingOn,
        MulticastStreamingOff,
        NdiQuality,
        UsbAudioOn,
        UsbAudioOff,
    );
    row!(o, "Multicast::On", MulticastStreaming::On);
    row!(o, "Multicast::Off", MulticastStreaming::Off);
    for (qn, q) in [
        ("High", NdiQuality::High),
        ("Medium", NdiQuality::Medium),
        ("Low", NdiQuality::Low),
        ("Off", NdiQuality::Off),
    ] {
        row!(o, format!("NdiQuality[{qn}]"), SetNdiQuality::new(q));
    }
    row!(o, "UsbAudio::On", UsbAudio::On);
    row!(o, "UsbAudio::Off", UsbAudio::Off);
    // system
    cover!(
        covered,
        AddressSet,
        InterfaceClear,
        CommandCancel,
        SettingsSave,
    );
    row!(o, "AddressSet", AddressSetCommand::new());
    row!(
        o,
        "AddressSet@cam3",
        AddressSetCommand::new(),
        CameraId::CAMERA_3
    );
    row!(
        o,
        "AddressSet@bcast",
        AddressSetCommand::new(),
        CameraId::BROADCAST
    );
    row!(o, "InterfaceClear", InterfaceClearCommand::new());
    row!(
        o,
        "InterfaceClear@cam3",
        InterfaceClearCommand::new(),
        CameraId::CAMERA_3
    );
    row!(
        o,
        "InterfaceClear@bcast",
        InterfaceClearCommand::new(),
        CameraId::BROADCAST
    );
    row!(
        o,
        "CommandCancel[S1]",
        CommandCancelCommand::new(ViscaSocket::S1)
    );
    row!(
        o,
        "CommandCancel[S2]",
        CommandCancelCommand::new(ViscaSocket::S2)
    );
    row!(
        o,
        "CommandCancel[S2]@cam2",
        CommandCancelCommand::new(ViscaSocket::S2),
        CameraId::CAMERA_2
    );
    row!(o, "SettingsSave", SettingsSaveCommand::new());
    // motion sync / variable speed
    cover!(covered, MotionSyncMode, MotionSyncPreset, VariableSpeedMode);
    row!(
        o,
        "MotionSyncMode[On]",
        SetMotionSyncMode::new(MotionSyncMode::On)
    );
    row!(
        o,
        "MotionSyncMode[Off]",
        SetMotionSyncMode::new(MotionSyncMode::Off)
    );
    for s in [0u8, 1, 2, 3, 0x18, 0xFF] {
        rowr!(
            o,
            format!("MotionSyncPreset[{s}]"),
            MotionSyncSpeed::new(s).map(SetMotionSyncPreset::new)
        );
    }
    row!(
        o,
        "VariableSpeed[Standard24]",
        SetVariableSpeedMode::new(VariableSpeedMode::Standard24)
    );
    row!(
        o,
        "VariableSpeed[Fine50]",
        SetVariableSpeedMode::new(VariableSpeedMode::Fine50)
    );
    // Sony FR7 R7 v4 controls and boundary values.
    cover!(
        covered,
        Fr7AeLevelUp,
        Fr7AeLevelDown,
        Fr7AutoGain,
        Fr7AutoShutter,
        Fr7AutoIris,
        Fr7IrisUp,
        Fr7IrisDown,
        Fr7WhiteBalanceSet,
        Fr7PresetWhiteUp,
        Fr7PresetWhiteDown,
        Fr7PresetWhiteDirect,
        Fr7MemoryColorTemperatureUp,
        Fr7MemoryColorTemperatureDown,
        Fr7MemoryColorTemperatureDirect,
        Fr7MemoryRedGainUp,
        Fr7MemoryRedGainDown,
        Fr7MemoryRedGainDirect,
        Fr7MemoryBlueGainUp,
        Fr7MemoryBlueGainDown,
        Fr7MemoryBlueGainDirect,
        Fr7MemoryTintUp,
        Fr7MemoryTintDown,
        Fr7MemoryTintDirect,
        Fr7AtwColorTemperatureOffsetUp,
        Fr7AtwColorTemperatureOffsetDown,
        Fr7AtwColorTemperatureOffsetDirect,
        Fr7AtwTintOffsetUp,
        Fr7AtwTintOffsetDown,
        Fr7AtwTintOffsetDirect,
        Fr7MasterBlackUp,
        Fr7MasterBlackDown,
        Fr7MasterBlackDirect,
        Fr7RedBlackUp,
        Fr7RedBlackDown,
        Fr7RedBlackDirect,
        Fr7BlueBlackUp,
        Fr7BlueBlackDown,
        Fr7BlueBlackDirect,
        Fr7DetailSetting,
        Fr7KneeSetting,
        Fr7SetKneeMode,
        Fr7SetKneeSlope,
        Fr7SetKneePoint,
        Fr7ZoomHighResolutionTele,
        Fr7ZoomHighResolutionWide,
        Fr7ZoomHighResolutionStop,
        Fr7SetPresetSpeedSelection,
        Fr7SetCommonPresetSpeed,
        Fr7SetSeparatePresetMode,
        Fr7SetSeparatePresetSpeed,
        Fr7SetSeparatePresetDuration,
        WhiteBalancePreset
    );
    row!(o, "Fr7AeLevelUp[R7]", Fr7AeLevel::Up);
    row!(o, "Fr7AeLevelDown[R7]", Fr7AeLevel::Down);
    row!(o, "Fr7AutoGain[R7]", Fr7AutoGain::new(true));
    row!(o, "Fr7AutoShutter[R7]", Fr7AutoShutter::new(true));
    row!(o, "Fr7AutoIris[R7]", Fr7AutoIris::new(true));
    row!(o, "Fr7IrisUp[R7]", Fr7IrisUp::new(IrisStep::MAX));
    row!(o, "Fr7IrisDown[R7]", Fr7IrisDown::new(IrisStep::MAX));
    row!(o, "Fr7WhiteBalanceSet[R7]", Fr7WhiteBalanceSet);
    row!(
        o,
        "Fr7PresetWhiteUp[R7]",
        Fr7PresetWhite::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7PresetWhiteDown[R7]",
        Fr7PresetWhite::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7PresetWhiteDirect[R7]",
        Fr7PresetWhite::Direct(WhiteKelvin::MAX)
    );
    row!(
        o,
        "Fr7MemoryColorTemperatureUp[R7]",
        Fr7MemoryColorTemperature::Up(MemoryColorTemperatureStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryColorTemperatureDown[R7]",
        Fr7MemoryColorTemperature::Down(MemoryColorTemperatureStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryColorTemperatureDirect[R7]",
        Fr7MemoryColorTemperature::Direct(WhiteKelvin::MAX)
    );
    row!(
        o,
        "Fr7MemoryRedGainUp[R7]",
        Fr7MemoryRedGain::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryRedGainDown[R7]",
        Fr7MemoryRedGain::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryRedGainDirect[R7]",
        Fr7MemoryRedGain::Direct(WhiteBalanceGain::MAX)
    );
    row!(
        o,
        "Fr7MemoryBlueGainUp[R7]",
        Fr7MemoryBlueGain::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryBlueGainDown[R7]",
        Fr7MemoryBlueGain::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryBlueGainDirect[R7]",
        Fr7MemoryBlueGain::Direct(WhiteBalanceGain::MAX)
    );
    row!(
        o,
        "Fr7MemoryTintUp[R7]",
        Fr7MemoryTint::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryTintDown[R7]",
        Fr7MemoryTint::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7MemoryTintDirect[R7]",
        Fr7MemoryTint::Direct(TintOffset::MAX)
    );
    row!(
        o,
        "Fr7AtwColorTemperatureOffsetUp[R7]",
        Fr7AtwColorTemperatureOffset::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7AtwColorTemperatureOffsetDown[R7]",
        Fr7AtwColorTemperatureOffset::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7AtwColorTemperatureOffsetDirect[R7]",
        Fr7AtwColorTemperatureOffset::Direct(TintOffset::MAX)
    );
    row!(
        o,
        "Fr7AtwTintOffsetUp[R7]",
        Fr7AtwTintOffset::Up(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7AtwTintOffsetDown[R7]",
        Fr7AtwTintOffset::Down(WhiteBalanceStep::MAX)
    );
    row!(
        o,
        "Fr7AtwTintOffsetDirect[R7]",
        Fr7AtwTintOffset::Direct(TintOffset::MAX)
    );
    row!(
        o,
        "Fr7MasterBlackUp[R7]",
        Fr7MasterBlack::Up(BlackStep::MAX)
    );
    row!(
        o,
        "Fr7MasterBlackDown[R7]",
        Fr7MasterBlack::Down(BlackStep::MAX)
    );
    row!(
        o,
        "Fr7MasterBlackDirect[R7]",
        Fr7MasterBlack::Direct(BlackLevel::MAX)
    );
    row!(o, "Fr7RedBlackUp[R7]", Fr7RedBlack::Up(BlackStep::MAX));
    row!(o, "Fr7RedBlackDown[R7]", Fr7RedBlack::Down(BlackStep::MAX));
    row!(
        o,
        "Fr7RedBlackDirect[R7]",
        Fr7RedBlack::Direct(BlackLevel::MAX)
    );
    row!(o, "Fr7BlueBlackUp[R7]", Fr7BlueBlack::Up(BlackStep::MAX));
    row!(
        o,
        "Fr7BlueBlackDown[R7]",
        Fr7BlueBlack::Down(BlackStep::MAX)
    );
    row!(
        o,
        "Fr7BlueBlackDirect[R7]",
        Fr7BlueBlack::Direct(BlackLevel::MAX)
    );
    row!(o, "Fr7DetailSetting[R7]", Fr7DetailSetting::new(true));
    row!(o, "Fr7KneeSetting[R7]", Fr7KneeSetting::new(true));
    row!(
        o,
        "Fr7SetKneeMode[R7]",
        Fr7SetKneeMode::new(Fr7KneeMode::Manual)
    );
    row!(
        o,
        "Fr7SetKneeSlope[R7]",
        Fr7SetKneeSlope::new(KneeSlope::MAX)
    );
    row!(
        o,
        "Fr7SetKneePoint[R7]",
        Fr7SetKneePoint::new(KneePoint::MAX)
    );
    row!(
        o,
        "Fr7ZoomHighResolutionTele[R7]",
        Fr7ZoomHighResolution::Tele(HighResolutionZoomSpeed::MAX)
    );
    row!(
        o,
        "Fr7ZoomHighResolutionWide[R7]",
        Fr7ZoomHighResolution::Wide(HighResolutionZoomSpeed::MAX)
    );
    row!(
        o,
        "Fr7ZoomHighResolutionStop[R7]",
        Fr7ZoomHighResolutionStop
    );
    row!(
        o,
        "Fr7SetPresetSpeedSelection[R7]",
        Fr7SetPresetSpeedSelection::new(Fr7PresetSpeedSelection::Common)
    );
    row!(
        o,
        "Fr7SetCommonPresetSpeed[R7]",
        Fr7SetCommonPresetSpeed::new(PresetSpeed::MAX)
    );
    row!(
        o,
        "Fr7SetSeparatePresetMode[R7]",
        Fr7SetSeparatePresetMode::new(Fr7PresetNumber::MAX, Fr7PresetSeparateMode::Duration)
    );
    row!(
        o,
        "Fr7SetSeparatePresetSpeed[R7]",
        Fr7SetSeparatePresetSpeed::new(Fr7PresetNumber::MAX, PresetSpeed::MAX)
    );
    row!(
        o,
        "Fr7SetSeparatePresetDuration[R7]",
        Fr7SetSeparatePresetDuration::new(Fr7PresetNumber::MAX, PresetDuration::MAX)
    );
    row!(
        o,
        "WhiteBalancePreset[R7]",
        WhiteBalanceCommand::new(WhiteBalanceMode::Preset)
    );
    row!(o, "Fr7AeLevelInquiry", Fr7AeLevelInquiry);
    row!(o, "Fr7AutoGainInquiry", Fr7AutoGainInquiry);
    row!(o, "Fr7AutoShutterInquiry", Fr7AutoShutterInquiry);
    row!(o, "Fr7AutoIrisInquiry", Fr7AutoIrisInquiry);
    row!(o, "Fr7PresetWhiteInquiry", Fr7PresetWhiteInquiry);
    row!(
        o,
        "Fr7MemoryColorTemperatureInquiry",
        Fr7MemoryColorTemperatureInquiry
    );
    row!(o, "Fr7MemoryRedGainInquiry", Fr7MemoryRedGainInquiry);
    row!(o, "Fr7MemoryBlueGainInquiry", Fr7MemoryBlueGainInquiry);
    row!(o, "Fr7MemoryTintInquiry", Fr7MemoryTintInquiry);
    row!(
        o,
        "Fr7AtwColorTemperatureOffsetInquiry",
        Fr7AtwColorTemperatureOffsetInquiry
    );
    row!(o, "Fr7AtwTintOffsetInquiry", Fr7AtwTintOffsetInquiry);
    row!(o, "Fr7MasterBlackInquiry", Fr7MasterBlackInquiry);
    row!(o, "Fr7RedBlackInquiry", Fr7RedBlackInquiry);
    row!(o, "Fr7BlueBlackInquiry", Fr7BlueBlackInquiry);
    row!(o, "Fr7DetailInquiry", Fr7DetailInquiry);
    row!(o, "Fr7KneeInquiry", Fr7KneeInquiry);
    row!(o, "Fr7KneeModeInquiry", Fr7KneeModeInquiry);
    row!(o, "Fr7KneeSlopeInquiry", Fr7KneeSlopeInquiry);
    row!(o, "Fr7KneePointInquiry", Fr7KneePointInquiry);
    row!(
        o,
        "Fr7KneePermissionInquiry",
        Fr7KneePermissionInquiry::new(Fr7KneeParameter::Setting)
    );
    row!(
        o,
        "Fr7ClearImageZoomPositionInquiry",
        Fr7ClearImageZoomPositionInquiry
    );
    row!(
        o,
        "Fr7PresetSpeedSelectionInquiry",
        Fr7PresetSpeedSelectionInquiry
    );
    row!(
        o,
        "Fr7CommonPresetSpeedInquiry",
        Fr7CommonPresetSpeedInquiry
    );
    row!(
        o,
        "Fr7SeparatePresetModeInquiry",
        Fr7SeparatePresetModeInquiry::new(Fr7PresetNumber::MIN)
    );
    row!(
        o,
        "Fr7SeparatePresetSpeedInquiry",
        Fr7SeparatePresetSpeedInquiry::new(Fr7PresetNumber::MIN)
    );
    row!(
        o,
        "Fr7SeparatePresetDurationInquiry",
        Fr7SeparatePresetDurationInquiry::new(Fr7PresetNumber::MIN)
    );
    // inquiries (common set)
    macro_rules! inq { ($($t:ident),* $(,)?) => { $( row!(o, stringify!($t), $t); )* } }
    inq!(
        AutoFocusSensitivityInquiry,
        AutoWhiteBalanceSensitivityInquiry,
        BacklightInquiry,
        BlueGainInquiry,
        BlueTuningInquiry,
        BrightnessInquiry,
        ColorTemperatureInquiry,
        ContrastInquiry,
        ExposureCompensationInquiry,
        ExposureCompensationModeInquiry,
        ExposureCompensationPositionInquiry,
        ExposureModeInquiry,
        FlickerModeInquiry,
        FlipStateInquiry,
        FocusModeInquiry,
        FocusNearLimitInquiry,
        FocusPositionInquiry,
        FocusZoneInquiry,
        GainInquiry,
        GainLimitInquiry,
        GammaInquiry,
        HueInquiry,
        ImageFlipInquiry,
        IrisInquiry,
        LuminanceInquiry,
        MenuOpenCloseInquiry,
        MotionSyncModeInquiry,
        MotionSyncPresetInquiry,
        NdFilterInquiry,
        NdFilterPresetInquiry,
        NoiseReduction2DInquiry,
        NoiseReduction3DInquiry,
        PanTiltPositionInquiry,
        PictureEffectInquiry,
        PowerInquiry,
        RedGainInquiry,
        RedTuningInquiry,
        SaturationInquiry,
        SharpnessModeInquiry,
        SharpnessPositionInquiry,
        ShutterInquiry,
        TallyGreenInquiry,
        TallyRedInquiry,
        UsbAudioInquiry,
        VersionInquiry,
        WhiteBalanceModeInquiry,
        ZoomPositionInquiry,
    );
    row!(o, "PowerInquiry@cam7", PowerInquiry, CameraId::CAMERA_7);
    extra_rows(&mut o);
    o.extend(registered_profile_rows());

    let expected = BuiltinCommand::ALL.iter().copied().collect::<HashSet<_>>();
    assert_eq!(
        covered, expected,
        "every semantic-ledger command must be tied to the literal wire inventory",
    );
    assert_inquiry_coverage(&o);
    o
}

#[test]
fn wire_ledger_matches_literal_golden_inventory() {
    let actual = rows().join("\n") + "\n";
    assert_golden(&actual);
}

// A bless run must never report green, so it ends in a deliberate panic.
#[allow(clippy::expect_used, clippy::panic)]
fn assert_golden(actual: &str) {
    const EXPECTED: &str = include_str!("../tests/fixtures/issue_715_wire_golden.txt");
    if std::env::var("GRAFTON_VISCA_BLESS_WIRE_GOLDEN").as_deref() == Ok("1") {
        assert!(
            std::env::var_os("CI").is_none(),
            "GRAFTON_VISCA_BLESS_WIRE_GOLDEN regenerates the fixture and is refused under CI"
        );
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/issue_715_wire_golden.txt"
        );
        std::fs::write(path, actual).expect("write the regenerated wire golden fixture");
        panic!("fixture blessed; review the diff and rerun without the variable");
    }
    let actual_rows = actual.lines().collect::<Vec<_>>();
    let expected_rows = EXPECTED.lines().collect::<Vec<_>>();
    assert_eq!(
        actual_rows.len(),
        expected_rows.len(),
        "built-in VISCA wire inventory row count drifted",
    );
    for (index, (actual_row, expected_row)) in
        actual_rows.iter().zip(expected_rows.iter()).enumerate()
    {
        assert_eq!(
            actual_row,
            expected_row,
            "built-in VISCA wire inventory row {} drifted",
            index + 1,
        );
    }
}

fn assert_inquiry_coverage(rows: &[String]) {
    let names = rows
        .iter()
        .filter_map(|row| row.split_once('\t').map(|(name, _)| name))
        .collect::<HashSet<_>>();

    for metadata in BUILTIN_INQUIRIES {
        {
            assert!(
                names.contains(metadata.name),
                "queryable inquiry {} is absent from the literal wire inventory",
                metadata.name,
            );
        }
    }
}

// ---- Shared row helpers ----
use crate::command::encode::WireEncode as Enc;
fn dmc(a: u8, b: u8) -> Result<DirectMenuControl, Error> {
    Ok(DirectMenuControl::new(
        DirectMenuTarget::try_from(a)?,
        MenuButton::try_from(b)?,
    ))
}
fn extra_rows(o: &mut Vec<String>) {
    row!(
        o,
        "NR2DMode[Auto]",
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Auto)
    );
    row!(
        o,
        "NR2DMode[Manual]",
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Manual)
    );
    row!(
        o,
        "NoiseReduction2DModeInquiry",
        NoiseReduction2DModeInquiry
    );
}

// The facade builds exercise complete preparation and inert admission. The bare
// library has no owner/preparation module, so it exercises the same sealed
// target-validation and serialization seam, retaining identical fixture bytes.
#[cfg(not(any(feature = "blocking", feature = "async")))]
fn registered_wire<R: crate::Request>(
    request: &R,
    target: &crate::TargetRegistration,
) -> Result<Vec<u8>, Error> {
    request.validate_for_target(target)?;
    let mut buffer = [0; 64];
    let written = crate::Request::write_for_target(
        request,
        target,
        &mut buffer,
        crate::requests::RequestContractAuthority::new(),
    )?;
    Ok(buffer[..written].to_vec())
}

fn prepared_position<O: crate::OperationCommand<crate::completion::Targeted>>(
    request: &O,
    target: &crate::TargetRegistration,
) -> Result<Vec<u8>, Error> {
    #[cfg(any(feature = "blocking", feature = "async"))]
    {
        crate::prepared::test_support::prepare_operation::<crate::completion::Targeted, _>(
            request,
            target,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .map(|prepared| prepared.admit_with(|request, _, _, _| request.wire().as_bytes().to_vec()))
    }
    #[cfg(not(any(feature = "blocking", feature = "async")))]
    registered_wire(request, target)
}

fn prepared_limit<C: crate::PlainCommand>(
    request: &C,
    target: &crate::TargetRegistration,
) -> Result<Vec<u8>, Error> {
    #[cfg(any(feature = "blocking", feature = "async"))]
    {
        crate::prepared::test_support::prepare_command(
            request,
            target,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .map(|prepared| prepared.admit_with(|request, _| request.wire().as_bytes().to_vec()))
    }
    #[cfg(not(any(feature = "blocking", feature = "async")))]
    registered_wire(request, target)
}

fn prepared_query<Q: crate::Inquiry>(
    request: &Q,
    target: &crate::TargetRegistration,
) -> Result<Vec<u8>, Error> {
    #[cfg(any(feature = "blocking", feature = "async"))]
    {
        crate::prepared::test_support::prepare_inquiry(
            request,
            target,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .map(|prepared| prepared.admit_with(|request, _, _| request.wire().as_bytes().to_vec()))
    }
    #[cfg(not(any(feature = "blocking", feature = "async")))]
    registered_wire(request, target)
}

#[allow(clippy::expect_used)]
fn registered_target<P: crate::CompileTimeProfile>() -> crate::TargetRegistration {
    crate::TargetRegistration::new(
        CameraId::CAMERA_1,
        crate::ProfileSpec::from_compile_time::<P>().expect("source-backed profile"),
    )
    .expect("individual target")
}

#[allow(clippy::expect_used)]
fn unsigned_target() -> crate::TargetRegistration {
    let source =
        crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
    let mut capabilities = source.capabilities().clone();
    capabilities.profile_id = None;
    capabilities.model_name = "Unsigned-centered coordinate fixture".into();
    capabilities
        .pan_tilt
        .as_mut()
        .expect("pan/tilt")
        .coordinates = crate::PanTiltCoordinateConversion::new(
        crate::capabilities::CoordinateSystem::UnsignedCentered,
        crate::capabilities::PanTiltWireCodec::StandardVisca,
        14.4,
        14.4,
    )
    .expect("unsigned standard grammar");
    let profile = crate::ProfileSpec::builder(capabilities)
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(source.supports_operation_complete())
        .supports_command_cancel(source.supports_command_cancel())
        .position_inquiries(source.position_inquiries())
        .build()
        .expect("validated custom profile");
    crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("unsigned target")
}

#[allow(clippy::expect_used)]
fn registered_profile_rows() -> Vec<String> {
    use crate::{request::builtin, units::Degrees};
    let desktop = registered_target::<crate::profiles::SonyFR7>();
    let ceiling = desktop
        .clone()
        .with_mount(crate::MountPosition::Ceiling)
        .expect("ceiling");
    let standard = registered_target::<crate::profiles::PtzOpticsG2>();
    let brc = registered_target::<crate::profiles::SonyBRC300>();
    let unsigned = unsigned_target();
    let mut output = Vec::new();
    macro_rules! case {
        ($label:literal, $bytes:expr) => {{
            let result: Result<Vec<u8>, Error> = (|| $bytes)();
            output.push(match result {
                Ok(bytes) => format!("{}\t{}", $label, hex(&bytes)),
                Err(error) => format!("{}\tREJECT({:?})", $label, error.kind()),
            });
        }};
    }
    macro_rules! position {
        ($label:literal, $target:ident, $kind:ident, $pan:expr, $tilt:expr, $speed:expr, $other:expr) => {
            case!(
                $label,
                prepared_position(
                    &builtin::$kind::for_target(
                        Degrees($pan),
                        Degrees($tilt),
                        PanSpeed::new($speed)?,
                        TiltSpeed::new($other)?,
                        &$target,
                    )?,
                    &$target
                )
            );
        };
    }
    position!(
        "FR7[Desktop,ABS,+170,+195]",
        desktop,
        PanTiltAbsolute,
        170.0,
        195.0,
        9,
        5
    );
    position!(
        "FR7[Desktop,ABS,-170,-30]",
        desktop,
        PanTiltAbsolute,
        -170.0,
        -30.0,
        9,
        5
    );
    position!(
        "FR7[Desktop,REL,+200,0]",
        desktop,
        PanTiltRelative,
        200.0,
        0.0,
        9,
        5
    );
    position!(
        "FR7[Desktop,REL,0,-35]",
        desktop,
        PanTiltRelative,
        0.0,
        -35.0,
        9,
        5
    );
    position!(
        "FR7[Desktop,ABS,+200,0,refused]",
        desktop,
        PanTiltAbsolute,
        200.0,
        0.0,
        9,
        5
    );
    position!(
        "FR7[Desktop,ABS,0,-35,refused]",
        desktop,
        PanTiltAbsolute,
        0.0,
        -35.0,
        9,
        5
    );
    case!(
        "FR7[Desktop,LimitSet,UpRight]",
        prepared_limit(
            &builtin::PanTiltLimitSet::for_target(
                PanTiltLimitCorner::UpRight,
                Degrees(170.0),
                Degrees(195.0),
                &desktop,
            )?,
            &desktop
        )
    );
    case!(
        "FR7[Desktop,LimitSet,DownLeft]",
        prepared_limit(
            &builtin::PanTiltLimitSet::for_target(
                PanTiltLimitCorner::DownLeft,
                Degrees(-170.0),
                Degrees(-30.0),
                &desktop,
            )?,
            &desktop
        )
    );
    case!(
        "FR7[LimitClear,UpRight]",
        prepared_limit(
            &builtin::PanTiltLimitClear::for_profile(
                PanTiltLimitCorner::UpRight,
                desktop.profile(),
            )?,
            &desktop
        )
    );
    case!(
        "FR7[LimitClear,DownLeft]",
        prepared_limit(
            &builtin::PanTiltLimitClear::for_profile(
                PanTiltLimitCorner::DownLeft,
                desktop.profile(),
            )?,
            &desktop
        )
    );
    position!(
        "FR7[Ceiling,ABS,0,+15]",
        ceiling,
        PanTiltAbsolute,
        0.0,
        15.0,
        9,
        5
    );
    position!(
        "FR7[Ceiling,ABS,0,-210]",
        ceiling,
        PanTiltAbsolute,
        0.0,
        -210.0,
        9,
        5
    );
    position!(
        "FR7[Ceiling,ABS,0,+80,refused]",
        ceiling,
        PanTiltAbsolute,
        0.0,
        80.0,
        9,
        5
    );
    position!("G2[REL,0,-45]", standard, PanTiltRelative, 0.0, -45.0, 9, 7);
    position!(
        "G2[ABS,0,-45,refused]",
        standard,
        PanTiltAbsolute,
        0.0,
        -45.0,
        9,
        7
    );
    position!(
        "G2[ABS,+45,+15,paired-speed]",
        standard,
        PanTiltAbsolute,
        45.0,
        15.0,
        9,
        7
    );
    position!(
        "G2[ABS,-45,-15,paired-speed]",
        standard,
        PanTiltAbsolute,
        -45.0,
        -15.0,
        9,
        7
    );
    position!(
        "BRC300[ABS,+45,+15,single-speed]",
        brc,
        PanTiltAbsolute,
        45.0,
        15.0,
        9,
        9
    );
    position!(
        "BRC300[ABS,-45,-15,single-speed]",
        brc,
        PanTiltAbsolute,
        -45.0,
        -15.0,
        9,
        9
    );
    position!(
        "BRC300[REL,+45,+15,single-speed]",
        brc,
        PanTiltRelative,
        45.0,
        15.0,
        9,
        9
    );
    position!(
        "BRC300[REL,-45,-15,single-speed]",
        brc,
        PanTiltRelative,
        -45.0,
        -15.0,
        9,
        9
    );
    position!(
        "BRC300[ABS,unequal-speeds,refused]",
        brc,
        PanTiltAbsolute,
        45.0,
        15.0,
        9,
        7
    );
    position!(
        "Unsigned[ABS,zero,bias]",
        unsigned,
        PanTiltAbsolute,
        0.0,
        0.0,
        9,
        7
    );
    position!(
        "Unsigned[REL,zero,no-bias]",
        unsigned,
        PanTiltRelative,
        0.0,
        0.0,
        9,
        7
    );
    case!(
        "H900[PowerInquiry,target-aware]",
        prepared_query(
            &PowerInquiry,
            &registered_target::<crate::profiles::SonyBRCH900>(),
        )
    );
    output
}

#[test]
fn registered_profile_rows_match_reviewed_literals() {
    // R7 v4 p. 12 states endpoint words and mount ranges; p. 18 shows five
    // nibbles per axis and the exact limit-clear words. Pan degrees point
    // opposite the raw axis. Interior offsets use the shared nearest-unit
    // rule: -35 * 235.9 rounds to -8257; +200 pan degrees is -47180 units.
    // R12's BRC-300 grammar retains its reserved zero after the single speed,
    // five pan nibbles and four tilt nibbles. R11's power query is extended.
    let expected = [
        "FR7[Desktop,ABS,+170,+195]\t81 01 06 02 09 05 0F 06 03 05 09 00 0B 03 0B 00 FF",
        "FR7[Desktop,ABS,-170,-30]\t81 01 06 02 09 05 00 09 0C 0A 07 0F 0E 04 05 0B FF",
        "FR7[Desktop,REL,+200,0]\t81 01 06 03 09 05 0F 04 07 0B 04 00 00 00 00 00 FF",
        "FR7[Desktop,REL,0,-35]\t81 01 06 03 09 05 00 00 00 00 00 0F 0D 0F 0B 0F FF",
        "FR7[Desktop,ABS,+200,0,refused]\tREJECT(InvalidParameter)",
        "FR7[Desktop,ABS,0,-35,refused]\tREJECT(InvalidParameter)",
        "FR7[Desktop,LimitSet,UpRight]\t81 01 06 07 00 01 0F 06 03 05 09 00 0B 03 0B 00 FF",
        "FR7[Desktop,LimitSet,DownLeft]\t81 01 06 07 00 00 00 09 0C 0A 07 0F 0E 04 05 0B FF",
        "FR7[LimitClear,UpRight]\t81 01 06 07 01 01 07 0F 0F 0F 0F 07 0F 0F 0F 0F FF",
        "FR7[LimitClear,DownLeft]\t81 01 06 07 01 00 07 0F 0F 0F 0F 07 0F 0F 0F 0F FF",
        "FR7[Ceiling,ABS,0,+15]\t81 01 06 02 09 05 00 00 00 00 00 00 00 0D 0D 02 FF",
        "FR7[Ceiling,ABS,0,-210]\t81 01 06 02 09 05 00 00 00 00 00 0F 03 0E 07 0D FF",
        "FR7[Ceiling,ABS,0,+80,refused]\tREJECT(InvalidParameter)",
        "G2[REL,0,-45]\t81 01 06 03 09 07 00 00 00 00 0F 0D 07 08 FF",
        "G2[ABS,0,-45,refused]\tREJECT(InvalidParameter)",
        "G2[ABS,+45,+15,paired-speed]\t81 01 06 02 09 07 00 02 08 08 00 00 0D 08 FF",
        "G2[ABS,-45,-15,paired-speed]\t81 01 06 02 09 07 0F 0D 07 08 0F 0F 02 08 FF",
        "BRC300[ABS,+45,+15,single-speed]\t81 01 06 02 09 00 0F 0D 0B 07 00 00 0C 03 00 FF",
        "BRC300[ABS,-45,-15,single-speed]\t81 01 06 02 09 00 00 02 04 09 00 0F 03 0D 00 FF",
        "BRC300[REL,+45,+15,single-speed]\t81 01 06 03 09 00 0F 0D 0B 07 00 00 0C 03 00 FF",
        "BRC300[REL,-45,-15,single-speed]\t81 01 06 03 09 00 00 02 04 09 00 0F 03 0D 00 FF",
        "BRC300[ABS,unequal-speeds,refused]\tREJECT(InvalidParameter)",
        "Unsigned[ABS,zero,bias]\t81 01 06 02 09 07 08 00 00 00 08 00 00 00 FF",
        "Unsigned[REL,zero,no-bias]\t81 01 06 03 09 07 00 00 00 00 00 00 00 00 FF",
        "H900[PowerInquiry,target-aware]\t81 09 7E 04 00 FF",
    ];
    let actual = registered_profile_rows();
    assert_eq!(actual, expected);
    assert!(
        rows().ends_with(&actual),
        "registered cases must enter the golden generator"
    );
}
