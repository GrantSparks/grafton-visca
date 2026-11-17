use grafton_visca::{
    profiles::CommandTimeoutClass,
    request::{self, CommandPolicy, CommandRetryClass, ControlClass, Request, SubmissionClass},
    types::CameraId,
};

struct DownstreamUrgent;

impl Request for DownstreamUrgent {
    type Class = request::PlainClass;

    const MAX_SIZE: usize = 3;
    fn policy(&self) -> CommandPolicy {
        CommandPolicy::new(
            CommandTimeoutClass::Quick,
            CommandRetryClass::Never,
            SubmissionClass::Normal,
        )
    }

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize> {
        buffer[..3].copy_from_slice(&[(0x80 | target.value()), 0x01, 0xff]);
        Ok(3)
    }

    fn admission_control_class(
        &self,
        _authority: grafton_visca::requests::RequestContractAuthority,
    ) -> ControlClass {
        ControlClass::Urgent
    }
}

fn main() {}

// Downstream custom requests may choose ordinary classes, but cannot name the
// crate-only admission authority that grants the urgent safety lane.
//~ E0603
//~ "module `requests` is private"
