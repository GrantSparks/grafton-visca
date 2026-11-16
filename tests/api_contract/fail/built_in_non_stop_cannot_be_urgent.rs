#![allow(dead_code)]
pub use grafton_visca::request::{ControlClass, RetryClass, TimeoutClass};
mod builtin_policy {
    include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/src/request/builtin_policy.rs"));
}
// The generated builtin_request! policy constants evaluate this same checker.
const POLICY: (TimeoutClass, RetryClass, ControlClass) = builtin_policy::policy(
    false, false, true,
    Some((TimeoutClass::Quick, RetryClass::Movement, ControlClass::Urgent)),
);
fn main() { let _ = POLICY; }
//~ E0080
//~ "only a STOP ledger row may declare an urgent request policy"
