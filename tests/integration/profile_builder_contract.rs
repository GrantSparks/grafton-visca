//! Community profiles use one validated fact model and explicit permissions.
#![allow(clippy::expect_used)]
use grafton_visca::{
    capabilities::{
        Capabilities, CoordinateSystem, PanTiltWireCodec, TypedSupportSet, TypedSupportSurface,
    },
    profiles::{
        CompileTimeProfile, GenericVisca, PanTiltCoordinateConversion, ProfileEnvelope, ProfileId,
        ProfileSpec, ProfileSpecBuilder, TransportCompatibility,
    },
    Error,
};

fn community_facts() -> Capabilities {
    Capabilities::runtime_baseline("Community camera", 1).expect("identity")
}
fn community_builder() -> ProfileSpecBuilder {
    let base = GenericVisca::spec().expect("baseline");
    ProfileSpec::builder(community_facts())
        .transports(TransportCompatibility::new().with_tcp(5678))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(base.timing())
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(None)
}
struct Community;
impl CompileTimeProfile for Community {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    const ID: Option<ProfileId> = None;
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        community_builder().build()
    }
}
#[test]
fn community_profile_builds_without_a_metadata_trait_forest() {
    let spec = ProfileSpec::from_compile_time::<Community>().expect("community profile");
    assert_eq!(spec.name(), "Community camera");
    assert_eq!(spec.capabilities().profile_id, None);
    assert!(spec.capabilities().zoom.is_none());
    assert!(!spec.supports_command_cancel());
}
#[test]
fn builder_requires_explicit_operational_facts() {
    let error = ProfileSpec::builder(community_facts())
        .build()
        .expect_err("missing facts");
    assert!(
        matches!(&error, Error::InvalidRequest { reason: message }
        if message.contains("`transports`") && message.contains("is required")),
        "{error:?}"
    );
}
#[test]
fn invalid_network_ports_and_empty_transport_sets_are_rejected() {
    for transports in [
        TransportCompatibility::new().with_tcp(0),
        TransportCompatibility::new().with_udp(0),
        TransportCompatibility::new(),
    ] {
        assert!(community_builder().transports(transports).build().is_err());
    }
}
#[test]
fn sony_envelope_rejects_serial_transport() {
    assert!(community_builder()
        .transports(TransportCompatibility::new().with_serial())
        .envelope(ProfileEnvelope::SonyEncapsulated)
        .build()
        .is_err());
}
#[test]
fn inconsistent_sony_position_framing_is_rejected_by_the_checked_conversion() {
    let error = PanTiltCoordinateConversion::new(
        CoordinateSystem::UnsignedCentered,
        PanTiltWireCodec::SonyBrc300,
        1.0,
        1.0,
    )
    .expect_err("Sony framing requires signed coordinates");
    assert!(error.to_string().contains("signed-centered"), "{error}");
}
#[test]
fn optional_permissions_cannot_enable_an_absent_physical_domain() {
    let mut caps = community_facts();
    caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
    let base = GenericVisca::spec().expect("baseline");
    let error = ProfileSpec::builder(caps)
        .transports(base.transports())
        .envelope(base.envelope())
        .timing(base.timing())
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()
        .expect_err("absent zoom domain");
    assert!(error.to_string().contains("DirectZoom"), "{error}");
}
#[test]
fn adapter_rejects_static_identity_and_permission_disagreements() {
    struct WrongIdentity;
    impl CompileTimeProfile for WrongIdentity {
        const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
        const ID: Option<ProfileId> = Some(ProfileId::GenericVisca);
        fn spec() -> grafton_visca::Result<ProfileSpec> {
            Community::spec()
        }
    }
    struct WrongPermissions;
    impl CompileTimeProfile for WrongPermissions {
        const TYPED_SUPPORT: TypedSupportSet =
            TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        const ID: Option<ProfileId> = None;
        fn spec() -> grafton_visca::Result<ProfileSpec> {
            Community::spec()
        }
    }
    let error = ProfileSpec::from_compile_time::<WrongIdentity>().expect_err("identity mismatch");
    assert!(
        error.to_string().contains("CompileTimeProfile::ID"),
        "{error}"
    );
    let error =
        ProfileSpec::from_compile_time::<WrongPermissions>().expect_err("permission mismatch");
    assert!(
        error
            .to_string()
            .contains("CompileTimeProfile::TYPED_SUPPORT"),
        "{error}"
    );
}

#[test]
fn builder_requires_an_explicit_cancellation_fact() {
    let base = GenericVisca::spec().expect("baseline");
    let error = ProfileSpec::builder(community_facts())
        .transports(base.transports())
        .envelope(base.envelope())
        .timing(base.timing())
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(false)
        .position_inquiries(None)
        .build()
        .expect_err("cancellation must be declared");
    assert!(
        matches!(&error, Error::InvalidRequest { reason: message }
        if message.contains("`supports_command_cancel`") && message.contains("is required")),
        "{error}"
    );
}

#[cfg(feature = "serde")]
#[test]
fn persisted_profile_ids_use_explicit_stable_spellings() {
    for (id, spelling) in [
        (ProfileId::PtzOpticsG2, "ptzoptics_g2"),
        (ProfileId::PtzOpticsG3, "ptzoptics_g3"),
        (ProfileId::PtzOptics30x, "ptzoptics_30x"),
        (ProfileId::SonyFr7, "sony_fr7"),
        (ProfileId::SonyBrcH900, "sony_brc_h900"),
        (ProfileId::SonyEviH100, "sony_evi_h100"),
        (ProfileId::SonyBrc300, "sony_brc_300"),
        (ProfileId::NearusBrc300, "nearus_brc_300"),
        (ProfileId::GenericVisca, "generic_visca"),
    ] {
        assert_eq!(serde_json::to_value(id).expect("serialize ID"), spelling);
        assert_eq!(
            serde_json::from_value::<ProfileId>(spelling.into()).expect("deserialize ID"),
            id
        );
        let spec = id.spec().expect("registry spec");
        assert_eq!(spec.capabilities().profile_id, Some(id));
    }
    let group = grafton_visca::profiles::ProfileGroup::PtzOptics;
    assert_eq!(
        serde_json::to_value(group).expect("serialize group"),
        "ptzoptics"
    );
}
