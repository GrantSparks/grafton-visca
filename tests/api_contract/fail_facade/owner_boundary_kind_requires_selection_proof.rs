include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    use turn::coordinator::{OwnerCoordinator, Selected, TurnPlan};
    let mut coordinator = OwnerCoordinator::<u32>::default();
    let now = std::time::Instant::now();
    let due = crate::runtime::engine::RawCorrelationReleaseSet;
    if let TurnPlan::Select { turn, .. } = coordinator.plan(now, None, due, || false) {
        turn.classify(Selected::Boundary, due);
    }
});

//~ E0308
//~ "expected `NonBoundarySelected`, found `Selected`"
