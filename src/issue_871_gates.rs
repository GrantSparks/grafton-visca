//! Every noun-table command is exercised through real request preparation.
#![allow(clippy::expect_used, clippy::panic)]

use crate::{
    capabilities::{Capabilities, TypedSupportSet, TypedSupportSurface},
    command::{
        self,
        semantics::BuiltinCommand,
        surface::{row_gate, DomainGate, RowGate},
    },
    noun_table::noun_table,
    prepared::test_support::{prepare_builtin_command, prepare_builtin_operation},
    request::builtin,
    types,
    units::Degrees,
    Error, OperationalTuning, PositionInquirySupport, ProfileSpec, UnitInterval, ZoomDomain,
};
use std::collections::HashSet;

trait Sample: Sized {
    fn sample(profile: &ProfileSpec) -> Self;
}
macro_rules! samples {
    ($($ty:ty => $value:expr;)+) => {$(
        impl Sample for $ty {
            fn sample(_profile: &ProfileSpec) -> Self { $value }
        }
    )+};
}
samples! {
    bool => false;
    u8 => 0;
    Degrees<f32> => Degrees(0.0);
    UnitInterval => UnitInterval::new(0.0).expect("unit interval");
    ZoomDomain => ZoomDomain::Optical;
    types::ZoomSpeed => types::ZoomSpeed::new(1).expect("speed");
    types::ZoomPosition => types::ZoomPosition::new(0).expect("zoom");
    types::PanSpeed => types::PanSpeed::new(1).expect("speed");
    types::TiltSpeed => types::TiltSpeed::new(1).expect("speed");
    types::FocusSpeed => types::FocusSpeed::new(1).expect("speed");
    types::SpeedLevel => types::SpeedLevel::Slow;
    command::PanTiltDirection => command::PanTiltDirection::Up;
    command::PanTiltLimitCorner => command::PanTiltLimitCorner::DownLeft;
    command::FocusZone => command::FocusZone::Center;
    command::AutoFocusSensitivity => command::AutoFocusSensitivity::Normal;
    command::FocusLock => command::FocusLock::Off;
    command::PresetNumber => command::PresetNumber::new(0).expect("preset");
    command::PresetRecallSpeed => command::PresetRecallSpeed::new(1).expect("speed");
    command::ExposureMode => command::ExposureMode::Auto;
    types::ExposureCompensationLevel => types::ExposureCompensationLevel::new(0).expect("compensation");
    types::DynamicRangeLevel => types::DynamicRangeLevel::new(0).expect("range");
    types::IrisLevel => types::IrisLevel::new(0).expect("iris");
    types::BrightnessLevel => types::BrightnessLevel::new(0);
    types::GainLevel => types::GainLevel::new(0).expect("gain");
    types::GainLimit => types::GainLimit::new(0).expect("gain limit");
    command::AntiFlickerMode => command::AntiFlickerMode::Off;
    command::AutoWhiteBalanceSensitivity => command::AutoWhiteBalanceSensitivity::Normal;
    types::RedTuning => types::RedTuning::new(0).expect("tuning");
    types::BlueTuning => types::BlueTuning::new(0).expect("tuning");
    types::RedChannel => types::RedChannel::new(0).expect("red");
    types::BlueChannel => types::BlueChannel::new(0).expect("blue");
    types::SaturationLevel => types::SaturationLevel::new(0).expect("saturation");
    types::HueLevel => types::HueLevel::new(0).expect("hue");
    types::LuminanceLevel => types::LuminanceLevel::new(0).expect("luminance");
    types::ContrastLevel => types::ContrastLevel::new(0).expect("contrast");
    types::GammaLevel => types::GammaLevel::new(0).expect("gamma");
    command::SharpnessMode => command::SharpnessMode::Manual;
    types::SharpnessLevel => types::SharpnessLevel::new(0).expect("sharpness");
    command::NoiseReduction2DMode => command::NoiseReduction2DMode::Manual;
    types::NoiseReduction2DLevel => types::NoiseReduction2DLevel::new(1).expect("noise reduction");
    types::NoiseReduction3DLevel => types::NoiseReduction3DLevel::new(1).expect("noise reduction");
    command::ImageFlipMode => command::ImageFlipMode::Off;
    command::PictureEffectMode => command::PictureEffectMode::Off;
    command::NdFilterMode => command::NdFilterMode::Variable;
    command::NdFilterValue => command::NdFilterValue::new(0).expect("ND");
    command::MotionSyncMode => command::MotionSyncMode::Off;
    types::MotionSyncSpeed => types::MotionSyncSpeed::new(1).expect("speed");
    command::MenuDirection => command::MenuDirection::Up;
    command::DirectMenuTarget => command::DirectMenuTarget::NdFilter;
    command::MenuButton => command::MenuButton::Release;
    types::NdiQuality => types::NdiQuality::High;
    command::VariableSpeedMode => command::VariableSpeedMode::Standard24;
    types::IrisStep => types::IrisStep::MIN;
    types::WhiteKelvin => types::WhiteKelvin::MIN;
    types::WhiteBalanceStep => types::WhiteBalanceStep::MIN;
    types::MemoryColorTemperatureStep => types::MemoryColorTemperatureStep::MIN;
    types::WhiteBalanceGain => types::WhiteBalanceGain::MIN;
    types::TintOffset => types::TintOffset::MIN;
    types::BlackStep => types::BlackStep::MIN;
    types::BlackLevel => types::BlackLevel::MIN;
    types::KneeSlope => types::KneeSlope::MIN;
    types::KneePoint => types::KneePoint::MIN;
    types::HighResolutionZoomSpeed => types::HighResolutionZoomSpeed::MIN;
    types::Fr7PresetNumber => types::Fr7PresetNumber::MIN;
    types::PresetSpeed => types::PresetSpeed::MIN;
    types::PresetDuration => types::PresetDuration::MIN;
    command::Fr7KneeMode => command::Fr7KneeMode::Auto;
    command::Fr7KneeParameter => command::Fr7KneeParameter::Setting;
    command::Fr7PresetSpeedSelection => command::Fr7PresetSpeedSelection::Separate;
    command::Fr7PresetSeparateMode => command::Fr7PresetSeparateMode::Speed;
}
impl Sample for types::FocusPosition {
    fn sample(profile: &ProfileSpec) -> Self {
        Self::new(
            profile
                .capabilities()
                .focus
                .as_ref()
                .map_or(0, |domain| domain.range.min()),
        )
    }
}
impl Sample for types::ShutterSpeed {
    fn sample(profile: &ProfileSpec) -> Self {
        Self::new(
            profile
                .capabilities()
                .exposure
                .as_ref()
                .and_then(|domain| domain.shutter_speeds.first())
                .map_or(0, |entry| entry.value),
        )
    }
}

impl Sample for types::ColorTemp {
    fn sample(profile: &ProfileSpec) -> Self {
        Self::from_kelvin(
            profile
                .capabilities()
                .white_balance
                .as_ref()
                .and_then(|domain| domain.color_temp_range)
                .map_or(2500, |range| range.min()),
        )
        .expect("temperature")
    }
}

fn build(capabilities: Capabilities, source: &ProfileSpec, with_domains: bool) -> ProfileSpec {
    ProfileSpec::builder(capabilities)
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(PositionInquirySupport::new_with_iris_nd(
            with_domains,
            with_domains,
            with_domains,
            with_domains,
            with_domains,
        ))
        .build()
        .expect("validated gate test profile")
}

fn profiles() -> (ProfileSpec, ProfileSpec, ProfileSpec) {
    let source = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
    let mut caps = source.capabilities().clone();
    caps.profile_id = None;
    caps.model_name = "All documented row gates".into();
    caps.typed_support = TypedSupportSet::from_surfaces(&TypedSupportSurface::ALL);
    caps.zoom.as_mut().expect("zoom").digital_max = Some(0x7000);
    caps.focus.as_mut().expect("focus").one_push = true;
    caps.focus.as_mut().expect("focus").af_sensitivity = true;
    caps.focus.as_mut().expect("focus").near_limit_inquiry = true;
    caps.tally = true;
    caps.direct_menu = true;
    caps.nd_filter_mode = crate::capabilities::NdFilterMode::Variable;
    caps.pan_tilt.as_mut().expect("pan/tilt").variable_speed = true;
    caps.pan_tilt
        .as_mut()
        .expect("pan/tilt")
        .motion_sync_speed_range = Some(crate::capabilities::CapabilityRange::<u8>::new(1, 24));
    caps.white_balance
        .as_mut()
        .expect("white balance")
        .modes
        .push(command::WhiteBalanceMode::ATW);
    caps.white_balance
        .as_mut()
        .expect("white balance")
        .modes
        .push(command::WhiteBalanceMode::Preset);
    let full = build(caps, &source, true);
    let none = build(
        Capabilities::runtime_baseline("No control domains", 1).expect("baseline"),
        &source,
        false,
    );
    let mut caps = full.capabilities().clone();
    caps.typed_support = TypedSupportSet::empty();
    let no_typed = build(caps, &source, true);
    (full, none, no_typed)
}
fn target_for(profile: &ProfileSpec) -> crate::TargetRegistration {
    crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile.clone())
        .expect("registered gate-test target")
}

macro_rules! prepare {
    (plain, $request:expr, $profile:expr) => {
        prepare_builtin_command($request, &target_for($profile), OperationalTuning::new()).map(drop)
    };
    (applied, $request:expr, $profile:expr) => {
        prepare_builtin_operation::<crate::completion::AppliedOnly, _>(
            $request,
            &target_for($profile),
            OperationalTuning::new(),
        )
        .map(drop)
    };
    (targeted, $request:expr, $profile:expr) => {
        prepare_builtin_operation::<crate::completion::Targeted, _>(
            $request,
            &target_for($profile),
            OperationalTuning::new(),
        )
        .map(drop)
    };
}
macro_rules! assert_selected_row {
    ($request:ident, $row:ident, [by_value $($tokens:tt)*]) => {
        assert_eq!(
            $request.ledger_row(),
            BuiltinCommand::$row,
            "evaluated by-value request must agree with its constant witness"
        );
    };
    ($request:ident, $row:ident, $other:tt) => {};
}
macro_rules! exercise_row {
    ([], $($rest:tt)*) => {};
    ([$row:ident], $kind:ident, $ret:ty, [$($request:tt)*], [$($arg:ident: $ty:ty),*], $full:ident, $none:ident, $no_typed:ident, $covered:ident) => {{
        $(let $arg = <$ty as Sample>::sample(&$full);)*
        let _registration = target_for(&$full);
        let request = crate::noun_facade::noun_request!(&$full, &_registration; $ret; $($request)*).expect(stringify!($row));
        assert_selected_row!(request, $row, [$($request)*]);
        assert!(prepare!($kind, &request, &$full).is_ok(), "{:?}: positive preparation", BuiltinCommand::$row);
        $covered.insert(BuiltinCommand::$row);
        if !matches!(row_gate(BuiltinCommand::$row), RowGate::Always | RowGate::Domain(DomainGate::MenuControl)) {
            let denied_profile = match row_gate(BuiltinCommand::$row) {
                RowGate::Typed(_) => &$no_typed,
                RowGate::Always | RowGate::Domain(_) => &$none,
            };
            builtin::reset_request_write_count();
            let denied = prepare!($kind, &request, denied_profile);
            assert_eq!(builtin::request_write_count(), 0, "{:?}: denied before encoding", BuiltinCommand::$row);
            assert!(matches!(denied, Err(Error::FeatureNotSupported { .. })), "{:?}: negative preparation: {denied:?}", BuiltinCommand::$row);
        }
    }};
}
macro_rules! inquiry_gate_of {
    ([$module:ident :: $request:ident :: new($($argument:expr),*)]) => {
        <$module::$request as crate::command::surface::BuiltinInquiryGate>::GATE
    };
    ([$request:ty]) => {
        <$request as crate::command::surface::BuiltinInquiryGate>::GATE
    };
}
macro_rules! collect_surfaces {
    (inquiry, [], [$($request:tt)*], $seen:ident) => {
        if let RowGate::Typed(surfaces) = inquiry_gate_of!([$($request)*]) {
            $seen.extend(surfaces.iter());
        }
    };
    ($kind:ident, [$row:ident], $request:tt, $seen:ident) => {
        if let RowGate::Typed(surfaces) = row_gate(BuiltinCommand::$row) {
            $seen.extend(surfaces.iter());
        }
    };
    ($kind:ident, [], $request:tt, $seen:ident) => {};
}

macro_rules! agreement_row {
    (inquiry, [], $ret:ty, [$($request:tt)*], [$($arg:ident: $ty:ty),*], $profile:ident) => {{
        $(let $arg = <$ty as Sample>::sample($profile);)*
        let gate = inquiry_gate_of!([$($request)*]);
        if let RowGate::Typed(surfaces) = gate {
            let expected = surfaces.iter().all(|surface| $profile.capabilities().supports_typed(surface));
            let registration = target_for($profile);
            assert_eq!(crate::Request::validate_for_target(&$($request)*, &registration).is_ok(), expected,
                "{} inquiry {}", $profile.name(), stringify!($($request)*));
        }
    }};
    ($kind:ident, [$row:ident], $ret:ty, [$($request:tt)*], [$($arg:ident: $ty:ty),*], $profile:ident) => {{
        if let RowGate::Typed(surfaces) = row_gate(BuiltinCommand::$row) {
            $(let $arg = <$ty as Sample>::sample($profile);)*
            let registration = target_for($profile);
            let request = crate::noun_facade::noun_request!($profile, &registration; $ret; $($request)*).expect(stringify!($row));
            let expected = surfaces.iter().all(|surface| $profile.capabilities().supports_typed(surface));
            assert_eq!(crate::Request::validate_for_target(&request, &registration).is_ok(), expected,
                "{} command {:?}", $profile.name(), BuiltinCommand::$row);
        }
    }};
    ($kind:ident, [], $ret:ty, $request:tt, $args:tt, $profile:ident) => {};
}

macro_rules! gate_tests {
    ($(@noun $noun:ident { accessor: $accessor:ident, getter: $getter:ident, dyn_trait: $dyn_trait:ident, gate: $gate:tt, doc: $doc:literal $(,)? };
       $($(#[$row_doc:meta])* $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty $(where $rgate:ident $(+ $extra:ident)*)? = [$($request:tt)*];)*)*
       @exceptions; $($exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty;)*) => {
        #[test]
        fn every_builtin_and_mutated_spec_agrees_with_discovery_for_typed_rows() {
            let (full, none, no_typed) = profiles();
            let profiles = [
                ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2"),
                ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG3>().expect("G3"),
                ProfileSpec::from_compile_time::<crate::profiles::PtzOptics30X>().expect("30X"),
                ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("FR7"),
                ProfileSpec::from_compile_time::<crate::profiles::SonyBRCH900>().expect("H900"),
                ProfileSpec::from_compile_time::<crate::profiles::SonyEVIH100>().expect("EVI"),
                ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().expect("BRC300"),
                ProfileSpec::from_compile_time::<crate::profiles::NearusBRC300>().expect("Nearus"),
                ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().expect("generic"),
                full, none, no_typed,
            ];
            for profile in &profiles {
                $($(agreement_row!($kind, [$($command)?], $ret, [$($request)*], [$($arg: $ty),*], profile);)*)*
            }
        }

        #[test]
        fn every_command_row_admits_its_gate_and_refuses_missing_permissions_before_encoding() {
            let (full, none, no_typed) = profiles();
            let mut covered = HashSet::new();
            let mut surfaces = HashSet::new();
            $($(collect_surfaces!($kind, [$($command)?], [$($request)*], surfaces);)*)*
            $($(exercise_row!([$($command)?], $kind, $ret, [$($request)*], [$($arg: $ty),*], full, none, no_typed, covered);)*)*
            // Optical-plus-digital zoom refines an argument value rather than
            // a whole row; ZoomTarget validates that extra permission.
            surfaces.insert(TypedSupportSurface::DigitalZoomRange);
            assert_eq!(surfaces, TypedSupportSurface::ALL.into_iter().collect(),
                "every declared typed permission must gate a reachable noun row");
            for row in BuiltinCommand::ALL {
                if !matches!(row_gate(*row), RowGate::Always) {
                    assert!(covered.contains(row), "unexercised command gate: {row:?}");
                }
            }
        }
    };
}
noun_table!(gate_tests);
