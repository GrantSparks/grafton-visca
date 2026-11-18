include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    turn::coordinator::OwnerCoordinator::<u32>::retain();
    //~^ E0061 "this function takes 2 arguments but 0 arguments were supplied"
});
