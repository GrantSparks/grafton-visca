//! Policies indexed by the sealed request lifecycle.

use super::{CommandRetryClass, InquiryRetryClass};
use crate::raw::{OperationReplyShape, RawReplyShape};
use crate::{CommandTimeoutClass, SubmissionClass};
#[cfg(feature = "serde")]
use crate::{Error, Result};

/// Policy for [`super::PlainClass`] requests, including [`crate::raw::PlainRequest`].
///
/// A command timeout cannot be an inquiry timeout, and [`crate::request::SubmissionClass`]
/// cannot select the owner's urgent safety lane (#679).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "CommandPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct CommandPolicy {
    timeout: CommandTimeoutClass,
    retry: CommandRetryClass,
    class: SubmissionClass,
    reply_shape: RawReplyShape,
}

/// Policy for [`super::OperationClass`] requests, including raw operations.
///
/// Its reply shape requires terminal camera evidence and cannot be `NoReply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "OperationPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct OperationPolicy {
    timeout: CommandTimeoutClass,
    retry: CommandRetryClass,
    class: SubmissionClass,
    reply_shape: OperationReplyShape,
}

/// Policy for [`super::InquiryClass`] requests, including [`crate::raw::InquiryRequest`].
///
/// Inquiry timeout and reply protocols are fixed by the request class, so this
/// policy stores neither a timeout class nor a command reply shape. Its
/// [`InquiryRetryClass`] permits only `Never` or `Inquiry`, so command retry
/// families cannot silently change reply-loss handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "InquiryPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct InquiryPolicy {
    retry: InquiryRetryClass,
    class: SubmissionClass,
}

// Persistence re-enters the public constructors rather than writing private
// policy fields. Typed fields retain the closed command-timeout, ordinary-lane
// and operation-reply subsets. These representations are never schema or TS
// exports: those describe the public policy's actual serialized fields.
#[cfg(feature = "serde")]
macro_rules! command_policy_data {
    ($policy:ident, $data:ident, $reply:ty) => {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $data {
            timeout: CommandTimeoutClass,
            retry: CommandRetryClass,
            class: SubmissionClass,
            reply_shape: $reply,
        }

        impl TryFrom<$data> for $policy {
            type Error = Error;

            fn try_from(value: $data) -> Result<Self> {
                Ok(Self::new(value.timeout, value.retry, value.class)
                    .with_reply_shape(value.reply_shape))
            }
        }
    };
}

#[cfg(feature = "serde")]
command_policy_data!(CommandPolicy, CommandPolicyData, RawReplyShape);
#[cfg(feature = "serde")]
command_policy_data!(OperationPolicy, OperationPolicyData, OperationReplyShape);

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InquiryPolicyData {
    retry: InquiryRetryClass,
    class: SubmissionClass,
}

#[cfg(feature = "serde")]
impl TryFrom<InquiryPolicyData> for InquiryPolicy {
    type Error = Error;

    fn try_from(value: InquiryPolicyData) -> Result<Self> {
        Ok(Self::new(value.retry, value.class))
    }
}

macro_rules! ordinary_policy_accessors {
    () => {
        /// Returns the selected ordinary submission class.
        #[must_use]
        pub const fn submission_class(self) -> SubmissionClass {
            self.class
        }
    };
}

macro_rules! command_policy {
    ($policy:ident, $reply:ident) => {
        impl $policy {
            /// Creates an explicit policy in an ordinary submission lane.
            ///
            /// `timeout_class` selects the registered profile's command deadline.
            #[must_use]
            pub const fn new(
                timeout_class: CommandTimeoutClass,
                retry: CommandRetryClass,
                class: SubmissionClass,
            ) -> Self {
                Self {
                    timeout: timeout_class,
                    retry,
                    class,
                    reply_shape: $reply::AckThenCompletion,
                }
            }

            /// Returns this policy with the declared reply protocol.
            #[must_use]
            pub const fn with_reply_shape(mut self, reply_shape: $reply) -> Self {
                self.reply_shape = reply_shape;
                self
            }

            /// Returns the selected command timeout class.
            #[must_use]
            pub const fn timeout_class(self) -> CommandTimeoutClass {
                self.timeout
            }

            /// Returns the selected retry class.
            #[must_use]
            pub const fn retry_class(self) -> CommandRetryClass {
                self.retry
            }

            ordinary_policy_accessors!();
        }
    };
}

command_policy!(CommandPolicy, RawReplyShape);
command_policy!(OperationPolicy, OperationReplyShape);

impl CommandPolicy {
    /// Returns the declared command reply protocol.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.reply_shape
    }
}

impl OperationPolicy {
    /// Returns the declared operation reply protocol as its raw wire shape.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.reply_shape.raw()
    }
}

impl InquiryPolicy {
    /// Creates an inquiry policy in an ordinary submission lane.
    #[must_use]
    pub const fn new(retry: InquiryRetryClass, class: SubmissionClass) -> Self {
        Self { retry, class }
    }

    /// Returns the selected inquiry retry family.
    #[must_use]
    pub const fn retry_class(self) -> InquiryRetryClass {
        self.retry
    }

    ordinary_policy_accessors!();
}

/// Admission of a command according to its ordinary policy or reviewed STOP
/// semantics. The inquiry policy has only an ordinary submission class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAdmission {
    Ordinary(SubmissionClass),
    Stop,
}

pub(crate) type AdmissionOf<C> = <<C as crate::request::Class>::Policy as Sealed>::Admission;

pub trait Sealed: Copy {
    type Admission;
    fn ordinary_admission(self) -> Self::Admission;
}

impl Sealed for CommandPolicy {
    type Admission = CommandAdmission;

    fn ordinary_admission(self) -> Self::Admission {
        CommandAdmission::Ordinary(self.submission_class())
    }
}
impl Sealed for OperationPolicy {
    type Admission = CommandAdmission;

    fn ordinary_admission(self) -> Self::Admission {
        CommandAdmission::Ordinary(self.submission_class())
    }
}
impl Sealed for InquiryPolicy {
    type Admission = SubmissionClass;

    fn ordinary_admission(self) -> Self::Admission {
        self.submission_class()
    }
}
