//! Mechanical preservation inventory for issue #548 (Phase 1 of #542).

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

use source_scan::{builtin_command_rows, declarations};

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

/// Every method each dynamic noun trait declares.
///
/// A per-trait *count* could not see a delete-one-add-one edit inside one
/// trait, which is the drift a closed surface most needs to notice. The
/// spellings are the surface, so the spellings are what is pinned.
const EXPECTED_DYN_NOUN_METHODS: &[(&str, &[&str])] = &[
    ("DynPower", &["off", "on", "state"]),
    (
        "DynZoom",
        &[
            "position",
            "set_digital_zoom",
            "set_normalized",
            "set_normalized_in_domain",
            "set_position",
            "stop",
            "tele",
            "tele_variable",
            "wide",
            "wide_variable",
        ],
    ),
    ("DynSystem", &["save_settings", "version"]),
    (
        "DynPanTilt",
        &[
            "absolute",
            "down",
            "home",
            "left",
            "limit_clear",
            "limit_set",
            "move_direction",
            "position",
            "relative",
            "reset",
            "right",
            "stop",
            "up",
        ],
    ),
    (
        "DynFocus",
        &[
            "auto",
            "far",
            "far_variable",
            "infinity",
            "manual",
            "mode",
            "near",
            "near_limit",
            "near_variable",
            "one_push",
            "position",
            "push_af_press",
            "push_af_release",
            "range",
            "sensitivity",
            "set_lock",
            "set_near_limit",
            "set_position",
            "set_sensitivity",
            "set_zone",
            "snap",
            "stop",
            "toggle",
            "zone",
        ],
    ),
    (
        "DynPresets",
        &["recall", "reset", "set", "set_recall_speed"],
    ),
    (
        "DynExposure",
        &[
            "auto_slow_shutter_off",
            "auto_slow_shutter_on",
            "brightness",
            "brightness_direct",
            "brightness_down",
            "brightness_reset",
            "brightness_set",
            "brightness_up",
            "compensation",
            "compensation_direct",
            "compensation_down",
            "compensation_enabled",
            "compensation_off",
            "compensation_on",
            "compensation_position",
            "compensation_reset",
            "compensation_up",
            "dynamic_range",
            "flicker_mode",
            "gain",
            "gain_direct",
            "gain_down",
            "gain_limit",
            "gain_reset",
            "gain_up",
            "iris",
            "iris_control",
            "iris_direct",
            "iris_down",
            "iris_reset",
            "iris_up",
            "mode",
            "set_anti_flicker",
            "set_dynamic_range",
            "set_gain_limit",
            "set_mode",
            "shutter",
            "shutter_direct",
            "shutter_down",
            "shutter_reset",
            "shutter_up",
            "spotlight_off",
            "spotlight_on",
        ],
    ),
    (
        "DynWhiteBalance",
        &[
            "atw",
            "auto",
            "blue_gain",
            "blue_tuning",
            "color_temperature",
            "color_temperature_mode",
            "decrease_blue_gain",
            "decrease_color_temperature",
            "decrease_red_gain",
            "increase_blue_gain",
            "increase_color_temperature",
            "increase_red_gain",
            "indoor",
            "manual",
            "mode",
            "one_push",
            "one_push_trigger",
            "outdoor",
            "red_gain",
            "red_tuning",
            "reset_blue_gain",
            "reset_color_temperature",
            "reset_red_gain",
            "sensitivity",
            "set_blue_gain",
            "set_blue_tuning",
            "set_color_temperature",
            "set_red_gain",
            "set_red_tuning",
            "set_sensitivity",
        ],
    ),
    (
        "DynImage",
        &[
            "backlight",
            "black_white",
            "black_white_mode",
            "contrast",
            "decrease_sharpness",
            "defog_level",
            "disable_flip",
            "disable_horizontal_flip",
            "disable_noise_reduction_2d",
            "disable_noise_reduction_3d",
            "enable_flip",
            "enable_horizontal_flip",
            "flip",
            "flip_mode",
            "freeze_off",
            "freeze_on",
            "gamma",
            "hue",
            "increase_sharpness",
            "luminance",
            "noise_reduction_2d",
            "noise_reduction_3d",
            "noise_reduction_level",
            "noise_reduction_mode",
            "picture_effect",
            "reset_sharpness",
            "resolution",
            "saturation",
            "set_backlight",
            "set_contrast",
            "set_flip_both",
            "set_flip_mode",
            "set_gamma",
            "set_hue",
            "set_luminance",
            "set_noise_reduction_2d",
            "set_noise_reduction_3d",
            "set_picture_effect",
            "set_saturation",
            "set_sharpness",
            "set_sharpness_mode",
            "sharpness_level",
            "sharpness_mode",
        ],
    ),
    (
        "DynTally",
        &[
            "auto_adjust_enabled",
            "bright_hi",
            "bright_lo",
            "flash",
            "green_off",
            "green_on",
            "green_status",
            "off",
            "on",
            "red_off",
            "red_on",
            "red_status",
            "status",
        ],
    ),
    (
        "DynNdFilter",
        &[
            "auto_off",
            "auto_on",
            "position",
            "preset",
            "set_mode",
            "set_stops",
            "set_value",
            "step_down",
            "step_up",
        ],
    ),
    (
        "DynMotionSync",
        &["mode", "preset", "set_mode", "set_preset", "set_speed"],
    ),
    (
        "DynMenu",
        &[
            "cancel",
            "direct",
            "display",
            "navigate",
            "select",
            "status",
            "toggle_display",
        ],
    ),
    (
        "DynAdvanced",
        &[
            "auto_trace_enabled",
            "broadcast_domain",
            "digital_mode_enabled",
            "digital_ptz_enabled",
            "focus_unlock",
            "multicast_off",
            "multicast_on",
            "night_day_mode",
            "set_ndi_quality",
            "set_variable_speed_mode",
            "standby_enabled",
            "two_tone_mode_enabled",
            "usb_audio_enabled",
            "usb_audio_off",
            "usb_audio_on",
        ],
    ),
    (
        "DynMotion",
        &[
            "is_moving",
            "is_moving_axes",
            "stop_all_motion",
            "wait_until_idle",
        ],
    ),
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

/// Every method one dynamic noun trait declares.
///
/// Since #617 the declarations are generated from `src/noun_table.rs`, so a
/// trait body is a `noun_table!` invocation naming the arm it is generated
/// from.  This reader follows that invocation into the table rather than
/// reporting an empty trait: the property being pinned — the exact method
/// spellings of each closed noun — is unchanged, only its source moved.
fn trait_methods(source: &str, table: &str, traits: &[&str]) -> Vec<String> {
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
            if let Some(rest) = trimmed.strip_prefix("noun_table!(") {
                let noun = rest
                    .split("=>")
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                methods.extend(source_scan::noun_table_methods(table, &noun));
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
    // Declaration regions only: both noun files list every accessor name as a
    // string literal inside their in-file inventory tests, so a `contains`
    // check over the whole file survives deleting the accessor itself.
    let async_nouns = declarations(include_str!("../src/async_nouns.rs"));
    let blocking_nouns = declarations(include_str!("../src/blocking_nouns.rs"));
    let surface = declarations(include_str!("../src/command/surface.rs"));

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
    let semantics = declarations(include_str!("../src/command/semantics.rs"));
    let dispositions = surface.matches("noun_entry!(").count()
        + surface.matches("broadcast_entry!(").count()
        + surface.matches("internal_entry!(").count();
    assert_eq!(
        dispositions,
        builtin_command_rows(&semantics),
        "the static surface ledger drifted from the semantic inventory"
    );
}

#[test]
fn dynamic_control_inventory_is_closed() {
    let owner = declarations(include_str!("../src/dynapi/owner_projection.rs"));
    let nouns = declarations(include_str!("../src/dynapi/nouns.rs"));
    // The dynamic noun traits are generated from the shared row table, so the
    // spellings this gate pins are read from there.
    let table = declarations(include_str!("../src/noun_table.rs"));
    let custom = declarations(include_str!("../src/dynapi/custom.rs"));
    // The forbidden-token gates below are negative, so they read the whole file:
    // a legacy spelling hiding in a test module is still a legacy spelling.
    let whole_nouns = include_str!("../src/dynapi/nouns.rs");
    let mut actual = public_trait_names(&[&owner, &nouns]);
    actual.extend(["DynAppliedRequest", "DynTargetedRequest"].map(str::to_owned));
    actual.sort();
    actual.dedup();
    assert_eq!(actual, EXPECTED_DYN_TRAITS);

    for (name, expected) in EXPECTED_DYN_NOUN_METHODS {
        assert_eq!(
            trait_methods(&nouns, &table, &[*name]),
            *expected,
            "dynamic noun {name} drifted"
        );
    }
    // Every declared trait is covered by the inventory above, so a whole new
    // noun cannot appear without a row of its own.
    let mut inventoried: Vec<String> = EXPECTED_DYN_NOUN_METHODS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    inventoried.push("DynSessionCameraNouns".to_owned());
    inventoried.sort();
    assert_eq!(inventoried, public_trait_names(&[&nouns]));
    assert!(custom.contains("pub trait DynTargetedRequest"));
    assert!(custom.contains("pub trait DynAppliedRequest"));
    // `_op` was a bare `contains` here: any innocent identifier containing that
    // sequence — `set_optical_*`, `stop_operation`, a future `*_option` — would
    // have tripped it, and a real legacy method could hide inside a longer
    // word. The legacy spelling is a method *named* `<noun>_op`, so match
    // declarations rather than characters.
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
    // The implementation may use private `*_result` adapters to preserve
    // fallible command constructors.  Only a public result-twin method would
    // violate the closed noun surface.
    assert!(!whole_nouns.contains("fn result("));
    assert_eq!(nouns.matches("pub trait DynSessionCameraNouns").count(), 1);

    // Derived instead of matched against the constant's formatted source: the
    // declared projection sizes must add up to the methods the noun traits
    // actually carry.  Motion is a safety/observation view, not a projection
    // of the command or inquiry ledgers.  Every noun method is therefore one
    // of exactly three things: a command row, a typed inquiry, or one of the
    // declared non-ledger convenience wrappers, whose own membership is gated
    // in `src/dynapi/nouns.rs`.
    let projected: usize = public_trait_names(&[&nouns])
        .iter()
        .filter(|name| name.as_str() != "DynSessionCameraNouns" && name.as_str() != "DynMotion")
        .map(|name| trait_methods(&nouns, &table, &[name.as_str()]).len())
        .sum();
    assert_eq!(
        projected,
        declared_usize(&nouns, "DYN_NOUN_TARGET_METHOD_COUNT")
            + declared_usize(&nouns, "DYN_NOUN_INQUIRY_METHOD_COUNT")
            + declared_usize(&nouns, "DYN_NOUN_CONVENIENCE_METHOD_COUNT"),
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

/// Every `name = value` entry of the manifest's `[features]` table, in
/// declaration order and with comment lines dropped.
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

/// The 1.x feature aliases removed by 2.0. A name absent from the whole
/// `[features]` table — as a key *and* from every feature's implied list — is a
/// name Cargo rejects outright with `does not contain this feature: <name>`,
/// which is what the deleted `removed-features` CI job spent a job slot
/// re-proving against a live compiler.
const REMOVED_1X_FEATURE_ALIASES: [&str; 3] = ["mode-async", "async-core", "mode-blocking"];

#[test]
fn removed_1x_feature_aliases_are_absent_from_the_manifest() {
    let table = manifest_feature_table();
    for removed in REMOVED_1X_FEATURE_ALIASES {
        for (name, value) in &table {
            assert_ne!(
                *name, removed,
                "removed 1.x feature alias {removed} is declared again"
            );
            let implied = value
                .trim_matches(|c: char| c == '[' || c == ']')
                .split(',')
                .map(|entry| entry.trim().trim_matches('"'));
            for entry in implied {
                assert_ne!(
                    entry, removed,
                    "removed 1.x feature alias {removed} is implied by feature {name}"
                );
            }
        }
    }
}

/// `tcp` gated no code: there was never a `cfg(feature = "tcp")` in the crate,
/// so the marker only made a supported build look configurable. Standard TCP
/// ships with whichever facade is enabled and this test pins that it stays a
/// non-feature.
#[test]
fn tcp_is_not_a_feature() {
    for (name, value) in manifest_feature_table() {
        assert_ne!(name, "tcp", "`tcp` gates no code and must not be a feature");
        assert!(
            !value.contains("\"tcp\""),
            "feature {name} implies the removed `tcp` marker"
        );
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
