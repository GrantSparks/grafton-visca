#![allow(dead_code)]

pub use grafton_visca::{
    config::TargetRegistration, profiles::ProfileSpec, state_cache::KnownState,
    state_cache::StateKey, types::TallyMode, Error,
};

mod contract {
    include!(concat!(
        env!("GRAFTON_VISCA_SOURCE_ROOT"),
        "/src/request/builtin_contract.rs"
    ));
}

struct Tally;
impl contract::StateValue for Tally {
    const KEY: StateKey = StateKey::TallyMode;
    type Value = TallyMode;
    fn known(value: TallyMode) -> KnownState {
        KnownState::TallyMode(value)
    }
}

struct MissingValue;
impl contract::BuiltinStateValue<TallyMode> for MissingValue {}

fn main() {
    // This is the same projection obligation that builtin_request! generates.
    let _ = <contract::Set<Tally> as contract::BuiltinStateProjection<MissingValue>>::project(
        &MissingValue,
    );
}

//~ E0046
//~ "missing `state_value` in implementation"
