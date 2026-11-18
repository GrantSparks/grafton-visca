include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/tests/api_contract/support/owner_coordinator.rs"));

owner_coordinator_contract!({
    use turn::coordinator::{OwnerCoordinator, TurnPlan};
    let mut first = OwnerCoordinator::<u32>::default();
    let mut second = OwnerCoordinator::<u32>::default();
    let now = std::time::Instant::now();
    let due = crate::runtime::engine::RawCorrelationReleaseSet;
    if let TurnPlan::Select { turn, .. } = first.plan(now, None, due, || false) {
        {
            let vacancy = vacancy(turn);
            second.retain(vacancy, 7);
            //~^ E0599 "no method named `retain` found for struct `OwnerCoordinator<u32>`"
        }
    }
});
