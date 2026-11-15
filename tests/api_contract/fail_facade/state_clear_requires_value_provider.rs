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

fn main() {
    let _ = <contract::Clear<Tally> as contract::BuiltinStateProjection<MissingValue>>::project(&MissingValue);
}

//~ E0277
//~ "the trait bound `MissingValue: BuiltinStateValue<bool>` is not satisfied"
