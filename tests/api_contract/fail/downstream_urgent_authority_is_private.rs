use grafton_visca::{request, CameraId, ControlClass, Request, RetryClass, TimeoutClass};

struct DownstreamUrgent;

impl Request for DownstreamUrgent {
    type Class = request::Plain;

    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Urgent;

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize> {
        buffer[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }

    fn admission_control_class(
        &self,
        _authority: grafton_visca::requests::RequestContractAuthority,
    ) -> grafton_visca::Result<ControlClass> {
        Ok(ControlClass::Urgent)
    }
}

fn main() {}

// Downstream custom requests may choose ordinary classes, but cannot name the
// crate-only admission authority that grants the urgent safety lane.
//~ E0603
//~ "module `requests` is private"
