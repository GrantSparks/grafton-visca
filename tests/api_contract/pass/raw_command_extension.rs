use grafton_visca::{
    command::{CommandKind, InquiryKind, ViscaCommand},
    timeout::CommandCategory,
    CameraId, Error,
};

struct CustomCommand;

impl ViscaCommand for CustomCommand {
    type Response = ();

    const MAX_SIZE: usize = 2;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0xFF;
        Ok(Self::MAX_SIZE)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}

fn main() {
    let command = CustomCommand;
    let encoded = command.to_fixed_bytes::<2>(CameraId::CAMERA_1).unwrap();
    assert_eq!(encoded.as_slice(), &[0x81, 0xFF]);
    assert_eq!(command.command_kind(), CommandKind::Command);
}
