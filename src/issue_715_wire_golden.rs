//! Exhaustive built-in encoder snapshot for issue #715.
//!
//! The fixture is the 491-row phase-4 mechanical comparison against the pinned
//! 1.2 oracle, plus three v2-only 2D-NR mode rows. Its 31 changed comparison
//! rows are the source-backed 2.0 deltas ratified in D5. Expectations
//! are literal text, never calculated through the encoder under test. The
//! semantic-ledger assertion below makes a newly added built-in command fail
//! until it is deliberately represented by this generator and reviewed in the
//! fixture; generated inquiry metadata is checked the same way.

#![allow(
    clippy::redundant_closure_call,
    unused_imports,
    dead_code,
    unused_qualifications,
    deprecated
)]

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
fn pp(v: i16) -> Result<PanPosition, Error> {
    PanPosition::new(v)
}
fn tp(v: i16) -> Result<TiltPosition, Error> {
    TiltPosition::new(v)
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
    for (p, t) in [
        (144i16, -72i16),
        (0, 0),
        (-1, -1),
        (2448, 1296),
        (-2448, -432),
        (2449, 0),
        (0, 1297),
        (0, -433),
        (i16::MAX, i16::MIN),
    ] {
        rowr!(
            o,
            format!("PanTilt::AbsolutePosition[{p},{t}]"),
            Ok(PanTilt::AbsolutePosition {
                pan: pp(p)?,
                tilt: tp(t)?,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        rowr!(
            o,
            format!("PanTilt::RelativePosition[{p},{t}]"),
            Ok(PanTilt::RelativePosition {
                pan: pp(p)?,
                tilt: tp(t)?,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        rowr!(
            o,
            format!("PanTilt::LimitSet[DownLeft,{p},{t}]"),
            Ok(PanTilt::LimitSet {
                corner: PanTiltLimitCorner::DownLeft,
                pan: pp(p)?,
                tilt: tp(t)?
            })
        );
        rowr!(
            o,
            format!("PanTilt::LimitSet[UpRight,{p},{t}]"),
            Ok(PanTilt::LimitSet {
                corner: PanTiltLimitCorner::UpRight,
                pan: pp(p)?,
                tilt: tp(t)?
            })
        );
    }
    for (p, t) in [(0x0090u16, 0xFFB8u16), (0xFFFF, 0x0000), (0x1234, 0xABCD)] {
        rowr!(
            o,
            format!("PanTilt::AbsolutePositionRaw[{p:04X},{t:04X}]"),
            Ok(PanTilt::AbsolutePositionRaw {
                pan_u16: p,
                tilt_u16: t,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        rowr!(
            o,
            format!("PanTilt::RelativePositionRaw[{p:04X},{t:04X}]"),
            Ok(PanTilt::RelativePositionRaw {
                pan_u16: p,
                tilt_u16: t,
                pan_speed: ps(0x0A)?,
                tilt_speed: ts(0x05)?
            })
        );
        row!(
            o,
            format!("PanTilt::LimitSetRaw[DownLeft,{p:04X},{t:04X}]"),
            PanTilt::LimitSetRaw {
                corner: PanTiltLimitCorner::DownLeft,
                pan_u16: p,
                tilt_u16: t
            }
        );
        row!(
            o,
            format!("PanTilt::LimitSetRaw[UpRight,{p:04X},{t:04X}]"),
            PanTilt::LimitSetRaw {
                corner: PanTiltLimitCorner::UpRight,
                pan_u16: p,
                tilt_u16: t
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
            Ok(Focus::FarWithSpeed(crate::command::focus::FocusSpeed::new(
                s
            )?))
        );
        rowr!(
            o,
            format!("Focus::NearWithSpeed[{s}]"),
            Ok(Focus::NearWithSpeed(
                crate::command::focus::FocusSpeed::new(s)?
            ))
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
        BrightnessDirect,
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
    for l in [0u16, 5, 0x15, 0x16, 0x21, 0x22, 0xFFFF] {
        rowr!(
            o,
            format!("Shutter::SetSpeed[{l:04X}]"),
            Ok(Shutter::SetSpeed(ShutterSpeed::new(l)?))
        );
    }
    row!(o, "Brightness::Reset", Brightness::Reset);
    row!(o, "Brightness::Up", Brightness::Up);
    row!(o, "Brightness::Down", Brightness::Down);
    for l in [0u16, 5, 0x1B, 0x1F, 0x20, 0xFF, 0x100] {
        rowr!(
            o,
            format!("Brightness::SetLevel[{l:04X}]"),
            Ok(Brightness::SetLevel(BrightnessLevel::new(l)?))
        );
        rowr!(
            o,
            format!("Brightness::Direct[{l:04X}]"),
            Ok(Brightness::Direct(BrightnessLevel::new(l)?))
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
        row!(
            o,
            format!("Sharpness::SetLevel[{v:02X}]"),
            Sharpness::SetLevel { value: v }
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
    row!(
        o,
        "PictureEffect[Unknown 0x02]",
        PictureEffectCommand {
            mode: PictureEffectMode::Unknown(0x02)
        }
    );
    row!(
        o,
        "ImageFlip[On]",
        crate::command::flip::ImageFlip { flip: Flip::On }
    );
    row!(
        o,
        "ImageFlip[Off]",
        crate::command::flip::ImageFlip { flip: Flip::Off }
    );
    row!(
        o,
        "HorizontalFlip[on]",
        crate::command::flip::HorizontalFlip { on: true }
    );
    row!(
        o,
        "HorizontalFlip[off]",
        crate::command::flip::HorizontalFlip { on: false }
    );
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
            SetMotionSyncPreset::new(s)
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
    // inquiries (common set)
    macro_rules! inq { ($($t:ident),* $(,)?) => { $( row!(o, stringify!($t), $t); )* } }
    inq!(
        AutoFocusSensitivityInquiry,
        AutoTraceInquiry,
        AutoWhiteBalanceSensitivityInquiry,
        BacklightInquiry,
        BlueGainInquiry,
        BlueTuningInquiry,
        BrightnessInquiry,
        BroadcastDomainInquiry,
        ColorTemperatureInquiry,
        ContrastInquiry,
        DefogLevelInquiry,
        DefogModeInquiry,
        DigitalInquiry,
        DigitalPtzInquiry,
        DynamicRangeInquiry,
        ExposureCompensationInquiry,
        ExposureCompensationModeInquiry,
        ExposureCompensationPositionInquiry,
        ExposureModeInquiry,
        FlickerModeInquiry,
        FlipStateInquiry,
        FocusModeInquiry,
        FocusNearLimitInquiry,
        FocusPositionInquiry,
        FocusRangeInquiry,
        FocusUnlockInquiry,
        FocusZoneInquiry,
        GainInquiry,
        GainLimitInquiry,
        GammaInquiry,
        HueInquiry,
        ImageFlipInquiry,
        IrisControlInquiry,
        IrisInquiry,
        LuminanceInquiry,
        MenuOpenCloseInquiry,
        MotionSyncModeInquiry,
        MotionSyncPresetInquiry,
        NdFilterInquiry,
        NdFilterPresetInquiry,
        NightDayModeInquiry,
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
        StandbyInquiry,
        TallyAutoAdjustInquiry,
        TallyGreenInquiry,
        TallyRedInquiry,
        TallyStatusInquiry,
        TwoToneModeInquiry,
        UsbAudioInquiry,
        VersionInquiry,
        WhiteBalanceModeInquiry,
        ZoomPositionInquiry,
    );
    row!(o, "PowerInquiry@cam7", PowerInquiry, CameraId::CAMERA_7);
    extra_rows(&mut o);

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

fn assert_golden(actual: &str) {
    const EXPECTED: &str = include_str!("../tests/fixtures/issue_715_wire_golden.txt");
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
        if metadata.command.is_some() {
            assert!(
                names.contains(metadata.name),
                "queryable inquiry {} is absent from the literal wire inventory",
                metadata.name,
            );
        }
    }
}

// ---- HEAD-specific prelude ----
use crate::command::encode::WireEncode as Enc;
fn dmc(a: u8, b: u8) -> Result<DirectMenuControl, Error> {
    DirectMenuControl::new(a, b)
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
