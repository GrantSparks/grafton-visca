//! Implementing the profile contract does not grant every domain marker.
use grafton_visca::{
    capabilities::HasZoom, capabilities::TypedSupportSet, profiles::CompileTimeProfile,
    profiles::ProfileId, profiles::ProfileSpec,
};
struct Community;
impl CompileTimeProfile for Community {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    const ID: Option<ProfileId> = None;
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        let baseline = grafton_visca::profiles::GenericVisca::spec()?;
        let facts =
            grafton_visca::capabilities::Capabilities::runtime_baseline("Community camera", 1)?;
        ProfileSpec::builder(facts)
            .transports(baseline.transports())
            .envelope(baseline.envelope())
            .timing(baseline.timing())
            .maximum_command_sockets(1)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(None)
            .build()
    }
}
fn requires_zoom<P: CompileTimeProfile + HasZoom>() {}
fn main() {
    requires_zoom::<Community>();
}

//~ E0277
//~ "profile `Community` does not declare Zoom support"
