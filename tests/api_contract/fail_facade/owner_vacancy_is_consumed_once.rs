include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    use turn::coordinator::{Disposition, OwnerCoordinator, Selected, TurnPlan};
    let mut coordinator = OwnerCoordinator::<u32>::default();
    let now = std::time::Instant::now();
    let due = crate::runtime::engine::RawCorrelationReleaseSet;
    if let TurnPlan::Select { turn, .. } = coordinator.plan(now, None, due, || false) {
        if let Disposition::Retain(vacancy) = turn.classify(Selected::Boundary, due) {
            OwnerCoordinator::retain(vacancy, 7);
            OwnerCoordinator::retain(vacancy, 8);
        }
    }
});

//~ E0382
//~ "use of moved value: `vacancy`"
