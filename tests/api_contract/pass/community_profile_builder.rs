//! A community profile needs only validated facts and the matching marker.
use grafton_visca::{
    capabilities::Capabilities, capabilities::CapabilityRange, capabilities::HasZoom,
    capabilities::TypedSupportSet, capabilities::ZoomCaps, profiles::CompileTimeProfile,
    profiles::GenericVisca, profiles::ProfileEnvelope, profiles::ProfileId, profiles::ProfileSpec,
    profiles::TransportCompatibility,
};
struct Community;
impl CompileTimeProfile for Community {
    const DOMAINS: grafton_visca::capabilities::DomainSet = grafton_visca::capabilities::DomainSet::from_domains(&[grafton_visca::capabilities::Domain::Zoom]);
    const TRANSPORTS: grafton_visca::capabilities::TransportSet = grafton_visca::profiles::TransportCompatibility::new().with_tcp(5678).transport_set();
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    const ID: Option<ProfileId> = None;
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        let baseline = GenericVisca::spec()?;
        let mut facts = Capabilities::runtime_baseline("Community zoom", 1)?;
        facts.zoom = Some(ZoomCaps {
            optical_max: 0x4000,
            digital_max: None,
            speed: CapabilityRange::<u8>::new(0, 7)?,
            direct: false,
            variable: true,
            optical_ratio: None,
        });
        ProfileSpec::builder(facts)
            .transports(TransportCompatibility::new().with_tcp(5678))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(baseline.timing())
            .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(None)
            .build()
    }
}
impl HasZoom for Community {}
fn requires_zoom<P: CompileTimeProfile + HasZoom>() {}
fn main() {
    requires_zoom::<Community>();
    let spec = ProfileSpec::from_compile_time::<Community>().unwrap();
    assert!(spec.capabilities().zoom.is_some());
    assert!(spec.capabilities().pan_tilt.is_none());
    assert_eq!(Community::ID, None);
}
