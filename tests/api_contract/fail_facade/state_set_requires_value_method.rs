#![allow(dead_code)]

pub use grafton_visca::{Error, KnownState, ProfileSpec, StateKey, TargetRegistration};

mod contract {
    include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/src/request/builtin_contract.rs"));
}

struct Tally;
impl contract::StateValue for Tally {
    const KEY: StateKey = StateKey::TallyMode;
    type Value = bool;
    fn known(value: bool) -> KnownState {
        KnownState::TallyMode(value)
    }
}

struct MissingValue;
impl contract::BuiltinStateValue<bool> for MissingValue {}

fn main() {
    // This is the same projection obligation that builtin_request! generates.
    let _ = <contract::Set<Tally> as contract::BuiltinStateProjection<MissingValue>>::project(&MissingValue);
}

//~ E0046
//~ "missing `state_value` in implementation"
