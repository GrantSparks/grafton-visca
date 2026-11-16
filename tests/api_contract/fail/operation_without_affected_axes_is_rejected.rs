use grafton_visca::{
    operation::Targeted, request, request::ControlClass, request::OperationCommand,
    request::Request, request::RetryClass, request::TimeoutClass, types::CameraId, Error,
};

struct MissingAffectedAxes;

impl Request for MissingAffectedAxes {
    type Class = request::OperationClass<Targeted>;

    const MAX_SIZE: usize = 2;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        buffer[..2].copy_from_slice(&[(0x80 | target.value()), 0xff]);
        Ok(2)
    }
}

fn requires_operation<O: OperationCommand<Targeted>>(_: O) {}

fn main() {
    requires_operation(MissingAffectedAxes);
}

//~ E0277
//~ "MissingAffectedAxes: OperationCommand<"
