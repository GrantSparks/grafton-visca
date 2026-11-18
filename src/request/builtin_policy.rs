use crate::request::{CommandPolicy, CommandRetryClass, OperationPolicy};
use crate::{CommandTimeoutClass, SubmissionClass};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinPolicy {
    pub(crate) timeout: CommandTimeoutClass,
    pub(crate) retry: CommandRetryClass,
    pub(crate) class: SubmissionClass,
}
impl BuiltinPolicy {
    pub(crate) const fn command(self) -> CommandPolicy {
        CommandPolicy::new(self.timeout, self.retry, self.class)
    }
    pub(crate) const fn operation(self) -> OperationPolicy {
        OperationPolicy::new(self.timeout, self.retry, self.class)
    }
}

/// Resolves and checks the policy derived from a built-in's ledger row.
/// This function is evaluated by the generated request's associated constants.
pub(crate) const fn policy(
    stop: bool,
    operation: bool,
    explicit: Option<(CommandTimeoutClass, CommandRetryClass, SubmissionClass)>,
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
    explicit: Option<(CommandTimeoutClass, CommandRetryClass, SubmissionClass)>,
) -> (CommandTimeoutClass, CommandRetryClass, SubmissionClass) {
    if stop {
        (
            CommandTimeoutClass::Quick,
            CommandRetryClass::Standard,
            SubmissionClass::User,
        )
    } else if let Some(policy) = explicit {
        policy
    } else if operation {
        (
            CommandTimeoutClass::Movement,
            CommandRetryClass::Movement,
            SubmissionClass::User,
        )
    } else {
        (
            CommandTimeoutClass::Quick,
            CommandRetryClass::Standard,
            SubmissionClass::Normal,
        )
    }
}
