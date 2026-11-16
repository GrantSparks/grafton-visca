use grafton_visca::{
    capabilities::TypedSupportSet, profiles::CompileTimeProfile, profiles::ProfileSpec,
};
struct Custom;
impl CompileTimeProfile for Custom {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        unimplemented!()
    }
}
fn main() {}

//~ E0046
//~ "missing: `ID`"
