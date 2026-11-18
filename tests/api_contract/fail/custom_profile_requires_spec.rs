use grafton_visca::{
    capabilities::TypedSupportSet, profiles::CompileTimeProfile, profiles::ProfileId,
};
struct Custom;
impl CompileTimeProfile for Custom {
//~^ E0046 "missing: `spec`"
    const DOMAINS: grafton_visca::capabilities::DomainSet = grafton_visca::capabilities::DomainSet::empty();
    const TRANSPORTS: grafton_visca::capabilities::TransportSet = grafton_visca::capabilities::TransportSet::empty();
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    const ID: Option<ProfileId> = None;
}
fn main() {}
