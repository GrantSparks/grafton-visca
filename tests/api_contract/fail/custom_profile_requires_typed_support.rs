use grafton_visca::{profiles::CompileTimeProfile, profiles::ProfileId, profiles::ProfileSpec};
struct Custom;
impl CompileTimeProfile for Custom {
    const ID: Option<ProfileId> = None;
    fn spec() -> grafton_visca::Result<ProfileSpec> {
        unimplemented!()
    }
}
fn main() {}

//~ E0046
//~ "missing: `TYPED_SUPPORT`"
