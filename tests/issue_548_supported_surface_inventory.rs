//! Mechanical preservation inventory for issue #548 (Phase 1 of #542).
//!
//! The source checks in this file are deliberately limited to public names,
//! legacy spellings, and readable inventory totals. The noun registry's
//! command/method correctness is compiled and tested inside the crate; this
//! integration test must not rebuild that registry by parsing its tokens.

pub use grafton_visca::Error;
pub mod error {
    pub use grafton_visca::Error;
}

use grafton_visca::{
    camera::profiles::ProfileId, capabilities::TypedSupportSurface, CameraId, Inquiry, Request,
    ViscaEnum, ViscaInquiry, ViscaValue,
};

#[path = "common/source_scan.rs"]
mod source_scan;

use source_scan::declarations;

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
    "NoiseReduction2D",
    "NoiseReduction3D",
    "PictureEffect",
    "Tally",
    "DirectMenu",
    "NdFilter",
    "VariableSpeed",
    "MotionSync",
    "FocusZoneInquiry",
    "UsbAudio",
    "PtzOpticsAntiFlicker",
    "PtzOpticsSettingsSave",
    "PtzOpticsPresetRecallSpeed",
    "SonySpotlight",
    "SonyAutoSlowShutter",
    "PtzOpticsMulticastStreaming",
    "PtzOpticsNdiQuality",
    "ExposureMode",
    "IrisControlInquiry",
    "NoiseReduction2DControl",
    "NoiseReduction3DControl",
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

fn public_trait_names(sources: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for source in sources {
        for line in source.lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub trait ") else {
                continue;
            };
            let name = rest
                .split(|character: char| {
                    character == '<' || character == ':' || character.is_whitespace()
                })
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
    let async_nouns = declarations(include_str!("../src/async_nouns.rs"));
    let blocking_nouns = declarations(include_str!("../src/blocking_nouns.rs"));
    let surface = declarations(include_str!("../src/command/surface.rs"));

    for accessor in EXPECTED_ACCESSORS {
        assert!(async_nouns.contains(accessor), "missing async {accessor}");
        assert!(
            blocking_nouns.contains(accessor),
            "missing blocking {accessor}"
        );
    }
    assert!(surface.contains("pub(crate) const fn surface_entry"));
    assert!(surface.contains("BuiltinCommand::ALL"));

    // The generated facades each consume one invocation per one of the 14
    // registry noun arms. The compiled parity test checks the actual names;
    // this integration gate keeps the count visible to reviewers.
    assert_eq!(async_nouns.matches("=> async_noun_methods);").count(), 14);
    assert_eq!(
        blocking_nouns.matches("=> blocking_noun_methods);").count(),
        14
    );

    // The command registry is intentionally readable at a glance: 150 IDs,
    // of which 147 are target-facing and three are protocol exceptions.
    assert!(surface.contains("BUILTIN_COMMAND_COUNT: usize = 150"));
    assert!(surface.contains("TARGET_FACING_COMMAND_COUNT: usize = 147"));
    assert!(surface.contains("NON_NOUN_COMMAND_COUNT: usize = 3"));
}

#[test]
fn dynamic_control_inventory_is_closed() {
    let owner = declarations(include_str!("../src/dynapi/owner_projection.rs"));
    let nouns = declarations(include_str!("../src/dynapi/nouns.rs"));
    let custom = declarations(include_str!("../src/dynapi/custom.rs"));
    let whole_nouns = include_str!("../src/dynapi/nouns.rs");

    let mut actual = public_trait_names(&[&owner, &nouns]);
    actual.extend(["DynAppliedRequest", "DynTargetedRequest"].map(str::to_owned));
    actual.sort();
    actual.dedup();
    assert_eq!(actual, EXPECTED_DYN_TRAITS);

    // Each of the 14 noun traits has one declaration and one implementation
    // projection. Both are generated from the shared registry; no method list
    // is reconstructed from source tokens here.
    assert_eq!(nouns.matches("pub trait Dyn").count(), 16); // 14 nouns + Motion + owner nouns
    assert_eq!(
        nouns.matches("noun_table!(").count(),
        28,
        "every noun arm needs declaration and implementation consumers"
    );
    assert!(custom.contains("pub trait DynTargetedRequest"));
    assert!(custom.contains("pub trait DynAppliedRequest"));

    // Keep the six public totals visible without making the test a second
    // registry. The crate's compiled tests derive the same totals from the
    // command/inquiry registries.
    for (name, value) in [
        ("DYN_NOUN_TARGET_METHOD_COUNT", "147"),
        ("DYN_NOUN_INQUIRY_METHOD_COUNT", "62"),
        ("DYN_NOUN_CONVENIENCE_METHOD_COUNT", "9"),
        ("DYN_NOUN_COUNT", "14"),
    ] {
        assert!(
            nouns.contains(&format!("{name}: usize = {value}")),
            "missing readable dynamic inventory total {name}={value}"
        );
    }

    // The forbidden-token gates below are negative, so they read the whole
    // file: a legacy spelling hiding in a test module is still a legacy
    // spelling. The legacy spelling is a method named `<noun>_op`.
    let legacy_op_methods: Vec<&str> = whole_nouns
        .lines()
        .map(str::trim_start)
        .filter_map(|line| {
            let line = line
                .strip_prefix("pub(crate) ")
                .or_else(|| line.strip_prefix("pub "))
                .unwrap_or(line);
            line.strip_prefix("fn ")
        })
        .filter_map(|rest| rest.split('(').next())
        .map(str::trim)
        .filter(|name| name.ends_with("_op"))
        .collect();
    assert!(
        legacy_op_methods.is_empty(),
        "forbidden legacy `_op` dynamic methods: {legacy_op_methods:?}"
    );

    for forbidden in [
        "and_wait",
        "defog_mode",
        "nr_speed",
        "nd_filter_position",
        "fn lock(",
        "fn unlock(",
    ] {
        assert!(
            !whole_nouns.contains(forbidden),
            "forbidden legacy dynamic surface token {forbidden:?}"
        );
    }
    assert!(!whole_nouns.contains("fn result("));
    assert_eq!(nouns.matches("pub trait DynSessionCameraNouns").count(), 1);
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
    assert!(!inventory.contains("`mode-async`"));
    assert!(!inventory.contains("`async-core`"));
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

/// Every `name = value` entry of the manifest's `[features]` table.
fn manifest_feature_table() -> Vec<(&'static str, &'static str)> {
    let manifest = include_str!("../Cargo.toml");
    let mut in_features = false;
    let mut entries = Vec::new();
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
            if let Some((name, value)) = line.split_once('=') {
                entries.push((name.trim(), value.trim()));
            }
        }
    }
    entries
}

#[test]
fn ecosystem_feature_inventory_matches_cargo_manifest() {
    let mut features: Vec<&str> = manifest_feature_table()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| *name != "default")
        .collect();
    features.sort_unstable();
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
            "test-utils",
            "transport-serial",
            "transport-serial-tokio",
            "ts-rs",
        ]
    );
}

/// The 1.x feature aliases removed by 2.0.
const REMOVED_1X_FEATURE_ALIASES: [&str; 3] = ["mode-async", "async-core", "mode-blocking"];

#[test]
fn removed_1x_feature_aliases_are_absent_from_the_manifest() {
    let table = manifest_feature_table();
    for removed in REMOVED_1X_FEATURE_ALIASES {
        for (name, value) in &table {
            assert_ne!(*name, removed);
            let implied = value
                .trim_matches(|character: char| character == '[' || character == ']')
                .split(',')
                .map(|entry| entry.trim().trim_matches('"'));
            for entry in implied {
                assert_ne!(entry, removed);
            }
        }
    }
}

#[test]
fn tcp_is_not_a_feature() {
    for (name, value) in manifest_feature_table() {
        assert_ne!(name, "tcp", "`tcp` gates no code and must not be a feature");
        assert!(!value.contains("\"tcp\""));
    }
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
