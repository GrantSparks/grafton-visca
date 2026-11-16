#![allow(dead_code)]
pub use grafton_visca::request::{ControlClass, RetryClass, TimeoutClass};
mod builtin_policy {
    include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/src/request/builtin_policy.rs"));
}
// The generated builtin_request! policy constants evaluate this same checker.
const POLICY: (TimeoutClass, RetryClass, ControlClass) = builtin_policy::policy(
    true, false, true,
    Some((TimeoutClass::Quick, RetryClass::Movement, ControlClass::Normal)),
);
fn main() { let _ = POLICY; }
//~ E0080
//~ "a STOP ledger row requires the quick movement urgent policy"
