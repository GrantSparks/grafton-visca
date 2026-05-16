use grafton_visca::{
    command::{InquiryKind, ViscaCommand},
    CameraId, Error,
};

struct CustomCommand;

impl ViscaCommand for CustomCommand {
    type Response = ();

    const MAX_SIZE: usize = 2;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = grafton_visca::command::VISCA_TERMINATOR;
        Ok(Self::MAX_SIZE)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}

fn main() {}
