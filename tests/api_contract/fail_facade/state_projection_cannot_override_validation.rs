#![allow(dead_code)]

pub use grafton_visca::{Error, KnownState, ProfileSpec, StateKey, TargetRegistration};

mod contract {
    include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/src/request/builtin_contract.rs"));
}

struct NoEffect;

impl contract::BuiltinValidation for NoEffect {
    fn applied_state(&self) -> Option<contract::AppliedStateProjection> {
        Some(contract::AppliedStateProjection::set(KnownState::TallyMode(true)))
    }
}

fn main() {}

//~ E0407
//~ "method `applied_state` is not a member of trait `contract::BuiltinValidation`"
