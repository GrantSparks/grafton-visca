use grafton_visca::{__macro_support::InquiryKind, command::ViscaCommand, types::CameraId, Error};

struct CustomCommand;

impl ViscaCommand for CustomCommand {
    type Response = ();

    const MAX_SIZE: usize = 2;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        buffer[0] = 0x80 | camera_id.value();
        buffer[1] = grafton_visca::wire::VISCA_TERMINATOR;
        Ok(Self::MAX_SIZE)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}

fn main() {}

//~ E0432
//~ "unresolved import `grafton_visca::command::ViscaCommand`"
