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

struct NoEffect;

impl contract::BuiltinValidation for NoEffect {
    fn applied_state(&self) -> Option<contract::AppliedStateProjection> {
    //~^ E0407 "method `applied_state` is not a member of trait `contract::BuiltinValidation`"
        Some(contract::AppliedStateProjection::set(
            KnownState::TallyMode(TallyMode::On),
        ))
    }
}

fn main() {}
