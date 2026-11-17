include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    let mut coordinator = turn::coordinator::OwnerCoordinator::<u32>::default();
    let _ = turn::coordinator::VacantSlot { coordinator: &mut coordinator };
});

//~ E0451
//~ "field `coordinator` of struct `VacantSlot` is private"
