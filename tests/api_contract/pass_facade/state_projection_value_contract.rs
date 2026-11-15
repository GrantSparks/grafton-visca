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

struct Value(bool);
impl contract::BuiltinStateValue<bool> for Value {
    fn state_value(&self) -> bool {
        self.0
    }
}

fn main() {
    let _ = <contract::Set<Tally> as contract::BuiltinStateProjection<Value>>::project(&Value(true));
    let _ = <contract::Clear<Tally> as contract::BuiltinStateProjection<Value>>::project(&Value(false));
    let _ = <contract::Invalidate<Tally> as contract::BuiltinStateProjection<()>>::project(&());
    let _ = <contract::NoState as contract::BuiltinStateProjection<()>>::project(&());
}
