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
    type Value = TallyMode;
    fn known(value: TallyMode) -> KnownState {
        KnownState::TallyMode(value)
    }
}

struct MissingValue;

fn main() {
    let _ = <contract::Clear<Tally> as contract::BuiltinStateProjection<MissingValue>>::project(
    //~^ E0277 "the trait bound `MissingValue: BuiltinStateValue<grafton_visca::types::TallyMode>` is not satisfied"
        &MissingValue,
    );
}
