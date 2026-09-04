use grafton_visca::{
    completion::Targeted, request, CameraId, ControlClass, Error, OperationCommand, Request,
    RetryClass, TimeoutClass,
};

struct MissingAffectedAxes;

impl Request for MissingAffectedAxes {
    type Class = request::Operation<Targeted>;

    const MAX_SIZE: usize = 2;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        buffer[..2].copy_from_slice(&[target.to_address_byte(), 0xff]);
        Ok(2)
    }
}

fn requires_operation<O: OperationCommand<Targeted>>(_: O) {}

fn main() {
    requires_operation(MissingAffectedAxes);
}

//~ E0277
//~ "MissingAffectedAxes: OperationCommand<"
