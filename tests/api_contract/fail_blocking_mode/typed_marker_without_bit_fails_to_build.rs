//@ build
#![cfg(feature = "blocking")]
//! An optional marker cannot bypass its compile-time permission bit.
use grafton_visca::{
    capabilities::HasNdFilter, capabilities::TypedSupportSet, profiles::CompileTimeProfile,
    profiles::ProfileId, profiles::ProfileSpec,
};
#[derive(Debug, Clone, Copy)]
struct NoNdPermission;
impl CompileTimeProfile for NoNdPermission {
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
            .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(None)
            .build()
    }
}
impl HasNdFilter for NoNdPermission {}
fn main() {
    let camera: Option<grafton_visca::blocking::Camera<NoNdPermission>> = None;
    let _ = camera
        .unwrap()
        .nd_filter()
        .set_density(grafton_visca::types::NdFilterDensity::new(0).unwrap());
}

//~ E0080
//~ "profile marker `HasNdFilter` requires its typed-support bit"
