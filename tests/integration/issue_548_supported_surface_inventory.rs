//! Mechanical preservation inventory for issue #548 (Phase 1 of #542).
//!
//! The source checks in this file are deliberately limited to public names,
//! legacy spellings, and readable inventory totals. The noun registry's
//! command/method correctness is compiled and tested inside the crate; this
//! integration test must not rebuild that registry by parsing its tokens.

#[allow(unused_imports)]
pub use grafton_visca::Error;
pub mod error {
    #[allow(unused_imports)]
    pub use grafton_visca::Error;
}

use grafton_visca::{
    profiles::ProfileId,
    request::{Inquiry, Request},
    types::CameraId,
    value, value_enum, ViscaInquiry,
};

use grafton_visca::wire::WireValue;
use grafton_visca_test_support::source_scan;

use source_scan::declarations;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
struct InventoryInquiry;

#[value_enum]
enum InventoryEnum {
    First = 1,
    Second = 2,
}

#[value(min = 1, max = 2)]
struct InventoryValue(u8);

const EXPECTED_PROFILES: &[&str] = &[
    "PtzOpticsG2",
    "PtzOpticsG3",
    "PtzOptics30x",
    "SonyFr7",
    "SonyBrcH900",
    "SonyEviH100",
    "SonyBrc300",
    "NearusBrc300",
    "GenericVisca",
];

fn assert_same_names(mut actual: Vec<String>, mut documented: Vec<String>, label: &str) {
    actual.sort_unstable();
    documented.sort_unstable();
    assert!(
        actual.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate {label} source name"
    );
    assert!(
        documented.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate {label} document name"
    );
    assert_eq!(
        actual, documented,
        "{label} document must match production source"
    );
}

/// Pins the public static accessor names and makes each one a compile-time
/// fact: every name must resolve in each enabled facade, so a renamed or
/// missing accessor fails to build rather than relying on a source scan.
macro_rules! static_accessor_inventory {
    ($($accessor:ident),* $(,)?) => {
        const EXPECTED_ACCESSORS: &[&str] = &[$(stringify!($accessor)),*];

        #[cfg(feature = "async")]
        fn async_accessors_resolve<P>()
        where
            P: grafton_visca::profiles::CompileTimeProfile
                + grafton_visca::capabilities::HasPower
                + grafton_visca::capabilities::HasZoom
                + grafton_visca::capabilities::HasFocus
                + grafton_visca::capabilities::HasPanTilt
                + grafton_visca::capabilities::HasPresets
                + grafton_visca::capabilities::HasExposure
                + grafton_visca::capabilities::HasWhiteBalance
                + grafton_visca::capabilities::HasImage
                + grafton_visca::capabilities::HasMenu
                + grafton_visca::capabilities::HasTally
                + grafton_visca::capabilities::HasNdFilter
                + grafton_visca::capabilities::HasMotionSync
                + 'static,
        {
            $( let _: Option<grafton_visca::r#async::$accessor<'static, P>> = None; )*
        }

        #[cfg(feature = "blocking")]
        fn blocking_accessors_resolve<P>()
        where
            P: grafton_visca::profiles::CompileTimeProfile
                + grafton_visca::capabilities::HasPower
                + grafton_visca::capabilities::HasZoom
                + grafton_visca::capabilities::HasFocus
                + grafton_visca::capabilities::HasPanTilt
                + grafton_visca::capabilities::HasPresets
                + grafton_visca::capabilities::HasExposure
                + grafton_visca::capabilities::HasWhiteBalance
                + grafton_visca::capabilities::HasImage
                + grafton_visca::capabilities::HasMenu
                + grafton_visca::capabilities::HasTally
                + grafton_visca::capabilities::HasNdFilter
                + grafton_visca::capabilities::HasMotionSync
                + 'static,
        {
            $( let _: Option<grafton_visca::blocking::$accessor<'static, P>> = None; )*
        }
    };
}

static_accessor_inventory!(
    ExposureAccessor,
    FocusAccessor,
    ImageAccessor,
    MenuAccessor,
    MotionSyncAccessor,
    NdFilterAccessor,
    PanTiltAccessor,
    PowerAccessor,
    PresetsAccessor,
    StreamingAccessor,
    SystemAccessor,
    TallyAccessor,
    WhiteBalanceAccessor,
    ZoomAccessor,
);

#[test]
fn built_in_profile_and_transport_inventory_is_closed() {
    let actual: Vec<_> = ProfileId::all()
        .iter()
        .map(|profile| format!("{profile:?}"))
        .collect();
    assert_eq!(actual, EXPECTED_PROFILES);

    // (TCP port, UDP port, serial) per profile, from each model's source:
    // PTZOptics R1/R5 raw ports, Sony R7/R11 encapsulated UDP, and the
    // RS-232C/RS-422-only EVI-H100 (R8), BRC-300 (R12) and Nearus (R21).
    let expected = [
        (Some(5678), Some(1259), true),
        (Some(5678), Some(1259), true),
        (Some(5678), Some(1259), true),
        (None, Some(52381), false),
        (None, Some(52381), false),
        (None, None, true),
        (None, None, true),
        (None, None, true),
        (Some(5678), Some(1259), true),
    ];
    for (profile, (tcp, udp, serial)) in ProfileId::all().iter().zip(expected) {
        assert_eq!(profile.default_tcp_port(), tcp, "{profile:?} TCP drift");
        assert_eq!(profile.default_udp_port(), udp, "{profile:?} UDP drift");
        assert_eq!(
            profile
                .spec()
                .expect("registered profile")
                .transports()
                .supports_serial(),
            serial,
            "{profile:?} serial drift"
        );
        assert_eq!(
            profile
                .spec()
                .expect("registered profile")
                .transports()
                .tcp_port()
                .is_some(),
            tcp.is_some()
        );
        assert_eq!(
            profile
                .spec()
                .expect("registered profile")
                .transports()
                .udp_port()
                .is_some(),
            udp.is_some()
        );
    }
}

#[test]
fn built_in_group_and_vendor_inventory_is_closed() {
    use grafton_visca::{capabilities::InquirySupport, profiles::ProfileGroup};

    let expected = [
        (
            ProfileGroup::GenericVisca,
            &[
                ProfileId::GenericVisca,
                ProfileId::SonyBrc300,
                ProfileId::SonyEviH100,
                ProfileId::NearusBrc300,
            ][..],
            (false, false, true, false),
            InquirySupport::Partial,
        ),
        (
            ProfileGroup::PtzOptics,
            &[
                ProfileId::PtzOpticsG2,
                ProfileId::PtzOpticsG3,
                ProfileId::PtzOptics30x,
            ][..],
            (true, true, true, false),
            InquirySupport::Full,
        ),
        (
            ProfileGroup::SonyProfessional,
            &[ProfileId::SonyFr7, ProfileId::SonyBrcH900][..],
            (false, true, false, true),
            InquirySupport::Full,
        ),
    ];
    for (group, members, (tcp, udp, serial, sony), inquiry) in expected {
        assert_eq!(group.profiles(), members, "{group:?} membership");
        for member in members {
            assert_eq!(member.profile_group(), group, "{member:?} group");
        }
        assert_eq!(
            (
                group.supports_tcp(),
                group.supports_udp(),
                group.supports_serial(),
                group.uses_sony_encapsulation()
            ),
            (tcp, udp, serial, sony),
            "{group:?} transport facts"
        );
        assert_eq!(
            group.inquiry_support(),
            inquiry,
            "{group:?} inquiry support"
        );
    }

    let vendors: Vec<_> = ProfileId::all()
        .iter()
        .map(|profile| profile.vendor())
        .collect();
    assert_eq!(
        vendors,
        [
            "PtzOptics",
            "PtzOptics",
            "PtzOptics",
            "Sony",
            "Sony",
            "Sony",
            "Sony",
            "Nearus",
            "Generic"
        ]
    );
}

#[test]
fn static_noun_and_control_inventory_is_closed() {
    // The accessor names are compile-time facts (`static_accessor_inventory!`);
    // this keeps the closed list itself readable and duplicate-free.
    let mut sorted = EXPECTED_ACCESSORS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, EXPECTED_ACCESSORS);
    #[cfg(feature = "async")]
    async_accessors_resolve::<
        grafton_visca_test_support::profile_fixtures::OptionalSurfacesTypedSupport,
    >();
    #[cfg(feature = "blocking")]
    blocking_accessors_resolve::<
        grafton_visca_test_support::profile_fixtures::OptionalSurfacesTypedSupport,
    >();

    let noun_tokens = source_scan::production_tokens(include_str!("../../src/noun_table.rs"));
    let source_names: Vec<_> = noun_tokens
        .windows(3)
        .filter(|tokens| {
            tokens[0] == "accessor" && tokens[1] == ":" && tokens[2].ends_with("Accessor")
        })
        .map(|tokens| tokens[2].clone())
        .collect();
    let mut compiled_names: Vec<_> = EXPECTED_ACCESSORS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    compiled_names.push("MotionAccessor".to_owned());
    assert_same_names(source_names, compiled_names, "compiled static nouns");

    let surface = declarations(include_str!("../../src/command/surface.rs"));
    assert!(surface.contains("pub(crate) const fn surface_entry"));
    // No second command inventory: canonical noun rows project their exact
    // protocol identities, and the const index checks their ledger classes.
    assert_eq!(surface.matches("noun_table!(surface_registry)").count(), 1);
    assert!(surface.contains(
        "rows: <$request as crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS"
    ));
    assert!(surface.contains("registry_class(row, surface.kind)"));
}

#[test]
fn runtime_profile_control_inventory_uses_the_native_accessors() {
    // Runtime profiles resolve the same concrete accessor types as typed profiles.
    // The public AnyProfile fixture also checks every method's native future output.
    #[cfg(feature = "async")]
    {
        use grafton_visca::{profiles::AnyProfile, r#async::*};
        let _: Option<PowerAccessor<'static, AnyProfile>> = None;
        let _: Option<PanTiltAccessor<'static, AnyProfile>> = None;
        let _: Option<MotionAccessor<'static>> = None;
    }
    #[cfg(feature = "blocking")]
    {
        use grafton_visca::{blocking::*, profiles::AnyProfile};
        let _: Option<PowerAccessor<'static, AnyProfile>> = None;
        let _: Option<PanTiltAccessor<'static, AnyProfile>> = None;
        let _: Option<MotionAccessor<'static>> = None;
    }

    let noun_tokens = source_scan::production_tokens(include_str!("../../src/noun_table.rs"));
    let legacy_methods: Vec<_> = noun_tokens
        .windows(2)
        .filter(|tokens| tokens[1] == "(" && tokens[0].ends_with("_op"))
        .map(|tokens| tokens[0].clone())
        .collect();
    assert!(
        legacy_methods.is_empty(),
        "forbidden legacy noun methods: {legacy_methods:?}"
    );
}

#[test]
fn ecosystem_derive_test_and_extension_inventory_is_documented() {
    let inventory = include_str!("../../docs/architecture_inventory.md");
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
        "test-utils",
        "ViscaInquiry",
        "value_enum",
        "value",
        "ScriptedBlockingTransport",
        "ScriptedTransport",
        "ManualClockExecutor",
        "ViscaCameraSimulator",
        "CameraSession",
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
    let _decoder = Inquiry::decoder(&InventoryInquiry);

    assert_eq!(
        InventoryEnum::from_wire(1).expect("enum discriminant 1 is valid"),
        InventoryEnum::First
    );
    assert_eq!(InventoryEnum::Second.to_wire(), 2);
    assert_eq!(
        InventoryValue::new(1).expect("value 1 is valid"),
        InventoryValue(1)
    );
    assert!(InventoryValue::new(3).is_err());
}

/// Every `name = value` entry of the manifest's `[features]` table.
fn manifest_feature_table() -> Vec<(&'static str, &'static str)> {
    let manifest = include_str!("../../Cargo.toml");
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
    let _ = Step::after(Duration::ZERO, vec![]);
    #[cfg(any(feature = "blocking", feature = "async"))]
    {
        use grafton_visca::testing::testkit::{ClockJoinHandle, ClockParticipant, ManualClock};
        let _: PhantomData<ManualClock> = PhantomData;
        let _: PhantomData<ClockParticipant> = PhantomData;
        let _: PhantomData<ClockJoinHandle<()>> = PhantomData;
    }

    #[cfg(feature = "blocking")]
    {
        use grafton_visca::testing::testkit::ScriptedBlockingTransport;
        let _: PhantomData<ScriptedBlockingTransport> = PhantomData;
    }

    #[cfg(feature = "async")]
    {
        use grafton_visca::testing::testkit::{
            LocalExecutor, ManualClockExecutor, ManualClockTimer, ScriptedTransport, TaskJoin,
        };
        let _: PhantomData<ManualClockExecutor> = PhantomData;
        let _: PhantomData<ManualClockTimer> = PhantomData;
        let _: PhantomData<LocalExecutor> = PhantomData;
        let _: PhantomData<TaskJoin<()>> = PhantomData;
        let _: PhantomData<ScriptedTransport> = PhantomData;
    }

    #[cfg(feature = "runtime-tokio")]
    {
        use grafton_visca::testing::ViscaCameraSimulator;
        let _: PhantomData<ViscaCameraSimulator> = PhantomData;
    }
}
