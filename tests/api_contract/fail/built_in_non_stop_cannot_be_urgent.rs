#![allow(dead_code)]
pub use grafton_visca::{
    profiles::CommandTimeoutClass,
    request::{self, CommandRetryClass, SubmissionClass},
};
// The included checker exhaustively matches its crate-local scheduling domain.
// Model that closed domain here without an external non_exhaustive match error.
#[derive(Debug, Clone, Copy)]
pub enum ControlClass {
    Background,
    Normal,
    User,
    Urgent,
}
mod builtin_policy {
    include!(concat!(
        env!("GRAFTON_VISCA_SOURCE_ROOT"),
        "/src/request/builtin_policy.rs"
    ));
}
// The generated builtin_request! policy constants evaluate this same checker.
const POLICY: builtin_policy::BuiltinPolicy = builtin_policy::policy(
    false,
    true,
    Some((
        CommandTimeoutClass::Quick,
        CommandRetryClass::Movement,
        ControlClass::Urgent,
    )),
);
fn main() {
    let _ = POLICY;
}
//~ E0080
//~ "only a STOP ledger row may declare an urgent request policy"
