use grafton_visca::{
    capabilities::TypedSupportSet, profiles::CompileTimeProfile, profiles::ProfileSpec,
};
struct Custom;
impl CompileTimeProfile for Custom {
//~^ E0046 "missing: `ID`"
    const DOMAINS: grafton_visca::capabilities::DomainSet = grafton_visca::capabilities::DomainSet::empty();
    const TRANSPORTS: grafton_visca::capabilities::TransportSet = grafton_visca::capabilities::TransportSet::empty();
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        unimplemented!()
    }
}
fn main() {}
