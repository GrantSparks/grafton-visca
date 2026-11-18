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

struct Value(TallyMode);
impl contract::BuiltinStateValue<TallyMode> for Value {
    fn state_value(&self) -> TallyMode {
        self.0
    }
}

fn main() {
    let _ = <contract::Set<Tally> as contract::BuiltinStateProjection<Value>>::project(&Value(
        TallyMode::On,
    ));
    let _ = <contract::Clear<Tally> as contract::BuiltinStateProjection<Value>>::project(&Value(
        TallyMode::Off,
    ));
    let _ = <contract::NoState as contract::BuiltinStateProjection<()>>::project(&());
}
