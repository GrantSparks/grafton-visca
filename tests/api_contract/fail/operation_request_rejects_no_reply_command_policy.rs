use grafton_visca::{
    operation::AppliedOnly,
    profiles::CommandTimeoutClass,
    raw::RawReplyShape,
    request::{CommandPolicy, CommandRetryClass, OperationClass, Request, SubmissionClass},
    types::CameraId,
    Error,
};

struct NoReplyOperation;

impl Request for NoReplyOperation {
    type Class = OperationClass<AppliedOnly>;
    const MAX_SIZE: usize = 2;

    fn policy(&self) -> CommandPolicy {
    //~^ E0053 "method `policy` has an incompatible type for trait"
        CommandPolicy::new(
            CommandTimeoutClass::Quick,
            CommandRetryClass::Never,
            SubmissionClass::Normal,
        )
        .with_reply_shape(RawReplyShape::NoReply)
    }

    fn write_into(&self, _: CameraId, _: &mut [u8]) -> Result<usize, Error> {
        Ok(2)
    }
}

fn main() {}
