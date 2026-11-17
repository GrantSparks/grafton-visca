use crate::request::{CommandPolicy, CommandRetryClass, OperationPolicy};
use crate::{CommandTimeoutClass, ControlClass};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinPolicy {
    pub(crate) timeout: CommandTimeoutClass,
    pub(crate) retry: CommandRetryClass,
    pub(crate) class: ControlClass,
}
impl BuiltinPolicy {
    const fn submission(self) -> crate::SubmissionClass {
        match self.class {
            ControlClass::Background => crate::SubmissionClass::Background,
            ControlClass::Normal => crate::SubmissionClass::Normal,
            ControlClass::User | ControlClass::Urgent => crate::SubmissionClass::User,
        }
    }
    pub(crate) const fn command(self) -> CommandPolicy {
        CommandPolicy::new(self.timeout, self.retry, self.submission())
    }
    pub(crate) const fn operation(self) -> OperationPolicy {
        OperationPolicy::new(self.timeout, self.retry, self.submission())
    }
}

/// Resolves and checks the policy derived from a built-in's ledger row.
/// This function is evaluated by the generated request's associated constants.
pub(crate) const fn policy(
    stop: bool,
    operation: bool,
    explicit: Option<(CommandTimeoutClass, CommandRetryClass, ControlClass)>,
) -> BuiltinPolicy {
    let resolved = resolve(stop, operation, explicit);
    BuiltinPolicy {
        timeout: resolved.0,
        retry: resolved.1,
        class: resolved.2,
    }
}

const fn resolve(
    stop: bool,
    operation: bool,
    explicit: Option<(CommandTimeoutClass, CommandRetryClass, ControlClass)>,
) -> (CommandTimeoutClass, CommandRetryClass, ControlClass) {
    if let Some(policy) = explicit {
        assert!(
            stop || !matches!(policy.2, ControlClass::Urgent),
            "only a STOP ledger row may declare an urgent request policy",
        );
        assert!(
            !stop
                || matches!(
                    policy,
                    (
                        CommandTimeoutClass::Quick,
                        CommandRetryClass::Standard,
                        ControlClass::Urgent
                    )
                ),
            "a STOP ledger row requires the quick standard urgent policy",
        );
        policy
    } else if stop {
        (
            CommandTimeoutClass::Quick,
            CommandRetryClass::Standard,
            ControlClass::Urgent,
        )
    } else if operation {
        (
            CommandTimeoutClass::Movement,
            CommandRetryClass::Movement,
            ControlClass::User,
        )
    } else {
        (
            CommandTimeoutClass::Quick,
            CommandRetryClass::Standard,
            ControlClass::Normal,
        )
    }
}
