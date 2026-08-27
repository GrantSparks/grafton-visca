//! Mechanical preservation inventory for issue #548 (Phase 1 of #542).

pub use grafton_visca::Error;
pub mod error {
    pub use grafton_visca::Error;
}
use grafton_visca::{
    camera::profiles::ProfileId, capabilities::TypedSupportSurface, CameraId, Inquiry, Request,
    ViscaEnum, ViscaInquiry, ViscaValue,
};

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
struct InventoryInquiry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
enum InventoryEnum {
    First = 1,
    Second = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "1", max = "2")]
struct InventoryValue(u8);

const EXPECTED_PROFILES: &[&str] = &[
    "PtzOpticsG2",
    "PtzOpticsG3",
    "PtzOptics30X",
    "SonyFr7",
    "SonyBrcH900",
    "SonyEviH100",
    "SonyBrc300",
    "NearusBrc300",
    "GenericVisca",
];

const EXPECTED_TYPED_GATES: &[&str] = &[
    "DirectZoom",
    "DigitalZoomToggle",
    "DigitalZoomRange",
    "IrisControl",
    "OnePushFocus",
    "PtzOpticsSnapFocus",
    "FocusLock",
    "PushAutoFocus",
    "FocusZone",
    "AutoFocusSensitivity",
    "FocusNearLimitInquiry",
    "BacklightCompensation",
    "WideDynamicRange",
    "ExposureCompensation",
    "BrightnessControl",
    "OnePushWhiteBalance",
    "AutoTrackingWhiteBalance",
    "AutoWhiteBalanceSensitivity",
    "ColorTemperature",
    "RgbGain",
    "RgbTuning",
    "ImageFlip",
    "ImageMirror",
    "CombinedImageFlip",
    "ContrastControl",
    "SharpnessControl",
    "SaturationControl",
    "HueControl",
    "LuminanceControl",
    "GammaControl",
    "NoiseReduction",
    "NoiseReduction2D",
    "NoiseReduction3D",
    "PictureEffect",
    "Tally",
    "DirectMenu",
    "NdFilter",
    "VariableSpeed",
    "MotionSync",
];

const EXPECTED_ACCESSORS: &[&str] = &[
    "AdvancedAccessor",
    "ExposureAccessor",
    "FocusAccessor",
    "ImageAccessor",
    "MenuAccessor",
    "MotionSyncAccessor",
    "NdFilterAccessor",
    "PanTiltAccessor",
    "PowerAccessor",
    "PresetsAccessor",
    "SystemAccessor",
    "TallyAccessor",
    "WhiteBalanceAccessor",
    "ZoomAccessor",
];

const EXPECTED_BLOCKING_ACCESSORS: &[&str] = &[
    "AdvancedAccessor",
    "ExposureAccessor",
    "FocusAccessor",
    "ImageAccessor",
    "MenuAccessor",
    "MotionSyncAccessor",
    "NdFilterAccessor",
    "PanTiltAccessor",
    "PowerAccessor",
    "PresetsAccessor",
    "SystemAccessor",
    "TallyAccessor",
    "WhiteBalanceAccessor",
    "ZoomAccessor",
];

#[allow(dead_code)]
const EXPECTED_CONTROL_TRAITS: &[&str] = &[
    "AutoFocusSensitivityControl",
    "AutoFocusSensitivityInquiryControl",
    "AutoTrackingWhiteBalanceControl",
    "AutoWhiteBalanceSensitivityControl",
    "AutoWhiteBalanceSensitivityInquiryControl",
    "BacklightCompensationControl",
    "BacklightCompensationInquiryControl",
    "BrightnessControl",
    "BrightnessInquiryControl",
    "ColorControl",
    "ColorTemperatureControl",
    "ColorTemperatureInquiryControl",
    "ContrastControl",
    "ContrastInquiryControl",
    "DigitalZoomControl",
    "DigitalZoomRangeControl",
    "DirectMenuControl",
    "DirectZoomControl",
    "ExposureCompensationControl",
    "ExposureCompensationInquiryControl",
    "ExposureControl",
    "FocusControl",
    "FocusLockControl",
    "FocusNearLimitInquiryControl",
    "FocusRangeInquiryControl",
    "FocusZoneControl",
    "FocusZoneInquiryControl",
    "GammaControl",
    "GammaInquiryControl",
    "HueControl",
    "HueInquiryControl",
    "ImageFlipControl",
    "ImageFlipInquiryControl",
    "ImageFlipModeControl",
    "ImageMirrorControl",
    "InquiryControl",
    "IrisControl",
    "IrisInquiryControl",
    "LuminanceControl",
    "LuminanceInquiryControl",
    "MenuControl",
    "MotionControl",
    "MotionSyncControl",
    "NdFilterControl",
    "NdFilterInquiryControl",
    "NoiseReduction2DControl",
    "NoiseReduction2DInquiryControl",
    "NoiseReduction3DControl",
    "NoiseReduction3DInquiryControl",
    "NoiseReductionInquiryControl",
    "OnePushFocusControl",
    "OnePushWhiteBalanceControl",
    "PanTiltControl",
    "PanTiltInquiryControl",
    "PictureEffectControl",
    "PictureEffectInquiryControl",
    "PowerControl",
    "PresetsControl",
    "PushAFControl",
    "RgbGainControl",
    "RgbGainInquiryControl",
    "RgbTuningControl",
    "RgbTuningInquiryControl",
    "SaturationControl",
    "SaturationInquiryControl",
    "SharpnessControl",
    "SharpnessInquiryControl",
    "SnapFocusControl",
    "StreamingControl",
    "SystemControl",
    "TallyControl",
    "VariableSpeedControl",
    "WhiteBalanceControl",
    "WideDynamicRangeControl",
    "WideDynamicRangeInquiryControl",
    "ZoomControl",
];

const EXPECTED_DYN_TRAITS: &[&str] = &[
    "DynAdvanced",
    "DynAppliedRequest",
    "DynExposure",
    "DynFocus",
    "DynImage",
    "DynMenu",
    "DynMotion",
    "DynMotionSync",
    "DynNdFilter",
    "DynPanTilt",
    "DynPower",
    "DynPresets",
    "DynSessionCameraControl",
    "DynSessionCameraNouns",
    "DynSystem",
    "DynTally",
    "DynTargetedRequest",
    "DynWhiteBalance",
    "DynZoom",
];

const EXPECTED_DYN_NOUN_METHOD_COUNTS: &[(&str, usize)] = &[
    ("DynPower", 3),
    ("DynZoom", 8),
    ("DynSystem", 2),
    ("DynPanTilt", 9),
    ("DynFocus", 24),
    ("DynPresets", 4),
    ("DynExposure", 43),
    ("DynWhiteBalance", 30),
    ("DynImage", 40),
    ("DynTally", 13),
    ("DynNdFilter", 8),
    ("DynMotionSync", 4),
    ("DynMenu", 6),
    ("DynAdvanced", 15),
    ("DynMotion", 3),
];

fn public_trait_names(sources: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for source in sources {
        for line in source.lines() {
            let line = line.trim_start();
            let Some(rest) = line.strip_prefix("pub trait ") else {
                continue;
            };
            let name = rest
                .split(|ch: char| ch == '<' || ch == ':' || ch.is_whitespace())
                .next()
                .unwrap_or_default();
            if !name.is_empty() {
                names.push(name.to_owned());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[allow(dead_code)]
fn macro_and_struct_names(source: &str, suffix: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("pub struct ") {
            let name = rest
                .split(|ch: char| ch == '<' || ch.is_whitespace())
                .next()
                .unwrap_or_default();
            if name.ends_with(suffix) {
                names.push(name.to_owned());
            }
        } else if line.ends_with(suffix)
            && line
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            names.push(line.to_owned());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Reads `pub const NAME: usize = N;` without depending on its formatting.
fn declared_usize(source: &str, name: &str) -> usize {
    let needle = format!("{name}: usize");
    source
        .split_once(needle.as_str())
        .and_then(|(_, rest)| rest.split_once('='))
        .and_then(|(_, rest)| rest.split_once(';'))
        .and_then(|(value, _)| value.trim().parse().ok())
        .unwrap_or_else(|| panic!("no usize constant named {name}"))
}

/// Rows listed in the closed `BuiltinCommand::ALL` slice.
fn ledger_rows(semantics: &str) -> usize {
    semantics
        .rsplit_once("pub const ALL: &[Self] = &[")
        .and_then(|(_, rest)| rest.split_once("    ];"))
        .map(|(slice, _)| slice.matches("Self::").count())
        .expect("BuiltinCommand::ALL slice")
}

fn trait_methods(source: &str, traits: &[&str]) -> Vec<String> {
    let mut methods = Vec::new();
    let mut selected = false;
    let mut depth = 0_i32;

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !selected && trimmed.starts_with("pub trait ") {
            let name = trimmed["pub trait ".len()..]
                .split(|ch: char| ch == '<' || ch == ':' || ch.is_whitespace())
                .next()
                .unwrap_or_default();
            selected = traits.contains(&name);
        }
        if selected {
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                if let Some(name) = rest.split('(').next() {
                    methods.push(name.trim().to_owned());
                }
            }
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth == 0 {
                selected = false;
            }
        }
    }

    methods.sort();
    methods.dedup();
    methods
}

#[test]
fn built_in_profile_and_transport_inventory_is_closed() {
    let actual: Vec<_> = ProfileId::all()
        .iter()
        .map(|profile| format!("{profile:?}"))
        .collect();
    assert_eq!(actual, EXPECTED_PROFILES);

    for profile in ProfileId::all() {
        let raw = !profile.uses_sony_encapsulation();
        assert_eq!(profile.supports_tcp(), raw, "{profile:?} TCP drift");
        assert!(profile.supports_udp(), "{profile:?} must retain UDP");
        assert_eq!(profile.supports_serial(), raw, "{profile:?} serial drift");
        assert_eq!(profile.default_tcp_port(), raw.then_some(5678));
        assert_eq!(
            profile.default_udp_port(),
            Some(if raw { 1259 } else { 52381 })
        );
    }
}

#[test]
fn typed_capability_gate_inventory_is_closed() {
    let actual: Vec<_> = TypedSupportSurface::ALL
        .iter()
        .map(|gate| format!("{gate:?}"))
        .collect();
    assert_eq!(actual, EXPECTED_TYPED_GATES);
}

#[test]
fn static_noun_and_control_inventory_is_closed() {
    let async_nouns = include_str!("../src/async_nouns.rs");
    let blocking_nouns = include_str!("../src/blocking_nouns.rs");
    let surface = include_str!("../src/command/surface.rs");

    for accessor in EXPECTED_ACCESSORS {
        assert!(async_nouns.contains(accessor), "missing async {accessor}");
    }
    for accessor in EXPECTED_BLOCKING_ACCESSORS {
        assert!(
            blocking_nouns.contains(accessor),
            "missing blocking {accessor}"
        );
    }
    assert!(surface.contains("pub(crate) const fn surface_entry"));
    assert!(surface.contains("BuiltinCommand::ALL"));

    // Derived instead of matched against another file's formatted source: the
    // closed ledger must give every semantic row exactly one disposition.
    let semantics = include_str!("../src/command/semantics.rs");
    let dispositions = surface.matches("noun_entry!(").count()
        + surface.matches("broadcast_entry!(").count()
        + surface.matches("internal_entry!(").count();
    assert_eq!(
        dispositions,
        ledger_rows(semantics),
        "the static surface ledger drifted from the semantic inventory"
    );
}

#[test]
fn dynamic_control_inventory_is_closed() {
    let owner = include_str!("../src/dynapi/owner_projection.rs");
    let nouns = include_str!("../src/dynapi/nouns.rs");
    let custom = include_str!("../src/dynapi/custom.rs");
    let mut actual = public_trait_names(&[owner, nouns]);
    actual.extend(["DynAppliedRequest", "DynTargetedRequest"].map(str::to_owned));
    actual.sort();
    actual.dedup();
    assert_eq!(actual, EXPECTED_DYN_TRAITS);

    for (name, expected) in EXPECTED_DYN_NOUN_METHOD_COUNTS {
        assert_eq!(
            trait_methods(nouns, &[*name]).len(),
            *expected,
            "dynamic noun {name} drifted"
        );
    }
    assert!(custom.contains("pub trait DynTargetedRequest"));
    assert!(custom.contains("pub trait DynAppliedRequest"));
    for forbidden in [
        "_op",
        "and_wait",
        "defog_mode",
        "nr_speed",
        "nd_filter_position",
        "fn lock(",
        "fn unlock(",
    ] {
        assert!(
            !nouns.contains(forbidden),
            "forbidden legacy dynamic surface token {forbidden:?}"
        );
    }
    // The implementation may use private `*_result` adapters to preserve
    // fallible command constructors.  Only a public result-twin method would
    // violate the closed noun surface.
    assert!(!nouns.contains("fn result("));
    assert_eq!(nouns.matches("pub trait DynSessionCameraNouns").count(), 1);

    // Derived instead of matched against the constant's formatted source: the
    // declared projection sizes must add up to the methods the noun traits
    // actually carry.  Motion is a safety/observation view, not a projection
    // of the command or inquiry ledgers.
    let projected: usize = public_trait_names(&[nouns])
        .iter()
        .filter(|name| name.as_str() != "DynSessionCameraNouns" && name.as_str() != "DynMotion")
        .map(|name| trait_methods(nouns, &[name.as_str()]).len())
        .sum();
    assert_eq!(
        projected,
        declared_usize(nouns, "DYN_NOUN_TARGET_METHOD_COUNT")
            + declared_usize(nouns, "DYN_NOUN_INQUIRY_METHOD_COUNT"),
        "the dynamic noun traits drifted from the declared projection sizes"
    );
}

#[test]
fn ecosystem_derive_test_and_extension_inventory_is_documented() {
    let inventory = include_str!("../docs/architecture_inventory.md");
    for required in [
        "blocking",
        "async",
        "runtime-tokio",
        "runtime-smol",
        "transport-serial",
        "transport-serial-tokio",
        "serde",
        "schemars",
        "ts-rs",
        "dyn-api",
        "test-utils",
        "ViscaInquiry",
        "ViscaEnum",
        "ViscaValue",
        "ScriptedBlockingTransport",
        "ScriptedTransport",
        "DeterministicExecutor",
        "ViscaCameraSimulator",
        "Connect",
        "CameraConfig",
        "BlockingTransport",
        "AsyncTransport",
        "execute",
        "inquire",
        "submit",
    ] {
        assert!(
            inventory.contains(&format!("`{required}`")),
            "missing supported-surface inventory entry for {required}"
        );
    }
    assert!(
        !inventory.contains("`mode-async`"),
        "legacy compatibility feature must not be advertised"
    );
    assert!(
        !inventory.contains("`async-core`"),
        "internal async feature must not be advertised"
    );
}

#[test]
fn downstream_derive_inventory_compiles_and_preserves_behavior() {
    let mut wire = [0_u8; InventoryInquiry::MAX_SIZE];
    let len = InventoryInquiry
        .write_into(CameraId::CAMERA_1, &mut wire)
        .expect("inventory inquiry must encode");
    assert_eq!(&wire[..len], &[0x81, 0x09, 0x04, 0x00, 0xFF]);
    let _route = Inquiry::route(&InventoryInquiry);
    let _decoder = Inquiry::decoder(&InventoryInquiry);

    assert_eq!(
        InventoryEnum::try_from(1).expect("enum discriminant 1 is valid"),
        InventoryEnum::First
    );
    assert_eq!(u8::from(InventoryEnum::Second), 2);
    assert_eq!(
        InventoryValue::new(1).expect("value 1 is valid"),
        InventoryValue(1)
    );
    assert!(InventoryValue::new(3).is_err());
}

#[test]
fn ecosystem_feature_inventory_matches_cargo_manifest() {
    let manifest = include_str!("../Cargo.toml");
    let mut in_features = false;
    let mut features = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line == "[features]" {
            in_features = true;
            continue;
        }
        if in_features && line.starts_with('[') {
            break;
        }
        if in_features && !line.starts_with('#') {
            if let Some((name, _)) = line.split_once('=') {
                let name = name.trim();
                if name != "default" {
                    features.push(name);
                }
            }
        }
    }
    features.sort();
    assert_eq!(
        features,
        [
            "async",
            "blocking",
            "dyn-api",
            "runtime-smol",
            "runtime-tokio",
            "schemars",
            "serde",
            "tcp",
            "test-utils",
            "transport-serial",
            "transport-serial-tokio",
            "ts-rs",
        ]
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn test_utility_inventory_compiles_in_the_selected_mode() {
    use grafton_visca::testing::testkit::{helpers, Step};
    #[cfg(any(feature = "async", feature = "blocking"))]
    use std::marker::PhantomData;
    use std::time::Duration;

    let _ = helpers::ack(1);
    let _ = Step::After {
        delay: Duration::ZERO,
        responses: vec![],
    };

    #[cfg(feature = "blocking")]
    {
        use grafton_visca::testing::testkit::ScriptedBlockingTransport;
        let _: PhantomData<ScriptedBlockingTransport> = PhantomData;
    }

    #[cfg(feature = "async")]
    {
        use grafton_visca::testing::testkit::{
            DeterministicClock, DeterministicExecutor, DeterministicExecutorExt, ScriptedTransport,
            TestExecutorSelector, TestExecutorType, TestExecutors,
        };
        let _: PhantomData<DeterministicClock> = PhantomData;
        let _: PhantomData<DeterministicExecutor> = PhantomData;
        let _: PhantomData<ScriptedTransport<()>> = PhantomData;
        let _: PhantomData<TestExecutorType> = PhantomData;
        let _: PhantomData<TestExecutors> = PhantomData;
        fn _requires_deterministic_ext<T: DeterministicExecutorExt>() {}
        fn _requires_selector<T: TestExecutorSelector>() {}
    }

    #[cfg(feature = "runtime-tokio")]
    {
        use grafton_visca::testing::ViscaCameraSimulator;
        let _: PhantomData<ViscaCameraSimulator> = PhantomData;
    }
}
