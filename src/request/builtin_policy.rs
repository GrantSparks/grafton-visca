use crate::{ControlClass, RetryClass, TimeoutClass};

/// Resolves and checks the policy derived from a built-in's ledger row.
/// This function is evaluated by the generated request's associated constants.
pub(crate) const fn policy(
    stop: bool,
    cancellation: bool,
    operation: bool,
    explicit: Option<(TimeoutClass, RetryClass, ControlClass)>,
) -> (TimeoutClass, RetryClass, ControlClass) {
    if let Some(policy) = explicit {
        assert!(
            stop || cancellation || !matches!(policy.2, ControlClass::Urgent),
            "only a STOP ledger row may declare an urgent request policy",
        );
        assert!(
            !stop
                || matches!(
                    policy,
                    (
                        TimeoutClass::Quick,
                        RetryClass::Movement,
                        ControlClass::Urgent
                    )
                ),
            "a STOP ledger row requires the quick movement urgent policy",
        );
        policy
    } else if stop {
        (
            TimeoutClass::Quick,
            RetryClass::Movement,
            ControlClass::Urgent,
        )
    } else if operation {
        (
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
    } else {
        (
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        )
    }
}
