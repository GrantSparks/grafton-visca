use grafton_visca::{
    capabilities::TypedSupportSet, profiles::CompileTimeProfile, profiles::ProfileId,
};
struct Custom;
impl CompileTimeProfile for Custom {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
    const ID: Option<ProfileId> = None;
}
fn main() {}

//~ E0046
//~ "missing: `spec`"
