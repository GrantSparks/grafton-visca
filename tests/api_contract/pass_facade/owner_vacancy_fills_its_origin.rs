include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    use turn::coordinator::{OwnerCoordinator, TurnPlan};
    let mut coordinator = OwnerCoordinator::<u32>::default();
    let now = std::time::Instant::now();
    let due = crate::runtime::engine::RawCorrelationReleaseSet;
    if let TurnPlan::Select { turn, .. } = coordinator.plan(now, None, due, || false) {
        {
            let vacancy = vacancy(turn);
            OwnerCoordinator::retain(vacancy, 7);
        }
    }
});
