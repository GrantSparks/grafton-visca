use grafton_visca::{profiles::CompileTimeProfile, profiles::ProfileId, profiles::ProfileSpec};
struct Custom;
impl CompileTimeProfile for Custom {
//~^ E0046 "missing: `TYPED_SUPPORT`"
    const DOMAINS: grafton_visca::capabilities::DomainSet = grafton_visca::capabilities::DomainSet::empty();
    const TRANSPORTS: grafton_visca::capabilities::TransportSet = grafton_visca::capabilities::TransportSet::empty();
    const ID: Option<ProfileId> = None;
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        unimplemented!()
    }
}
fn main() {}
