//! Every noun-table command is exercised through real request preparation.
#![allow(clippy::expect_used, clippy::panic)]

use crate::wire::WireValue;
use crate::{
    capabilities::{Capabilities, TypedSupportSurface},
    command::{
        self,
        semantics::{BuiltinCommand, BuiltinRequestContract},
        surface::{row_gate, RowGate},
    },
    noun_table::noun_table,
    prepared::test_support::{prepare_builtin_command, prepare_builtin_operation},
    request::builtin,
    types,
    units::Degrees,
    Error, OperationalTuning, ProfileSpec, UnitInterval, ZoomDomain,
};
use std::{any::TypeId, collections::HashSet};

trait Sample: Sized {
    fn sample(profile: &ProfileSpec) -> Self;

    fn values(profile: &ProfileSpec) -> Vec<Self> {
        vec![Self::sample(profile)]
    }

    fn inquiry_argument(&self) -> Option<u8> {
        None
    }
}

// The value's checked codec supplies the complete byte domain, including all
// closed enum variants. No second list of mode names or supported values is
// maintained here; profile admission is still exercised for every decoded
// value, even when that profile does not advertise it.
macro_rules! byte_samples {
    ($($ty:ty => $value:expr;)+) => {$(
        impl Sample for $ty {
            fn sample(_profile: &ProfileSpec) -> Self { $value }
            fn values(_profile: &ProfileSpec) -> Vec<Self> {
                (u8::MIN..=u8::MAX).filter_map(|raw| Self::from_wire(raw).ok()).collect()
            }
            fn inquiry_argument(&self) -> Option<u8> { Some((*self).to_wire()) }
        }
    )+};
}
byte_samples! {
    types::FocusMode => types::FocusMode::Auto;
    types::WhiteBalanceMode => types::WhiteBalanceMode::Auto;
    types::TallyMode => types::TallyMode::Off;
    types::TallyBrightness => types::TallyBrightness::Low;
    types::NoiseReduction2D => types::NoiseReduction2D::Off;
    types::NoiseReduction3D => types::NoiseReduction3D::Off;
    command::ExposureMode => command::ExposureMode::Auto;
    command::NdFilterMode => command::NdFilterMode::Variable;
    command::SharpnessMode => command::SharpnessMode::Manual;
    command::ImageFlipMode => command::ImageFlipMode::Off;
    command::NoiseReduction2DMode => command::NoiseReduction2DMode::Manual;
    types::PictureEffectMode => types::PictureEffectMode::Off;
    command::AutoFocusSensitivity => command::AutoFocusSensitivity::Normal;
    command::AutoWhiteBalanceSensitivity => command::AutoWhiteBalanceSensitivity::Normal;
    command::FocusZone => command::FocusZone::Center;
    command::AntiFlickerMode => command::AntiFlickerMode::Off;
    command::PanTiltLimitCorner => command::PanTiltLimitCorner::DownLeft;
    command::MenuButton => command::MenuButton::Release;
    command::DirectMenuTarget => command::DirectMenuTarget::NdFilter;
    command::PanTiltSpeedMode => command::PanTiltSpeedMode::Standard24;
    command::KneeMode => command::KneeMode::Auto;
    command::KneeParameter => command::KneeParameter::Setting;
    command::PresetSpeedSelection => command::PresetSpeedSelection::Separate;
    command::PresetSeparateMode => command::PresetSeparateMode::Speed;
    types::NdiQuality => types::NdiQuality::High;
    types::PresetSpeed => types::PresetSpeed::MIN;
    types::PresetNumber => types::PresetNumber::MIN;
    types::GainLimit => types::GainLimit::MIN;
    types::IrisStep => types::IrisStep::MIN;
    types::WhiteBalanceStep => types::WhiteBalanceStep::MIN;
    types::MemoryColorTemperatureStep => types::MemoryColorTemperatureStep::MIN;
    types::KneeSlope => types::KneeSlope::MIN;
    types::KneePoint => types::KneePoint::MIN;
    u8 => 0;
    types::ZoomSpeed => types::ZoomSpeed::new(1).expect("speed");
    types::PanSpeed => types::PanSpeed::new(1).expect("speed");
    types::TiltSpeed => types::TiltSpeed::new(1).expect("speed");
    types::FocusSpeed => types::FocusSpeed::new(1).expect("speed");
    types::ExposureCompensationLevel => types::ExposureCompensationLevel::new(0).expect("compensation");
    types::WideDynamicRangeLevel => types::WideDynamicRangeLevel::new(0).expect("range");
    types::IrisLevel => types::IrisLevel::new(0).expect("iris");
    types::BrightnessLevel => types::BrightnessLevel::new(0).expect("brightness");
    types::GainLevel => types::GainLevel::new(0).expect("gain");
    types::RgbTuning => types::RgbTuning::new(0).expect("tuning");
    types::RgbGain => types::RgbGain::new(0).expect("red");
    types::SaturationLevel => types::SaturationLevel::new(0).expect("saturation");
    types::HueLevel => types::HueLevel::new(0).expect("hue");
    types::LuminanceLevel => types::LuminanceLevel::new(0).expect("luminance");
    types::ContrastLevel => types::ContrastLevel::new(0).expect("contrast");
    types::GammaLevel => types::GammaLevel::new(0).expect("gamma");
    types::SharpnessLevel => types::SharpnessLevel::new(0).expect("sharpness");
    types::MotionSyncSpeed => types::MotionSyncSpeed::new(1).expect("speed");
    types::BlackStep => types::BlackStep::MIN;
}

impl Sample for bool {
    fn sample(_profile: &ProfileSpec) -> Self {
        false
    }
    fn values(_profile: &ProfileSpec) -> Vec<Self> {
        vec![false, true]
    }
}
macro_rules! samples {
    ($($ty:ty => $value:expr;)+) => {$(
        impl Sample for $ty {
            fn sample(_profile: &ProfileSpec) -> Self { $value }
        }
    )+};
}
samples! {
    Degrees => Degrees::new(0.0).expect("finite degrees");
    types::ZoomPosition => types::ZoomPosition::new(0).expect("zoom");
    types::SpeedLevel => types::SpeedLevel::Slow;
    types::NoiseReduction2DLevel => types::NoiseReduction2DLevel::new(1).expect("noise reduction");
    types::NoiseReduction3DLevel => types::NoiseReduction3DLevel::new(1).expect("noise reduction");
    command::NdFilterDensity => command::NdFilterDensity::new(0).expect("ND");
    types::WhiteBalanceOffset => types::WhiteBalanceOffset::MIN;
    types::WhiteBalanceGain => types::WhiteBalanceGain::MIN;
    types::BlackLevel => types::BlackLevel::MIN;
    types::HighResolutionZoomSpeed => types::HighResolutionZoomSpeed::MIN;
    std::time::Duration => std::time::Duration::from_secs(1);
}

impl Sample for UnitInterval {
    fn sample(_profile: &ProfileSpec) -> Self {
        Self::new(0.0).expect("unit interval")
    }
    fn values(_profile: &ProfileSpec) -> Vec<Self> {
        [0.0, 0.5, 1.0]
            .into_iter()
            .map(|value| Self::new(value).expect("unit interval"))
            .collect()
    }
}

impl Sample for ZoomDomain {
    fn sample(_profile: &ProfileSpec) -> Self {
        Self::Optical
    }
    fn values(_profile: &ProfileSpec) -> Vec<Self> {
        vec![Self::Optical, Self::OpticalPlusDigital]
    }
}

macro_rules! direction_samples {
    ($($ty:ty),+ $(,)?) => {$(
        impl Sample for $ty {
            fn sample(_profile: &ProfileSpec) -> Self { Self::Up }
            fn values(_profile: &ProfileSpec) -> Vec<Self> {
                static VALUES: std::sync::OnceLock<Vec<$ty>> = std::sync::OnceLock::new();
                VALUES.get_or_init(|| (u8::MIN..=u8::MAX)
                    .flat_map(|pan| (u8::MIN..=u8::MAX)
                        .filter_map(move |tilt| Self::from_wire((pan, tilt)).ok()))
                    .collect()).clone()
            }
        }
    )+};
}
direction_samples!(command::PanTiltDirection, command::MenuDirection);

impl Sample for types::PanTiltPosition {
    fn sample(_profile: &ProfileSpec) -> Self {
        Self::from_degrees(0.0, 0.0).expect("finite position")
    }
}

impl Sample for types::PanTiltSpeed {
    fn sample(profile: &ProfileSpec) -> Self {
        let domain = profile.capabilities().pan_tilt.as_ref();
        Self::new(
            types::PanSpeed::new(domain.map_or(1, |d| d.pan_speed.min())).expect("pan speed"),
            types::TiltSpeed::new(domain.map_or(1, |d| d.tilt_speed.min())).expect("tilt speed"),
        )
    }
}

impl<T: Sample> Sample for types::Speed<T> {
    fn sample(_profile: &ProfileSpec) -> Self {
        Self::Level(types::SpeedLevel::Slow)
    }
    fn values(profile: &ProfileSpec) -> Vec<Self> {
        (u8::MIN..=u8::MAX)
            .filter_map(|raw| types::SpeedLevel::from_wire(raw).ok())
            .map(Self::Level)
            .chain(T::values(profile).into_iter().map(Self::Exact))
            .collect()
    }
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
        .expect("profile focus position")
    }
}
impl Sample for types::ShutterSpeed {
    fn sample(profile: &ProfileSpec) -> Self {
        Self::new(
            profile
                .capabilities()
                .exposure
                .as_ref()
                .and_then(|domain| domain.shutter_codes.as_ref())
                .map_or(0, |domain| domain.min()),
        )
        .expect("profile shutter code")
    }

    fn values(_profile: &ProfileSpec) -> Vec<Self> {
        (u8::MIN..=u8::MAX)
            .filter_map(|raw| Self::from_wire(raw).ok())
            .collect()
    }
}

impl Sample for types::Kelvin {
    fn sample(profile: &ProfileSpec) -> Self {
        Self::new(
            profile
                .capabilities()
                .white_balance
                .as_ref()
                .and_then(crate::capabilities::WhiteBalanceCaps::color_temperature_range)
                .map_or(2500, |range| range.min()),
        )
        .expect("temperature")
    }
}

fn build(capabilities: Capabilities, source: &ProfileSpec, with_domains: bool) -> ProfileSpec {
    let grants = capabilities.typed_support;
    let mut builder = ProfileSpec::builder(capabilities)
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(if with_domains {
            source.position_inquiries()
        } else {
            None
        });
    for selection in source
        .encodings()
        .iter()
        .filter(|selection| grants.contains(selection.surface()))
    {
        builder = builder.encoding(selection);
    }
    builder.build().expect("validated gate test profile")
}

fn target_for(profile: &ProfileSpec) -> crate::TargetRegistration {
    crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile.clone())
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
// Expand the Cartesian product of the canonical method's argument domains.
// Most methods take one checked value; paired arguments retain their real
// constructor rather than an invented command/value inventory.
macro_rules! sample_arguments {
    ($profile:ident, [], $body:block) => { $body };
    ($profile:ident, [$arg:ident: $ty:ty $(, $rest:ident: $rest_ty:ty)*], $body:block) => {
        for $arg in <$ty as Sample>::values($profile) {
            sample_arguments!($profile, [$($rest: $rest_ty),*], $body);
        }
    };
}

macro_rules! inquiry_gate_of {
    ([with_decoder [$($request:tt)*] => $decoder:path]) => {
        inquiry_gate_of!([$($request)*])
    };
    ([checked $($request:tt)*]) => { inquiry_gate_of!([$($request)*]) };
    ([with_target |$target:ident| $($request:tt)*]) => { inquiry_gate_of!([$($request)*]) };
    ([with_profile |$profile:ident| $($request:tt)*]) => { inquiry_gate_of!([$($request)*]) };
    ([$root:ident::$noun:ident::$request:ident::$constructor:ident($($argument:tt)*)]) => {
        <$root::$noun::$request as crate::command::surface::BuiltinInquiryGate>::GATE
    };
    ([$request:ty]) => {
        <$request as crate::command::surface::BuiltinInquiryGate>::GATE
    };
}

#[derive(Default)]
struct SourceCoverage {
    selected: HashSet<(TypeId, BuiltinCommand)>,
    literal: HashSet<(TypeId, BuiltinCommand)>,
    denied: HashSet<BuiltinCommand>,
    surfaces: HashSet<TypedSupportSurface>,
    command_cases: usize,
    inquiry_cases: usize,
}

impl SourceCoverage {
    fn gate(&mut self, gate: RowGate) {
        if let RowGate::Typed(surfaces) | RowGate::DomainAndTyped(_, surfaces) = gate {
            self.surfaces.extend(surfaces.iter());
        }
    }
    fn record<C: BuiltinRequestContract + 'static>(&mut self, row: BuiltinCommand, frame: &[u8]) {
        self.command_cases += 1;
        self.selected.insert((TypeId::of::<C>(), row));
        if crate::issue_715_wire_golden::has_literal_frame(frame) {
            self.literal.insert((TypeId::of::<C>(), row));
        }
    }

    fn assert_family<C: BuiltinRequestContract + 'static>(&self) {
        for row in C::LEDGER_ROWS {
            if !row.source().is_sourced() {
                assert!(
                    !self.selected.contains(&(TypeId::of::<C>(), *row)),
                    "intentionally unsourced {row:?} was admitted by a built-in profile"
                );
                continue;
            }
            let granted = crate::profiles::ProfileId::all().iter().any(|id| {
                row_gate(*row)
                    .validate(&id.spec().expect("built-in profile"), "source family grant")
                    .is_ok()
            });
            if !granted {
                assert!(
                    !self.selected.contains(&(TypeId::of::<C>(), *row)),
                    "ungranted {row:?} was admitted by a built-in profile"
                );
                continue;
            }
            assert!(
                self.selected.contains(&(TypeId::of::<C>(), *row)),
                "{} declares {row:?} without an admitted public constructor witness",
                std::any::type_name::<C>()
            );
            if C::LEDGER_ROWS.len() > 1 {
                assert!(self.literal.contains(&(TypeId::of::<C>(), *row)),
                    "{} encoding/selector {row:?} has no admitted frame in the reviewed literal golden",
                    std::any::type_name::<C>());
            }
        }
    }

    fn check_other_encoding_targets<C: BuiltinRequestContract + 'static>(
        &mut self,
        request: &C,
        origin: &ProfileSpec,
        index: &crate::source_evidence::checked::Index<'_>,
    ) {
        if C::LEDGER_ROWS.len() > 1 {
            // A value constructed for one encoding remains a public request
            // value when submitted to another target. Its actual validator
            // must reject that use or select an independently sourced row.
            for id in crate::profiles::ProfileId::all() {
                if Some(*id) == origin.capabilities().profile_id {
                    continue;
                }
                let target = target_for(&id.spec().expect("built-in alternate target"));
                if let Some((row, frame)) =
                    crate::issue_715_wire_golden::admitted_command_source(request, &target, index)
                {
                    assert!(
                        row_gate(row)
                            .validate(target.profile(), "cross-target discovery")
                            .is_ok(),
                        "a value constructed for another target bypassed {row:?}'s discovery gate"
                    );
                    self.record::<C>(row, &frame);
                }
            }
        }
    }
}

macro_rules! source_agreement_row {
    (inquiry, [$key:ident], $ret:ty, [$($request:tt)*], [$($arg:ident: $ty:ty),*], $profile:ident, $denied:ident, $index:ident, $coverage:ident) => {{
        $coverage.gate(inquiry_gate_of!([$($request)*]));
        let registration = target_for($profile);
        let mut admitted = 0;
        sample_arguments!($profile, [$($arg: $ty),*], {
            if let Ok(request) = crate::noun_facade::noun_inquiry_request!($profile, &registration; $($request)*) {
                let expected_arguments: Vec<u8> = [None::<u8>, $(<$ty as Sample>::inquiry_argument(&$arg)),*]
                    .into_iter().flatten().collect();
                if crate::issue_715_wire_golden::admitted_inquiry_source(&request, &registration, &expected_arguments, &$index).is_some() {
                    admitted += 1;
                    $coverage.inquiry_cases += 1;
                    let prepared = crate::prepared::test_support::prepare_inquiry(
                        &request, &registration, OperationalTuning::new(), crate::prepared::ClassSelection::Request);
                    assert!(prepared.is_ok(), "{} inquiry {} passed validation but failed preparation: {prepared:?}",
                        $profile.name(), stringify!($key));
                    if inquiry_gate_of!([$($request)*]).validate(&$denied, "denied inquiry discovery").is_err() {
                        crate::command::inquiry_structs::reset_generated_inquiry_write_count();
                        let result = crate::prepared::test_support::prepare_inquiry(
                            &request, &target_for(&$denied), OperationalTuning::new(), crate::prepared::ClassSelection::Request);
                        assert!(result.is_err(), "{}: missing inquiry permission was admitted", stringify!($key));
                        assert_eq!(crate::command::inquiry_structs::generated_inquiry_write_count(), 0,
                            "{}: inquiry refusal must precede encoding", stringify!($key));
                    }
                }
            }
        });
        let expected = inquiry_gate_of!([$($request)*]).validate($profile, "canonical inquiry family").is_ok();
        assert_eq!(admitted > 0, expected,
            "{} inquiry {} must have a positive frame exactly when its discovery gate is granted",
            $profile.name(), stringify!($key));
    }};
    ($kind:ident, [$key:ident], $ret:ty, [$($request:tt)*], [$($arg:ident: $ty:ty),*], $profile:ident, $denied:ident, $index:ident, $coverage:ident) => {{
        for row in <$ret as BuiltinRequestContract>::LEDGER_ROWS { $coverage.gate(row_gate(*row)); }
        let registration = target_for($profile);
        let mut admitted = 0;
        sample_arguments!($profile, [$($arg: $ty),*], {
            if let Ok(request) = crate::noun_facade::noun_request!($profile, &registration; $ret; $($request)*) {
                if let Some((row, frame)) = crate::issue_715_wire_golden::admitted_command_source(&request, &registration, &$index) {
                    admitted += 1;
                    $coverage.record::<$ret>(row, &frame);
                    $coverage.check_other_encoding_targets(&request, $profile, &$index);
                    assert!(row_gate(row).validate_command(row, $profile, "admitted command discovery").is_ok(),
                        "{} {:?} admits a request its discovery gate refuses", $profile.name(), row);
                    let prepared = prepare!($kind, &request, $profile);
                    assert!(prepared.is_ok(), "{} {:?} passed target validation but failed class-specific preparation: {prepared:?}",
                        $profile.name(), row);
                    if row_gate(row).validate_command(row, &$denied, "denied command discovery").is_err() {
                        builtin::reset_request_write_count();
                        let result = prepare!($kind, &request, &$denied);
                        assert!(result.is_err(), "{row:?}: missing permissions admitted a command");
                        assert_eq!(builtin::request_write_count(), 0, "{row:?}: refusal must precede encoding");
                        $coverage.denied.insert(row);
                    }
                }
            }
        });
        let expected = <$ret as BuiltinRequestContract>::LEDGER_ROWS.iter()
            .any(|row| row_gate(*row).validate_command(*row, $profile, "canonical command family").is_ok());
        assert_eq!(admitted > 0, expected,
            "{} {} must have a positive frame exactly when its discovery gate is granted",
            $profile.name(), std::any::type_name::<$ret>());
    }};
}

macro_rules! family_coverage {
    (inquiry, $ret:ty, $coverage:ident) => {};
    ($kind:ident, $ret:ty, $coverage:ident) => {
        $coverage.assert_family::<$ret>();
    };
}

macro_rules! gate_tests {
    ($(@noun $noun:ident { $($header:tt)* };
       $($(#[$row_doc:meta])* $kind:ident [$key:ident] $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty $(where $rgate:ident $(+ $extra:ident)*)? = [$($request:tt)*];)*)*
    ) => {
        #[test]
        fn every_preflight_admitted_builtin_value_has_exact_positive_source_and_literal_family_coverage() {
            let index = crate::source_evidence::checked::Index::parse(include_str!("../docs/visca_reference.md"))
                .expect("checked positive source table");
            let first = crate::profiles::ProfileId::all()[0].spec().expect("builtin profile");
            let denied = build(Capabilities::runtime_baseline("No control domains", 1).expect("baseline"), &first, false);
            let mut coverage = SourceCoverage::default();
            for id in crate::profiles::ProfileId::all() {
                let profile = id.spec().expect("built-in individual profile");
                let profile = &profile;
                $($(source_agreement_row!($kind, [$key], $ret, [$($request)*], [$($arg: $ty),*], profile, denied, index, coverage);)*)*
            }
            $($(family_coverage!($kind, $ret, coverage);)*)*
            // This permission refines the OpticalPlusDigital argument rather
            // than a whole row; the generated samples exercise both domains.
            coverage.surfaces.insert(TypedSupportSurface::DigitalZoomRange);
            assert_eq!(coverage.surfaces, TypedSupportSurface::ALL.into_iter().collect(),
                "every declared typed permission must gate a canonical row or argument");
            for (_, row) in &coverage.selected {
                if row_gate(*row).validate_command(*row, &denied, "denied command discovery").is_err() {
                    assert!(coverage.denied.contains(row), "{row:?}: missing negative preflight witness");
                }
            }
            eprintln!("source admission proof: {} command cases, {} inquiry cases, {} request-type/row witnesses, {} literal witnesses",
                coverage.command_cases, coverage.inquiry_cases, coverage.selected.len(), coverage.literal.len());
        }
    };
}
noun_table!(gate_tests);

#[test]
fn mutated_specs_cannot_advertise_unpreparable_facts() {
    use crate::capabilities::{CapabilityRange, PresetRecallSpeedEncoding};
    for id in crate::profiles::ProfileId::all() {
        let mut baseline = id.raw_facts();
        baseline.capabilities.profile_id = None;
        ProfileSpec::from_facts(baseline.clone()).expect("exported builtin facts");
        for mutation in 0..6 {
            let mut facts = baseline.clone();
            let caps = &mut facts.capabilities;
            let field = match mutation {
                0 if caps.supports(TypedSupportSurface::Gain) => {
                    caps.exposure.as_mut().expect("gain domain").gain_range = None;
                    "gain_range"
                }
                1 => {
                    caps.zoom.as_mut().expect("zoom domain").optical_max = 0x8000;
                    "optical_max"
                }
                2 => {
                    caps.zoom.as_mut().expect("zoom domain").digital_max = Some(0x8000);
                    "digital_max"
                }
                3 if caps.supports(TypedSupportSurface::PresetRecallSpeed) => {
                    caps.presets.as_mut().expect("presets").recall_speed_range = None;
                    "typed_support"
                }
                4 if facts.encodings.preset_recall_speed()
                    == Some(PresetRecallSpeedEncoding::PtzOptics) =>
                {
                    caps.presets.as_mut().expect("presets").recall_speed_range =
                        Some(CapabilityRange::<u8>::new(1, 50).expect("valid capability bounds"));
                    "recall_speed_range"
                }
                5 => {
                    caps.typed_support = caps
                        .typed_support
                        .union(crate::capabilities::TypedSupportSet::from_surface(
                            TypedSupportSurface::TallyBrightness,
                        ))
                        .without(TypedSupportSurface::Tally);
                    "typed_support"
                }
                _ => continue,
            };
            let error = ProfileSpec::from_facts(facts)
                .expect_err("mutated discovery must not promise an unpreparable request");
            assert!(
                error.to_string().contains(field),
                "{id:?} mutation {mutation}: {error}"
            );
        }
    }
}

#[test]
fn sharpness_level_and_mode_have_independent_model_permissions() {
    for profile in [
        ProfileSpec::from_compile_time::<crate::profiles::SonyFr7>().expect("FR7"),
        ProfileSpec::from_compile_time::<crate::profiles::SonyEviH100>().expect("EVI"),
    ] {
        assert!(profile
            .capabilities()
            .supports(TypedSupportSurface::Sharpness));
        assert!(!profile
            .capabilities()
            .supports(TypedSupportSurface::SharpnessMode));
        let registration = target_for(&profile);
        let level = command::Sharpness::SetLevel {
            value: types::SharpnessLevel::new(5).expect("level"),
        };
        assert!(prepare_builtin_command(&level, &registration, OperationalTuning::new()).is_ok());
        assert!(crate::Request::validate_for_target(
            &command::SharpnessPositionInquiry,
            &registration
        )
        .is_ok());
        builtin::reset_request_write_count();
        let mode = command::Sharpness::Mode(command::SharpnessMode::Auto);
        assert!(matches!(
            prepare_builtin_command(&mode, &registration, OperationalTuning::new()),
            Err(Error::FeatureNotSupported { .. })
        ));
        assert_eq!(builtin::request_write_count(), 0);
        assert!(matches!(
            crate::Request::validate_for_target(&command::SharpnessModeInquiry, &registration),
            Err(Error::FeatureNotSupported { .. })
        ));
    }
}

#[test]
fn parameterized_preset_inquiries_share_command_bounds_before_encoding() {
    let full = crate::profiles::ProfileId::SonyFr7.spec().expect("FR7");
    let mut capabilities = full.capabilities().clone();
    capabilities.profile_id = None;
    capabilities
        .presets
        .as_mut()
        .expect("preset domain")
        .highest = 5;
    let profile = build(capabilities, &full, true);
    let registration = target_for(&profile);
    macro_rules! check {
        ($query:ident) => {{
            let admitted = command::$query::new(types::PresetNumber::new(5).expect("last preset"));
            assert!(crate::prepared::test_support::prepare_inquiry(
                &admitted,
                &registration,
                OperationalTuning::new(),
                crate::prepared::ClassSelection::Request
            )
            .is_ok());
            let denied =
                command::$query::new(types::PresetNumber::new(6).expect("wire-valid preset"));
            crate::command::inquiry_structs::reset_generated_inquiry_write_count();
            assert!(matches!(
                crate::prepared::test_support::prepare_inquiry(
                    &denied,
                    &registration,
                    OperationalTuning::new(),
                    crate::prepared::ClassSelection::Request
                ),
                Err(Error::ParameterOutOfRange {
                    parameter: "preset number",
                    value: 6,
                    min: 0,
                    max: 5
                })
            ));
            assert_eq!(
                crate::command::inquiry_structs::generated_inquiry_write_count(),
                0
            );
        }};
    }
    check!(Fr7SeparatePresetModeInquiry);
    check!(Fr7SeparatePresetSpeedInquiry);
    check!(Fr7SeparatePresetDurationInquiry);
    let preset = types::PresetNumber::new(6).expect("wire-valid preset");
    assert!(prepare_builtin_command(
        &command::Fr7SetSeparatePresetSpeed::new(preset, types::PresetSpeed::MIN),
        &registration,
        OperationalTuning::new()
    )
    .is_err());
}
