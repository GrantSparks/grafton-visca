use grafton_visca::{
    profiles::CommandTimeoutClass,
    request::{CommandPolicy, CommandRetryClass, InquiryClass, Request, SubmissionClass},
    types::CameraId,
};

struct InquiryWithCommandPolicy;

impl Request for InquiryWithCommandPolicy {
    type Class = InquiryClass;

    const MAX_SIZE: usize = 3;

    fn policy(&self) -> CommandPolicy {
    //~^ E0053 "method `policy` has an incompatible type for trait"
        CommandPolicy::new(
            CommandTimeoutClass::Quick,
            CommandRetryClass::Standard,
            SubmissionClass::Normal,
        )
    }

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize> {
        buffer[..3].copy_from_slice(&[0x80 | target.value(), 0x09, 0xff]);
        Ok(3)
    }
}

fn main() {}

// The sealed InquiryClass selects InquiryPolicy even for a downstream Request.
