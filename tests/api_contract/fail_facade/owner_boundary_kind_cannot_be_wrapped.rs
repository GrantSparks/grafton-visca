include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    use turn::coordinator::{NonBoundarySelected, Selected};
    let _ = NonBoundarySelected(Selected::Boundary);
    //~^ E0423 "cannot initialize a tuple struct which contains private fields"
});
